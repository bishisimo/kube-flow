//! 工作负载资源：Pod, Deployment, StatefulSet, DaemonSet, ReplicaSet。

use super::{
    build_list_params, compute_workload_pod_rollup, format_creation_time, label_selector_to_string,
    ResourceError, WorkloadPodRollup,
};
use crate::kube::resource_graph::selector_to_string;
use crate::kube::resource_get::get_resource_value;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::core::v1::Pod;
use kube::api::Api;
use kube::Client;
use serde::Serialize;

// ── 数据结构 ───────────────────────────────────────────────────────────────

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

// ── list 函数 ──────────────────────────────────────────────────────────────

fn map_pod(p: Pod, ns: &str) -> PodItem {
    let total = p.spec.as_ref().map(|s| s.containers.len()).unwrap_or(0);
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
        container_status: if total > 0 { Some(format!("{}/{}", ready, total)) } else { None },
        pod_ip: p.status.as_ref().and_then(|s| s.pod_ip.clone()),
        node_name: p.spec.and_then(|s| s.node_name),
        creation_time: format_creation_time(p.metadata.creation_timestamp.as_ref()),
    }
}

/// 列出 Workload（Deployment/StatefulSet/DaemonSet）管理的 Pods。
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
    Ok(list.items.into_iter().map(|p| map_pod(p, ns)).collect())
}

/// 列出 namespace 内 Pod 对象（用于工作负载 Pod 态势聚合）。
pub(crate) async fn list_pod_objects(
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
