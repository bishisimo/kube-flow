//! CRD 等资源实例的通用列表、读取与删除（Discovery + DynamicObject）。

use crate::kube::resources::{format_creation_time, ResourceError};
use kube::api::{Api, DeleteParams, DynamicObject, ListParams};
use kube::core::GroupVersion;
use kube::discovery::{self, Scope};
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

fn build_list_params(label_selector: Option<&str>) -> ListParams {
    match label_selector {
        Some(s) if !s.trim().is_empty() => ListParams::default().labels(s.trim()),
        _ => ListParams::default(),
    }
}

/// 列出指定 GVK 的资源实例；命名空间级资源在 `namespace == Some("__all__")` 或 `None` 时跨命名空间列举。
pub async fn list_crd_instances(
    client: &Client,
    api_version: &str,
    kind: &str,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<DynamicCrdInstanceItem>, ResourceError> {
    let gv: GroupVersion = api_version
        .parse()
        .map_err(|e| ResourceError::Serialize(format!("apiVersion: {}", e)))?;
    let gvk = gv.with_kind(kind);
    let (ar, caps) = discovery::pinned_kind(client, &gvk)
        .await
        .map_err(|e| ResourceError::Serialize(format!("无法解析资源类型: {}", e)))?;
    let lp = build_list_params(label_selector);
    let list = match caps.scope {
        Scope::Cluster => {
            let api = Api::<DynamicObject>::all_with(client.clone(), &ar);
            api.list(&lp).await.map_err(ResourceError::Kube)?
        }
        Scope::Namespaced => {
            let all_ns = namespace.map(|n| n == "__all__").unwrap_or(true);
            if all_ns {
                let api = Api::<DynamicObject>::all_with(client.clone(), &ar);
                api.list(&lp).await.map_err(ResourceError::Kube)?
            } else {
                let ns = namespace.unwrap_or("default");
                let api = Api::<DynamicObject>::namespaced_with(client.clone(), ns, &ar);
                api.list(&lp).await.map_err(ResourceError::Kube)?
            }
        }
    };
    let mut out = Vec::with_capacity(list.items.len());
    for item in list.items {
        let name = item.metadata.name.clone().unwrap_or_default();
        let namespace = item.metadata.namespace.clone();
        let creation_time = format_creation_time(item.metadata.creation_timestamp.as_ref());
        out.push(DynamicCrdInstanceItem {
            name,
            namespace,
            creation_time,
        });
    }
    Ok(out)
}

/// 按 apiVersion + Kind 获取单个对象并序列化为 YAML。
pub async fn get_dynamic_resource_yaml(
    client: &Client,
    api_version: &str,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<String, ResourceError> {
    let gv: GroupVersion = api_version
        .parse()
        .map_err(|e| ResourceError::Serialize(format!("apiVersion: {}", e)))?;
    let gvk = gv.with_kind(kind);
    let (ar, caps) = discovery::pinned_kind(client, &gvk)
        .await
        .map_err(|e| ResourceError::Serialize(format!("无法解析资源类型: {}", e)))?;
    let obj = match caps.scope {
        Scope::Cluster => {
            let api = Api::<DynamicObject>::all_with(client.clone(), &ar);
            api.get(name).await.map_err(ResourceError::Kube)?
        }
        Scope::Namespaced => {
            let ns = namespace.unwrap_or("default");
            let api = Api::<DynamicObject>::namespaced_with(client.clone(), ns, &ar);
            api.get(name).await.map_err(ResourceError::Kube)?
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
    let gv: GroupVersion = api_version
        .parse()
        .map_err(|e| ResourceError::Serialize(format!("apiVersion: {}", e)))?;
    let gvk = gv.with_kind(kind);
    let (ar, caps) = discovery::pinned_kind(client, &gvk)
        .await
        .map_err(|e| ResourceError::Serialize(format!("无法解析资源类型: {}", e)))?;
    let dp = DeleteParams::default();
    match caps.scope {
        Scope::Cluster => {
            let api = Api::<DynamicObject>::all_with(client.clone(), &ar);
            api.delete(name, &dp).await.map_err(ResourceError::Kube)?;
        }
        Scope::Namespaced => {
            let ns = namespace.unwrap_or("default");
            let api = Api::<DynamicObject>::namespaced_with(client.clone(), ns, &ar);
            api.delete(name, &dp).await.map_err(ResourceError::Kube)?;
        }
    }
    Ok(())
}
