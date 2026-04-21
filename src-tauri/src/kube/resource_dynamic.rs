//! CRD 等资源实例的通用列表、读取与删除（Discovery + DynamicObject）。
//!
//! `build_list_params` 和 `ResourceError` 直接复用 `resources` 模块，
//! GVK 解析逻辑提取为私有辅助函数 `resolve_gvk`，消除三处重复。

use crate::kube::resources::{build_list_params, format_creation_time, ResourceError};
use kube::api::{Api, DeleteParams, DynamicObject};
use kube::core::GroupVersion;
use kube::discovery::{self, ApiCapabilities, ApiResource, Scope};
use kube::Client;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DynamicCrdInstanceItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

/// 解析 `apiVersion` + `kind` → `(ApiResource, ApiCapabilities)`，供三个公开函数共享。
async fn resolve_gvk(
    client: &Client,
    api_version: &str,
    kind: &str,
) -> Result<(ApiResource, ApiCapabilities), ResourceError> {
    let gv: GroupVersion = api_version
        .parse()
        .map_err(|e| ResourceError::Serialize(format!("apiVersion: {}", e)))?;
    let gvk = gv.with_kind(kind);
    discovery::pinned_kind(client, &gvk)
        .await
        .map_err(|e| ResourceError::Serialize(format!("无法解析资源类型: {}", e)))
}

/// 通过 Discovery 仅凭 `kind` 名称查找 `(ApiResource, ApiCapabilities)`，不需要 apiVersion。
/// 优先匹配核心组（空组），多匹配时取第一个。
async fn resolve_by_kind(
    client: &Client,
    kind: &str,
) -> Result<(ApiResource, ApiCapabilities), ResourceError> {
    let disco = discovery::Discovery::new(client.clone())
        .run()
        .await
        .map_err(|e| ResourceError::Serialize(format!("discovery 失败: {}", e)))?;
    // 优先核心组，否则取第一个匹配
    let mut fallback: Option<(ApiResource, ApiCapabilities)> = None;
    for group in disco.groups() {
        for (ar, caps) in group.recommended_resources() {
            if ar.kind == kind {
                if group.name().is_empty() {
                    return Ok((ar, caps));
                }
                if fallback.is_none() {
                    fallback = Some((ar, caps));
                }
            }
        }
    }
    fallback.ok_or_else(|| ResourceError::UnsupportedKind(kind.to_string()))
}

/// 仅凭 `kind` 获取单个对象并序列化为 YAML（供 resource_get 委托调用）。
pub async fn get_resource_yaml_by_kind(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<String, ResourceError> {
    let (ar, caps) = resolve_by_kind(client, kind).await?;
    let obj = match caps.scope {
        Scope::Cluster => {
            Api::<DynamicObject>::all_with(client.clone(), &ar)
                .get(name)
                .await
                .map_err(ResourceError::Kube)?
        }
        Scope::Namespaced => {
            let ns = namespace.unwrap_or("default");
            Api::<DynamicObject>::namespaced_with(client.clone(), ns, &ar)
                .get(name)
                .await
                .map_err(ResourceError::Kube)?
        }
    };
    let v = serde_json::to_value(&obj).map_err(|e| ResourceError::Serialize(e.to_string()))?;
    serde_yaml::to_string(&v).map_err(|e| ResourceError::Serialize(e.to_string()))
}

/// 仅凭 `kind` 删除单个对象（供 resource_delete 委托调用）。
pub async fn delete_resource_by_kind(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<(), ResourceError> {
    let (ar, caps) = resolve_by_kind(client, kind).await?;
    let dp = DeleteParams::default();
    match caps.scope {
        Scope::Cluster => {
            Api::<DynamicObject>::all_with(client.clone(), &ar)
                .delete(name, &dp)
                .await
                .map_err(ResourceError::Kube)?;
        }
        Scope::Namespaced => {
            let ns = namespace.unwrap_or("default");
            Api::<DynamicObject>::namespaced_with(client.clone(), ns, &ar)
                .delete(name, &dp)
                .await
                .map_err(ResourceError::Kube)?;
        }
    }
    Ok(())
}

/// 列出指定 GVK 的资源实例；命名空间级资源在 `namespace == Some("__all__")` 或 `None` 时跨命名空间列举。
pub async fn list_crd_instances(
    client: &Client,
    api_version: &str,
    kind: &str,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<DynamicCrdInstanceItem>, ResourceError> {
    let (ar, caps) = resolve_gvk(client, api_version, kind).await?;
    let lp = build_list_params(label_selector);
    let list = match caps.scope {
        Scope::Cluster => {
            Api::<DynamicObject>::all_with(client.clone(), &ar)
                .list(&lp)
                .await
                .map_err(ResourceError::Kube)?
        }
        Scope::Namespaced => {
            let all_ns = namespace.map(|n| n == "__all__").unwrap_or(true);
            if all_ns {
                Api::<DynamicObject>::all_with(client.clone(), &ar)
                    .list(&lp)
                    .await
                    .map_err(ResourceError::Kube)?
            } else {
                let ns = namespace.unwrap_or("default");
                Api::<DynamicObject>::namespaced_with(client.clone(), ns, &ar)
                    .list(&lp)
                    .await
                    .map_err(ResourceError::Kube)?
            }
        }
    };
    Ok(list
        .items
        .into_iter()
        .map(|item| DynamicCrdInstanceItem {
            name: item.metadata.name.unwrap_or_default(),
            namespace: item.metadata.namespace,
            creation_time: format_creation_time(item.metadata.creation_timestamp.as_ref()),
        })
        .collect())
}

/// 按 apiVersion + Kind 获取单个对象并序列化为 YAML。
pub async fn get_dynamic_resource_yaml(
    client: &Client,
    api_version: &str,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<String, ResourceError> {
    let (ar, caps) = resolve_gvk(client, api_version, kind).await?;
    let obj = match caps.scope {
        Scope::Cluster => {
            Api::<DynamicObject>::all_with(client.clone(), &ar)
                .get(name)
                .await
                .map_err(ResourceError::Kube)?
        }
        Scope::Namespaced => {
            let ns = namespace.unwrap_or("default");
            Api::<DynamicObject>::namespaced_with(client.clone(), ns, &ar)
                .get(name)
                .await
                .map_err(ResourceError::Kube)?
        }
    };
    let v = serde_json::to_value(&obj).map_err(|e| ResourceError::Serialize(e.to_string()))?;
    serde_yaml::to_string(&v).map_err(|e| ResourceError::Serialize(e.to_string()))
}

/// 删除单个动态资源实例。
pub async fn delete_dynamic_resource(
    client: &Client,
    api_version: &str,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<(), ResourceError> {
    let (ar, caps) = resolve_gvk(client, api_version, kind).await?;
    let dp = DeleteParams::default();
    match caps.scope {
        Scope::Cluster => {
            Api::<DynamicObject>::all_with(client.clone(), &ar)
                .delete(name, &dp)
                .await
                .map_err(ResourceError::Kube)?;
        }
        Scope::Namespaced => {
            let ns = namespace.unwrap_or("default");
            Api::<DynamicObject>::namespaced_with(client.clone(), ns, &ar)
                .delete(name, &dp)
                .await
                .map_err(ResourceError::Kube)?;
        }
    }
    Ok(())
}
