//! 资源列表：按 kind/namespace 拉取；入口统一，便于扩展新资源类型。
//!
//! 子模块按 Kubernetes 域划分：
//! - `cluster`    — Namespace, Node（含 CPU/mem/GPU 格式化工具）
//! - `workloads`  — Pod, Deployment, StatefulSet, DaemonSet, ReplicaSet
//! - `networking` — Service, Endpoints, EndpointSlice, Ingress, IngressClass, NetworkPolicy
//! - `config`     — ConfigMap, Secret, ServiceAccount
//! - `rbac`       — Role, RoleBinding, ClusterRole, ClusterRoleBinding
//! - `storage`    — PVC, PV, StorageClass
//! - `batch`      — Job, CronJob
//! - `policy`     — ResourceQuota, LimitRange, PriorityClass, HPA, PodDisruptionBudget

pub mod batch;
pub mod cluster;
pub mod config;
pub mod networking;
pub mod policy;
pub mod rbac;
pub mod storage;
pub mod workloads;

pub use batch::*;
pub use cluster::*;
pub use config::*;
pub use networking::*;
pub use policy::*;
pub use rbac::*;
pub use storage::*;
pub use workloads::*;

use chrono::{DateTime, Utc};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::api::ListParams;
use crate::kube::resource_graph::selector_to_string;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;

#[path = "../workload_pod_rollup.rs"]
mod workload_pod_rollup;

pub use workload_pod_rollup::{compute_workload_pod_rollup, WorkloadPodRollup};

// ── 共用工具 ───────────────────────────────────────────────────────────────

/// 返回 namespace，若未指定则使用 "default"。供 get/delete 层复用。
pub(crate) fn ns_or_default(ns: Option<&str>) -> &str {
    ns.unwrap_or("default")
}

// ── 时间格式化 ─────────────────────────────────────────────────────────────

/// 将时间差格式化为 kubectl 风格的中文相对时间（几秒前、几分钟前等）
pub(crate) fn format_age_zh(ts: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let d = now.signed_duration_since(*ts);
    let secs = d.num_seconds();
    if secs < 60 {
        format!("{}秒前", secs.max(0))
    } else if secs < 3600 {
        format!("{}分钟前", d.num_minutes())
    } else if secs < 86400 {
        format!("{}小时前", d.num_hours())
    } else if secs < 2592000 {
        format!("{}天前", d.num_days())
    } else if secs < 31536000 {
        format!("{}个月前", d.num_days() / 30)
    } else {
        format!("{}年前", d.num_days() / 365)
    }
}

/// 从 metadata.creation_timestamp 格式化为可读字符串：YYYY-MM-DD HH:MM (几秒前/几分钟前/...)
pub(crate) fn format_creation_time(ts: Option<&Time>) -> Option<String> {
    ts.map(|t| {
        let abs = t.0.format("%Y-%m-%d %H:%M").to_string();
        let age = format_age_zh(&t.0);
        format!("{} ({})", abs, age)
    })
}

// ── List params & selector ─────────────────────────────────────────────────

/// 将 LabelSelector 转为 K8s API 接受的字符串
pub(crate) fn label_selector_to_string(sel: Option<&LabelSelector>) -> Option<String> {
    let v = serde_json::to_value(sel?).ok()?;
    selector_to_string(&v)
}

/// 构建 ListParams，支持可选的 label selector（格式如 app=nginx 或 env in (prod,staging)）。
/// 此函数同时供 resource_dynamic 模块复用，避免重复定义。
pub(crate) fn build_list_params(label_selector: Option<&str>) -> ListParams {
    match label_selector {
        Some(s) if !s.trim().is_empty() => ListParams::default().labels(s.trim()),
        _ => ListParams::default(),
    }
}

// ── 简单资源的宏 & trait ───────────────────────────────────────────────────

/// 为"只有 name + namespace + creation_time"的命名空间级资源生成 list_* 函数。
/// 用法：`list_simple_namespaced!(list_fn_name, K8sType, ItemType)`
macro_rules! list_simple_namespaced {
    ($fn_name:ident, $k8s_type:ty, $item_type:ty) => {
        pub async fn $fn_name(
            client: &kube::Client,
            namespace: Option<&str>,
            label_selector: Option<&str>,
        ) -> Result<Vec<$item_type>, ResourceError> {
            let ns = namespace.unwrap_or("default");
            let api: kube::api::Api<$k8s_type> = kube::api::Api::namespaced(client.clone(), ns);
            let list = api
                .list(&build_list_params(label_selector))
                .await
                .map_err(ResourceError::Kube)?;
            Ok(list
                .items
                .into_iter()
                .map(|r| <$item_type>::from_meta(r.metadata, ns))
                .collect())
        }
    };
}
pub(crate) use list_simple_namespaced;

/// 为"只有 name + creation_time"的集群级资源生成 list_* 函数。
/// 用法：`list_simple_cluster!(list_fn_name, K8sType, ItemType)`
macro_rules! list_simple_cluster {
    ($fn_name:ident, $k8s_type:ty, $item_type:ty) => {
        pub async fn $fn_name(
            client: &kube::Client,
            label_selector: Option<&str>,
        ) -> Result<Vec<$item_type>, ResourceError> {
            let api: kube::api::Api<$k8s_type> = kube::api::Api::all(client.clone());
            let list = api
                .list(&build_list_params(label_selector))
                .await
                .map_err(ResourceError::Kube)?;
            Ok(list
                .items
                .into_iter()
                .map(|r| <$item_type>::from_meta_cluster(r.metadata))
                .collect())
        }
    };
}
pub(crate) use list_simple_cluster;

/// 命名空间级简单资源的公共构造 trait。
pub(crate) trait SimpleNamespacedItem: Sized {
    fn from_meta(meta: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta, default_ns: &str) -> Self;
}

/// 集群级简单资源的公共构造 trait。
pub(crate) trait SimpleClusterItem: Sized {
    fn from_meta_cluster(meta: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta) -> Self;
}

// ── 错误类型 ───────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error("kube: {0}")]
    Kube(#[from] kube::Error),
    #[error("unsupported kind: {0}")]
    UnsupportedKind(String),
    #[error("serialize: {0}")]
    Serialize(String),
}
