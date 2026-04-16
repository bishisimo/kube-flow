//! 存储资源：PersistentVolumeClaim, PersistentVolume, StorageClass。

use super::{build_list_params, format_creation_time, ResourceError};
use k8s_openapi::api::core::v1::{PersistentVolume, PersistentVolumeClaim};
use k8s_openapi::api::storage::v1::StorageClass;
use kube::api::Api;
use kube::Client;
use serde::Serialize;

// ── 数据结构 ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct PersistentVolumeClaimItem {
    pub name: String,
    pub namespace: String,
    /// 状态：Pending / Bound / Lost 等，来自 status.phase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// 容量，来自 status.capacity["storage"]，Bound 后有值
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity: Option<String>,
    /// 绑定的 PV 名称，来自 spec.volume_name（Bound 后由 controller 写入）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<String>,
    /// StorageClass 名称，来自 spec.storage_class_name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersistentVolumeItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageClassItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_volume_expansion: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

// ── list 函数 ──────────────────────────────────────────────────────────────

/// 列出指定 namespace 的 PersistentVolumeClaims。
pub async fn list_persistent_volume_claims(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<PersistentVolumeClaimItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|p| {
            let status = p.status.as_ref().and_then(|s| s.phase.clone());
            let capacity = p
                .status
                .as_ref()
                .and_then(|s| s.capacity.as_ref())
                .and_then(|c| c.get("storage"))
                .map(|q| q.0.clone());
            let volume = p.spec.as_ref().and_then(|s| s.volume_name.clone());
            let storage_class = p.spec.as_ref().and_then(|s| s.storage_class_name.clone());
            PersistentVolumeClaimItem {
                name: p.metadata.name.unwrap_or_default(),
                namespace: p.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                status,
                capacity,
                volume,
                storage_class,
                creation_time: format_creation_time(p.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出集群级 PersistentVolumes。
pub async fn list_persistent_volumes(
    client: &Client,
    label_selector: Option<&str>,
) -> Result<Vec<PersistentVolumeItem>, ResourceError> {
    let api: Api<PersistentVolume> = Api::all(client.clone());
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|p| {
            let status = p.status.as_ref().and_then(|s| s.phase.clone());
            let capacity = p
                .spec
                .as_ref()
                .and_then(|s| s.capacity.as_ref())
                .and_then(|c| c.get("storage"))
                .map(|q| q.0.clone());
            PersistentVolumeItem {
                name: p.metadata.name.unwrap_or_default(),
                capacity,
                status,
                creation_time: format_creation_time(p.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出集群级 StorageClasses。
pub async fn list_storage_classes(
    client: &Client,
    label_selector: Option<&str>,
) -> Result<Vec<StorageClassItem>, ResourceError> {
    let api: Api<StorageClass> = Api::all(client.clone());
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|s| StorageClassItem {
            name: s.metadata.name.unwrap_or_default(),
            provisioner: Some(s.provisioner.clone()),
            allow_volume_expansion: s.allow_volume_expansion,
            creation_time: format_creation_time(s.metadata.creation_timestamp.as_ref()),
        })
        .collect();
    Ok(items)
}
