//! 配置资源：ConfigMap, Secret, ServiceAccount。

use super::{
    build_list_params, format_creation_time, list_simple_namespaced, ResourceError,
    SimpleNamespacedItem,
};
use k8s_openapi::api::core::v1::{ConfigMap, Secret, ServiceAccount};
use kube::api::Api;
use kube::Client;
use serde::Serialize;

// ── 数据结构 ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ConfigMapItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keys: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SecretItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keys: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceAccountItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

impl SimpleNamespacedItem for ServiceAccountItem {
    fn from_meta(meta: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta, default_ns: &str) -> Self {
        Self {
            name: meta.name.unwrap_or_default(),
            namespace: meta.namespace.unwrap_or_else(|| default_ns.to_string()),
            creation_time: format_creation_time(meta.creation_timestamp.as_ref()),
        }
    }
}

// ── list 函数 ──────────────────────────────────────────────────────────────

/// 列出指定 namespace 的 ConfigMaps。
pub async fn list_config_maps(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<ConfigMapItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<ConfigMap> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|c| {
            let keys = c.data.as_ref().map(|d| d.len() as u32);
            ConfigMapItem {
                name: c.metadata.name.unwrap_or_default(),
                namespace: c.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                keys,
                creation_time: format_creation_time(c.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 Secrets。
pub async fn list_secrets(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<SecretItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<Secret> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|s| {
            let keys = s.data.as_ref().map(|d| d.len() as u32);
            SecretItem {
                name: s.metadata.name.unwrap_or_default(),
                namespace: s.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                type_: s.type_.clone(),
                keys,
                creation_time: format_creation_time(s.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

list_simple_namespaced!(list_service_accounts, ServiceAccount, ServiceAccountItem);
