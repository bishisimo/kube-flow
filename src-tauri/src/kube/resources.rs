//! 资源列表：按 kind/namespace 拉取；入口统一，便于扩展新资源类型。

use chrono::{DateTime, Utc};
use crate::kube::related_targets::selector_to_string;
use crate::kube::resource_get::get_resource_value;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, Time};
use kube::api::{Api, ListParams};
use kube::Client;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::autoscaling::v2::HorizontalPodAutoscaler;
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::{
    ConfigMap, Endpoints, LimitRange, Namespace, Node, PersistentVolume, PersistentVolumeClaim,
    Pod, ResourceQuota, Secret, Service, ServiceAccount,
};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::api::networking::v1::{Ingress, IngressClass, NetworkPolicy};
use k8s_openapi::api::policy::v1::PodDisruptionBudget;
use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding, Role, RoleBinding};
use k8s_openapi::api::scheduling::v1::PriorityClass;
use k8s_openapi::api::storage::v1::StorageClass;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use serde::Serialize;

#[path = "workload_pod_rollup.rs"]
mod workload_pod_rollup;

pub use workload_pod_rollup::{compute_workload_pod_rollup, WorkloadPodRollup};

fn int_or_string_to_str(q: &IntOrString) -> String {
    match q {
        IntOrString::Int(i) => i.to_string(),
        IntOrString::String(s) => s.clone(),
    }
}

/// 将时间差格式化为 kubectl 风格的中文相对时间（几秒前、几分钟前等）
fn format_age_zh(ts: &DateTime<Utc>) -> String {
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

/// 将 LabelSelector 转为 K8s API 接受的字符串
pub(crate) fn label_selector_to_string(sel: Option<&LabelSelector>) -> Option<String> {
    let v = serde_json::to_value(sel?).ok()?;
    selector_to_string(&v)
}

/// 构建 ListParams，支持可选的 label selector（格式如 app=nginx 或 env in (prod,staging)）
fn build_list_params(label_selector: Option<&str>) -> ListParams {
    match label_selector {
        Some(s) if !s.trim().is_empty() => ListParams::default().labels(s.trim()),
        _ => ListParams::default(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NamespaceItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PodItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pod_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeploymentItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicas: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_selector: Option<String>,
    #[serde(default)]
    pub pod_rollup: WorkloadPodRollup,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_ip: Option<String>,
    /// 端口摘要；NodePort 时为 "port:nodePort/protocol"，如 "80:30080/TCP, 443:30443/TCP"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatefulSetItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicas: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_selector: Option<String>,
    #[serde(default)]
    pub pod_rollup: WorkloadPodRollup,
}

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

#[derive(Debug, Clone, Serialize)]
pub struct RoleItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

/// Subject 引用，仅 ServiceAccount 可下钻
#[derive(Debug, Clone, Serialize)]
pub struct SubjectRef {
    pub kind: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoleBindingItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_ref_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_ref_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjects: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjects_list: Option<Vec<SubjectRef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClusterRoleItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClusterRoleBindingItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_ref_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_ref_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjects: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjects_list: Option<Vec<SubjectRef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DaemonSetItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desired: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_selector: Option<String>,
    #[serde(default)]
    pub pod_rollup: WorkloadPodRollup,
}

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
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EndpointsItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subsets: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EndpointSliceItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoints: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplicaSetItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicas: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_selector: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CronJobItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_schedule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IngressItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IngressClassItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkPolicyItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceQuotaItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LimitRangeItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PriorityClassItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HorizontalPodAutoscalerItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicas: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PodDisruptionBudgetItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_available: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_unavailable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_disruptions: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

/// 列出集群 Namespace（集群级资源）。
pub async fn list_namespaces(
    client: &Client,
    label_selector: Option<&str>,
) -> Result<Vec<NamespaceItem>, ResourceError> {
    let api: Api<Namespace> = Api::all(client.clone());
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|n| NamespaceItem {
            name: n.metadata.name.unwrap_or_default(),
            creation_time: format_creation_time(n.metadata.creation_timestamp.as_ref()),
        })
        .collect();
    Ok(items)
}

/// 列出集群 Node（集群级资源）。
pub async fn list_nodes(
    client: &Client,
    label_selector: Option<&str>,
) -> Result<Vec<NodeItem>, ResourceError> {
    let api: Api<Node> = Api::all(client.clone());
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|n| {
            let status = n.status.as_ref().and_then(|s| {
                s.conditions.as_ref().and_then(|conds| {
                    conds.iter()
                        .find(|c| c.type_ == "Ready")
                        .map(|c| if c.status == "True" { "Ready".to_string() } else { "NotReady".to_string() })
                })
            });
            let internal_ip = n.status.as_ref().and_then(|s| {
                s.addresses.as_ref().and_then(|addrs| {
                    addrs.iter()
                        .find(|a| a.type_ == "InternalIP")
                        .map(|a| a.address.clone())
                })
            });
            NodeItem {
                name: n.metadata.name.unwrap_or_default(),
                status,
                internal_ip,
                creation_time: format_creation_time(n.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出 Workload（Deployment/StatefulSet/DaemonSet）管理的 Pods。
/// 从 workload spec.selector 提取 label selector 并调用 list_pods。
pub async fn list_pods_for_workload(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<Vec<PodItem>, ResourceError> {
    let supported = ["Deployment", "StatefulSet", "DaemonSet"];
    if !supported.contains(&kind) {
        return Err(ResourceError::UnsupportedKind(kind.to_string()));
    }
    let obj = get_resource_value(client, kind, name, namespace).await?;
    let spec = obj
        .get("spec")
        .and_then(|v| v.as_object())
        .ok_or_else(|| ResourceError::Serialize("workload spec not found".to_string()))?;
    let sel = spec
        .get("selector")
        .ok_or_else(|| ResourceError::Serialize("workload selector not found".to_string()))?;
    let ls = selector_to_string(sel)
        .ok_or_else(|| ResourceError::Serialize("invalid selector".to_string()))?;
    list_pods(client, namespace, Some(&ls)).await
}

/// 列出指定 namespace 的 Pods；None 表示 default。
pub async fn list_pods(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<PodItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<Pod> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|p| {
            let total = p
                .spec
                .as_ref()
                .map(|s| s.containers.len())
                .unwrap_or(0);
            let ready = p
                .status
                .as_ref()
                .and_then(|s| s.container_statuses.as_ref())
                .map(|statuses| statuses.iter().filter(|cs| cs.ready).count())
                .unwrap_or(0);
            PodItem {
                name: p.metadata.name.unwrap_or_default(),
                namespace: p.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                phase: p.status.as_ref().and_then(|s| s.phase.clone()),
                container_status: if total > 0 {
                    Some(format!("{}/{}", ready, total))
                } else {
                    None
                },
                pod_ip: p.status.as_ref().and_then(|s| s.pod_ip.clone()),
                node_name: p.spec.and_then(|s| s.node_name),
                creation_time: format_creation_time(p.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出使用指定 PVC 的 Pods（过滤 spec.volumes 中 persistentVolumeClaim.claimName 等于该 PVC 的 Pod）。
pub async fn list_pods_using_pvc(
    client: &Client,
    namespace: &str,
    pvc_name: &str,
) -> Result<Vec<PodItem>, ResourceError> {
    let api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let list = api.list(&ListParams::default()).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .filter(|p| {
            p.spec
                .as_ref()
                .and_then(|s| s.volumes.as_deref())
                .map(|vols| {
                    vols.iter().any(|v| {
                        v.persistent_volume_claim
                            .as_ref()
                            .map(|pvc| pvc.claim_name == pvc_name)
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        })
        .map(|p| {
            let total = p
                .spec
                .as_ref()
                .map(|s| s.containers.len())
                .unwrap_or(0);
            let ready = p
                .status
                .as_ref()
                .and_then(|s| s.container_statuses.as_ref())
                .map(|statuses| statuses.iter().filter(|cs| cs.ready).count())
                .unwrap_or(0);
            PodItem {
                name: p.metadata.name.unwrap_or_default(),
                namespace: p.metadata.namespace.unwrap_or_else(|| namespace.to_string()),
                phase: p.status.as_ref().and_then(|s| s.phase.clone()),
                container_status: if total > 0 {
                    Some(format!("{}/{}", ready, total))
                } else {
                    None
                },
                pod_ip: p.status.as_ref().and_then(|s| s.pod_ip.clone()),
                node_name: p.spec.and_then(|s| s.node_name),
                creation_time: format_creation_time(p.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出 namespace 内 Pod 对象（用于工作负载 Pod 态势聚合）。
async fn list_pod_objects(
    client: &Client,
    namespace: &str,
    label_selector: Option<&str>,
) -> Result<Vec<Pod>, ResourceError> {
    let api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    Ok(list.items)
}

/// 列出指定 namespace 的 Deployments。
pub async fn list_deployments(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<DeploymentItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<Deployment> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let pod_objects = list_pod_objects(client, ns, label_selector).await?;
    let items = list
        .items
        .into_iter()
        .map(|d| {
            let replicas = d.spec.as_ref().and_then(|s| s.replicas);
            let ready = d.status.as_ref().and_then(|s| s.ready_replicas);
            let label_selector = d.spec.as_ref().map(|s| &s.selector).and_then(|sel| label_selector_to_string(Some(sel)));
            let wns = d.metadata.namespace.as_deref().unwrap_or(ns);
            let pod_rollup = d
                .spec
                .as_ref()
                .map(|s| compute_workload_pod_rollup(&pod_objects, wns, &s.selector))
                .unwrap_or_default();
            DeploymentItem {
                name: d.metadata.name.unwrap_or_default(),
                namespace: d.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                replicas: replicas.or(Some(0)),
                ready: ready.or(Some(0)),
                creation_time: format_creation_time(d.metadata.creation_timestamp.as_ref()),
                label_selector,
                pod_rollup,
            }
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 Services。
pub async fn list_services(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<ServiceItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<Service> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|s| {
            let spec = s.spec.as_ref();
            let svc_type = spec.and_then(|sp| sp.type_.as_deref());
            let is_nodeport = svc_type == Some("NodePort");
            let ports_str = spec.and_then(|sp| {
                let ports = sp.ports.as_deref()?;
                if ports.is_empty() {
                    return None;
                }
                let parts: Vec<String> = ports
                    .iter()
                    .map(|p| {
                        let proto = p.protocol.as_deref().unwrap_or("TCP");
                        if is_nodeport {
                            if let Some(np) = p.node_port {
                                format!("{}:{}/{}", p.port, np, proto)
                            } else {
                                format!("{}/{}", p.port, proto)
                            }
                        } else {
                            format!("{}/{}", p.port, proto)
                        }
                    })
                    .collect();
                Some(parts.join(", "))
            });
            ServiceItem {
                name: s.metadata.name.unwrap_or_default(),
                namespace: s.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                service_type: spec.and_then(|sp| sp.type_.clone()),
                cluster_ip: s.spec.and_then(|spec| spec.cluster_ip),
                ports: ports_str,
                creation_time: format_creation_time(s.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出 selector 与 workload 匹配的 Services（Service.spec.selector 是 workload matchLabels 的子集）。
/// 用于 Deployment/StatefulSet/DaemonSet 的关联资源，展示暴露该 workload 的 Service。
pub async fn list_services_matching_workload_selector(
    client: &Client,
    namespace: &str,
    workload_selector: &serde_json::Value,
) -> Result<Vec<String>, ResourceError> {
    let workload_labels = workload_selector
        .get("matchLabels")
        .and_then(|v| v.as_object())
        .filter(|m| !m.is_empty());
    let Some(workload_labels) = workload_labels else {
        return Ok(Vec::new());
    };
    let api: Api<Service> = Api::namespaced(client.clone(), namespace);
    let list = api.list(&ListParams::default()).await.map_err(ResourceError::Kube)?;
    let mut names = Vec::new();
    for svc in list.items {
        let Some(spec) = svc.spec.as_ref() else { continue };
        let Some(sel) = spec.selector.as_ref() else { continue };
        if sel.is_empty() {
            continue;
        }
        let all_match = sel.iter().all(|(k, v)| {
            workload_labels
                .get(k)
                .and_then(|lv| lv.as_str())
                .map(|lv| lv == v)
                .unwrap_or(false)
        });
        if all_match {
            if let Some(name) = svc.metadata.name.as_deref() {
                names.push(name.to_string());
            }
        }
    }
    Ok(names)
}

/// 列出 selector 与 Pod labels 匹配的 Services（Service.spec.selector 是 Pod labels 的子集）。
/// 用于 Pod 的关联资源，展示当前 Pod 被哪些 Service 选中。
pub async fn list_services_matching_pod_labels(
    client: &Client,
    namespace: &str,
    pod_labels: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<String>, ResourceError> {
    if pod_labels.is_empty() {
        return Ok(Vec::new());
    }
    let api: Api<Service> = Api::namespaced(client.clone(), namespace);
    let list = api.list(&ListParams::default()).await.map_err(ResourceError::Kube)?;
    let mut names = Vec::new();
    for svc in list.items {
        let Some(spec) = svc.spec.as_ref() else { continue };
        let Some(sel) = spec.selector.as_ref() else { continue };
        if sel.is_empty() {
            continue;
        }
        let all_match = sel.iter().all(|(k, v)| {
            pod_labels
                .get(k)
                .and_then(|lv| lv.as_str())
                .map(|lv| lv == v)
                .unwrap_or(false)
        });
        if all_match {
            if let Some(name) = svc.metadata.name.as_deref() {
                names.push(name.to_string());
            }
        }
    }
    Ok(names)
}

/// 列出指定 namespace 的 StatefulSets。
pub async fn list_stateful_sets(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<StatefulSetItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<StatefulSet> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let pod_objects = list_pod_objects(client, ns, label_selector).await?;
    let items = list
        .items
        .into_iter()
        .map(|s| {
            let replicas = s.spec.as_ref().and_then(|sp| sp.replicas);
            let ready = s.status.and_then(|st| st.ready_replicas);
            let label_selector = s.spec.as_ref().map(|sp| &sp.selector).and_then(|sel| label_selector_to_string(Some(sel)));
            let wns = s.metadata.namespace.as_deref().unwrap_or(ns);
            let pod_rollup = s
                .spec
                .as_ref()
                .map(|sp| compute_workload_pod_rollup(&pod_objects, wns, &sp.selector))
                .unwrap_or_default();
            StatefulSetItem {
                name: s.metadata.name.unwrap_or_default(),
                namespace: s.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                replicas: replicas.or(Some(0)),
                ready: ready.or(Some(0)),
                creation_time: format_creation_time(s.metadata.creation_timestamp.as_ref()),
                label_selector,
                pod_rollup,
            }
        })
        .collect();
    Ok(items)
}

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

/// 列出指定 namespace 的 ServiceAccounts。
pub async fn list_service_accounts(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<ServiceAccountItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<ServiceAccount> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|s| ServiceAccountItem {
            name: s.metadata.name.unwrap_or_default(),
            namespace: s.metadata.namespace.unwrap_or_else(|| ns.to_string()),
            creation_time: format_creation_time(s.metadata.creation_timestamp.as_ref()),
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 Roles。
pub async fn list_roles(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<RoleItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<Role> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|r| RoleItem {
            name: r.metadata.name.unwrap_or_default(),
            namespace: r.metadata.namespace.unwrap_or_else(|| ns.to_string()),
            creation_time: format_creation_time(r.metadata.creation_timestamp.as_ref()),
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 RoleBindings。
pub async fn list_role_bindings(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<RoleBindingItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<RoleBinding> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|r| {
            let rr = &r.role_ref;
            let role_ref = Some(format!("{}/{}", rr.kind, rr.name));
            let role_ref_kind = Some(rr.kind.clone());
            let role_ref_name = Some(rr.name.clone());
            let subjects = r.subjects.as_ref().map(|s| s.len() as u32);
            let subjects_list: Option<Vec<SubjectRef>> = r.subjects.as_ref().map(|subs| {
                subs.iter()
                    .filter(|s| s.kind == "ServiceAccount")
                    .map(|s| SubjectRef {
                        kind: s.kind.clone(),
                        name: s.name.clone(),
                        namespace: s.namespace.clone(),
                    })
                    .collect()
            });
            let subjects_list = subjects_list.filter(|v| !v.is_empty());
            RoleBindingItem {
                name: r.metadata.name.unwrap_or_default(),
                namespace: r.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                role_ref,
                role_ref_kind,
                role_ref_name,
                subjects,
                subjects_list,
                creation_time: format_creation_time(r.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出集群级 ClusterRoles。
pub async fn list_cluster_roles(
    client: &Client,
    label_selector: Option<&str>,
) -> Result<Vec<ClusterRoleItem>, ResourceError> {
    let api: Api<ClusterRole> = Api::all(client.clone());
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|r| ClusterRoleItem {
            name: r.metadata.name.unwrap_or_default(),
            creation_time: format_creation_time(r.metadata.creation_timestamp.as_ref()),
        })
        .collect();
    Ok(items)
}

/// 列出集群级 ClusterRoleBindings。
pub async fn list_cluster_role_bindings(
    client: &Client,
    label_selector: Option<&str>,
) -> Result<Vec<ClusterRoleBindingItem>, ResourceError> {
    let api: Api<ClusterRoleBinding> = Api::all(client.clone());
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|r| {
            let rr = &r.role_ref;
            let role_ref = Some(format!("{}/{}", rr.kind, rr.name));
            let role_ref_kind = Some(rr.kind.clone());
            let role_ref_name = Some(rr.name.clone());
            let subjects = r.subjects.as_ref().map(|s| s.len() as u32);
            let subjects_list: Option<Vec<SubjectRef>> = r.subjects.as_ref().map(|subs| {
                subs.iter()
                    .filter(|s| s.kind == "ServiceAccount")
                    .map(|s| SubjectRef {
                        kind: s.kind.clone(),
                        name: s.name.clone(),
                        namespace: s.namespace.clone(),
                    })
                    .collect()
            });
            let subjects_list = subjects_list.filter(|v| !v.is_empty());
            ClusterRoleBindingItem {
                name: r.metadata.name.unwrap_or_default(),
                role_ref,
                role_ref_kind,
                role_ref_name,
                subjects,
                subjects_list,
                creation_time: format_creation_time(r.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 DaemonSets。
pub async fn list_daemon_sets(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<DaemonSetItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<DaemonSet> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let pod_objects = list_pod_objects(client, ns, label_selector).await?;
    let items = list
        .items
        .into_iter()
        .map(|d| {
            let desired = d.status.as_ref().map(|s| s.desired_number_scheduled);
            let ready = d.status.map(|s| s.number_ready);
            let label_selector = d.spec.as_ref().map(|s| &s.selector).and_then(|sel| label_selector_to_string(Some(sel)));
            let wns = d.metadata.namespace.as_deref().unwrap_or(ns);
            let pod_rollup = d
                .spec
                .as_ref()
                .map(|s| compute_workload_pod_rollup(&pod_objects, wns, &s.selector))
                .unwrap_or_default();
            DaemonSetItem {
                name: d.metadata.name.unwrap_or_default(),
                namespace: d.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                desired: desired.or(Some(0)),
                ready: ready.or(Some(0)),
                creation_time: format_creation_time(d.metadata.creation_timestamp.as_ref()),
                label_selector,
                pod_rollup,
            }
        })
        .collect();
    Ok(items)
}

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
            creation_time: format_creation_time(s.metadata.creation_timestamp.as_ref()),
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 Endpoints。
pub async fn list_endpoints(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<EndpointsItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<Endpoints> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|e| {
            let subsets = e.subsets.as_ref().map(|s| s.len() as u32);
            EndpointsItem {
                name: e.metadata.name.unwrap_or_default(),
                namespace: e.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                subsets,
                creation_time: format_creation_time(e.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 EndpointSlices。
pub async fn list_endpoint_slices(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<EndpointSliceItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<EndpointSlice> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|e| {
            let endpoints = Some(e.endpoints.len() as u32);
            EndpointSliceItem {
                name: e.metadata.name.unwrap_or_default(),
                namespace: e.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                address_type: Some(e.address_type.clone()),
                endpoints,
                creation_time: format_creation_time(e.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 ReplicaSets。
pub async fn list_replica_sets(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<ReplicaSetItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<ReplicaSet> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|r| {
            let replicas = r.spec.as_ref().and_then(|s| s.replicas);
            let ready = r.status.as_ref().and_then(|s| s.ready_replicas);
            let label_selector = r.spec.as_ref().map(|s| &s.selector).and_then(|sel| label_selector_to_string(Some(sel)));
            ReplicaSetItem {
                name: r.metadata.name.unwrap_or_default(),
                namespace: r.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                replicas: replicas.or(Some(0)),
                ready: ready.or(Some(0)),
                creation_time: format_creation_time(r.metadata.creation_timestamp.as_ref()),
                label_selector,
            }
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 Jobs。
pub async fn list_jobs(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<JobItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<Job> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|j| {
            let status = j.status.as_ref();
            let succeeded = status.and_then(|s| s.succeeded);
            let failed = status.and_then(|s| s.failed);
            let active = status.and_then(|s| s.active);
            let spec = j.spec.as_ref();
            let completions = spec.and_then(|s| s.completions);
            let completions_str = match (completions, succeeded, failed, active) {
                (Some(c), Some(s), Some(f), Some(a)) => Some(format!("{}/{}", s + f + a, c)),
                (Some(c), s, f, a) => Some(format!("{}/{}", s.unwrap_or(0) + f.unwrap_or(0) + a.unwrap_or(0), c)),
                _ => Some("1/1".to_string()),
            };
            let start = status.and_then(|s| s.start_time.as_ref());
            let completion = status.and_then(|s| s.completion_time.as_ref());
            let duration = match (start, completion) {
                (Some(st), Some(ct)) => {
                    let d = ct.0.signed_duration_since(st.0);
                    Some(format!("{}s", d.num_seconds().max(0)))
                }
                _ => None,
            };
            JobItem {
                name: j.metadata.name.unwrap_or_default(),
                namespace: j.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                completions: completions_str,
                duration,
                creation_time: format_creation_time(j.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 CronJobs。
pub async fn list_cron_jobs(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<CronJobItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<CronJob> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|c| {
            let schedule = c.spec.as_ref().map(|s| s.schedule.clone());
            let last_schedule = c.status.as_ref().and_then(|s| s.last_successful_time.as_ref().or(s.last_schedule_time.as_ref()));
            let last_schedule_str = last_schedule.and_then(|t| format_creation_time(Some(t)));
            CronJobItem {
                name: c.metadata.name.unwrap_or_default(),
                namespace: c.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                schedule,
                last_schedule: last_schedule_str,
                creation_time: format_creation_time(c.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 Ingresses。
pub async fn list_ingresses(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<IngressItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<Ingress> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|i| {
            let class = i.spec.as_ref().and_then(|s| s.ingress_class_name.clone());
            let hosts = i
                .spec
                .as_ref()
                .and_then(|s| s.rules.as_ref())
                .map(|r| r.iter().filter_map(|rule| rule.host.clone()).collect::<Vec<_>>().join(", "))
                .filter(|s| !s.is_empty());
            IngressItem {
                name: i.metadata.name.unwrap_or_default(),
                namespace: i.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                class,
                hosts,
                creation_time: format_creation_time(i.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出集群级 IngressClasses。
pub async fn list_ingress_classes(
    client: &Client,
    label_selector: Option<&str>,
) -> Result<Vec<IngressClassItem>, ResourceError> {
    let api: Api<IngressClass> = Api::all(client.clone());
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|i| IngressClassItem {
            name: i.metadata.name.unwrap_or_default(),
            controller: i.spec.and_then(|s| s.controller),
            creation_time: format_creation_time(i.metadata.creation_timestamp.as_ref()),
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 NetworkPolicies。
pub async fn list_network_policies(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<NetworkPolicyItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<NetworkPolicy> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|n| NetworkPolicyItem {
            name: n.metadata.name.unwrap_or_default(),
            namespace: n.metadata.namespace.unwrap_or_else(|| ns.to_string()),
            creation_time: format_creation_time(n.metadata.creation_timestamp.as_ref()),
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 ResourceQuotas。
pub async fn list_resource_quotas(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<ResourceQuotaItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<ResourceQuota> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|r| ResourceQuotaItem {
            name: r.metadata.name.unwrap_or_default(),
            namespace: r.metadata.namespace.unwrap_or_else(|| ns.to_string()),
            creation_time: format_creation_time(r.metadata.creation_timestamp.as_ref()),
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 LimitRanges。
pub async fn list_limit_ranges(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<LimitRangeItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<LimitRange> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|l| LimitRangeItem {
            name: l.metadata.name.unwrap_or_default(),
            namespace: l.metadata.namespace.unwrap_or_else(|| ns.to_string()),
            creation_time: format_creation_time(l.metadata.creation_timestamp.as_ref()),
        })
        .collect();
    Ok(items)
}

/// 列出集群级 PriorityClasses。
pub async fn list_priority_classes(
    client: &Client,
    label_selector: Option<&str>,
) -> Result<Vec<PriorityClassItem>, ResourceError> {
    let api: Api<PriorityClass> = Api::all(client.clone());
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|p| PriorityClassItem {
            name: p.metadata.name.unwrap_or_default(),
            value: Some(p.value),
            creation_time: format_creation_time(p.metadata.creation_timestamp.as_ref()),
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 HorizontalPodAutoscalers。
pub async fn list_horizontal_pod_autoscalers(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<HorizontalPodAutoscalerItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<HorizontalPodAutoscaler> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|h| {
            let status = h.status.as_ref();
            let current = status.and_then(|s| s.current_replicas);
            let desired = status.map(|s| s.desired_replicas);
            let replicas = match (current, desired) {
                (Some(c), Some(d)) => Some(format!("{}/{}", c, d)),
                (Some(c), _) => Some(format!("{}/{}", c, c)),
                _ => None,
            };
            let reference = h.spec.as_ref().map(|s| {
                let r = &s.scale_target_ref;
                format!("{}/{}", r.kind.as_str(), r.name.as_str())
            });
            HorizontalPodAutoscalerItem {
                name: h.metadata.name.unwrap_or_default(),
                namespace: h.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                reference,
                replicas,
                creation_time: format_creation_time(h.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 PodDisruptionBudgets。
pub async fn list_pod_disruption_budgets(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<PodDisruptionBudgetItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<PodDisruptionBudget> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|p| {
            let status = p.status.as_ref();
            let allowed = status.map(|s| s.disruptions_allowed);
            let min_avail = p.spec.as_ref().and_then(|s| s.min_available.as_ref()).map(int_or_string_to_str);
            let max_unavail = p.spec.as_ref().and_then(|s| s.max_unavailable.as_ref()).map(int_or_string_to_str);
            PodDisruptionBudgetItem {
                name: p.metadata.name.unwrap_or_default(),
                namespace: p.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                min_available: min_avail,
                max_unavailable: max_unavail,
                allowed_disruptions: allowed,
                creation_time: format_creation_time(p.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error("kube: {0}")]
    Kube(#[from] kube::Error),
    #[error("unsupported kind: {0}")]
    UnsupportedKind(String),
    #[error("serialize: {0}")]
    Serialize(String),
}
