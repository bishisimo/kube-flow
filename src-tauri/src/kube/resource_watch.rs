//! 资源 Watch：按 kind/namespace/label 建立 Watch 流，通过 Tauri 事件推送增量到前端。
//! 使用 kube::runtime::watcher 自动处理重连与 resourceVersion。

use crate::config::LogLevel;
use crate::debug_log;
use crate::env::EnvService;
use crate::kube::resources::{
    compute_workload_pod_rollup, format_cpu_total, format_creation_time, format_gpu, format_mem,
    label_selector_to_string, quantity_cpu_millis, quantity_mem_bytes, quantity_scalar_units, SubjectRef,
    ClusterRoleBindingItem, ClusterRoleItem, ConfigMapItem, DaemonSetItem, DeploymentItem,
    EndpointSliceItem, EndpointsItem, NamespaceItem, NodeItem, PersistentVolumeClaimItem,
    PersistentVolumeItem, PodItem, RoleBindingItem, RoleItem, SecretItem, ServiceAccountItem,
    ServiceItem, StatefulSetItem, StorageClassItem, WorkloadPodRollup,
};
use crate::kube::KubeClientStore;
use futures::stream::{self, StreamExt};
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, StatefulSet};
use k8s_openapi::api::core::v1::{
    ConfigMap, Endpoints, Namespace, Node, PersistentVolume, PersistentVolumeClaim, Pod, Secret,
    Service, ServiceAccount,
};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding, Role, RoleBinding};
use k8s_openapi::api::storage::v1::StorageClass;
use k8s_openapi::NamespaceResourceScope;
use kube::runtime::watcher::{watcher, Config as WatcherConfig};
use kube::{api::ResourceExt, Api, Client, Resource};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

const WATCH_EVENT: &str = "resource-watch-update";
const ALL_NAMESPACES_SENTINEL: &str = "__all__";

/// 按 env_id 存储当前活跃的 Watch 任务 AbortHandle
pub struct WatchStore {
    handles: Arc<RwLock<HashMap<String, tokio::task::AbortHandle>>>,
}

impl Default for WatchStore {
    fn default() -> Self {
        Self::new()
    }
}

impl WatchStore {
    pub fn new() -> Self {
        Self {
            handles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn stop(&self, env_id: &str) {
        let mut guard = self.handles.write().await;
        if let Some(h) = guard.remove(env_id) {
            h.abort();
        }
    }

    pub async fn insert(&self, env_id: String, handle: tokio::task::AbortHandle) {
        let mut guard = self.handles.write().await;
        if let Some(old) = guard.insert(env_id.clone(), handle) {
            old.abort();
        }
    }
}


fn pod_to_item(p: Pod, ns: &str) -> PodItem {
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
}

fn deployment_to_item(d: Deployment, ns: &str, pod_rollup: WorkloadPodRollup) -> DeploymentItem {
    let replicas = d.spec.as_ref().and_then(|s| s.replicas);
    let ready = d.status.as_ref().and_then(|s| s.ready_replicas);
    let label_selector = d.spec.as_ref().map(|s| &s.selector).and_then(|sel| label_selector_to_string(Some(sel)));
    DeploymentItem {
        name: d.metadata.name.unwrap_or_default(),
        namespace: d.metadata.namespace.unwrap_or_else(|| ns.to_string()),
        replicas: replicas.or(Some(0)),
        ready: ready.or(Some(0)),
        creation_time: format_creation_time(d.metadata.creation_timestamp.as_ref()),
        label_selector,
        pod_rollup,
    }
}

fn service_to_item(s: Service, ns: &str) -> ServiceItem {
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
}

fn namespace_to_item(n: Namespace) -> NamespaceItem {
    NamespaceItem {
        name: n.metadata.name.unwrap_or_default(),
        creation_time: format_creation_time(n.metadata.creation_timestamp.as_ref()),
    }
}

fn node_to_item(n: Node) -> NodeItem {
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
    let taint_count = n
        .spec
        .as_ref()
        .and_then(|spec| spec.taints.as_ref())
        .map(|taints| taints.len() as u32)
        .unwrap_or(0);
    let alloc_cpu = n
        .status
        .as_ref()
        .and_then(|s| s.allocatable.as_ref())
        .map(|m| quantity_cpu_millis(m.get("cpu")))
        .unwrap_or(0);
    let alloc_mem = n
        .status
        .as_ref()
        .and_then(|s| s.allocatable.as_ref())
        .map(|m| quantity_mem_bytes(m.get("memory")))
        .unwrap_or(0);
    let alloc_gpu = n
        .status
        .as_ref()
        .and_then(|s| s.allocatable.as_ref())
        .map(|m| {
            m.iter()
                .filter(|(name, _)| name.trim().to_lowercase().ends_with("/gpu"))
                .map(|(_, quantity)| quantity_scalar_units(Some(quantity)))
                .sum::<i64>()
        })
        .unwrap_or(0);
    NodeItem {
        name: n.metadata.name.unwrap_or_default(),
        status,
        taint_count: Some(taint_count),
        internal_ip,
        cpu_total: (alloc_cpu > 0).then(|| format_cpu_total(alloc_cpu)),
        memory_total: (alloc_mem > 0).then(|| format_mem(alloc_mem)),
        gpu_total: (alloc_gpu > 0).then(|| format_gpu(alloc_gpu)),
        cpu_requests: None,
        memory_requests: None,
        gpu_requests: None,
        creation_time: format_creation_time(n.metadata.creation_timestamp.as_ref()),
    }
}

fn statefulset_to_item(s: StatefulSet, ns: &str, pod_rollup: WorkloadPodRollup) -> StatefulSetItem {
    let replicas = s.spec.as_ref().and_then(|sp| sp.replicas);
    let ready = s.status.and_then(|st| st.ready_replicas);
    let label_selector = s.spec.as_ref().map(|sp| &sp.selector).and_then(|sel| label_selector_to_string(Some(sel)));
    StatefulSetItem {
        name: s.metadata.name.unwrap_or_default(),
        namespace: s.metadata.namespace.unwrap_or_else(|| ns.to_string()),
        replicas: replicas.or(Some(0)),
        ready: ready.or(Some(0)),
        creation_time: format_creation_time(s.metadata.creation_timestamp.as_ref()),
        label_selector,
        pod_rollup,
    }
}

fn configmap_to_item(c: ConfigMap, ns: &str) -> ConfigMapItem {
    let keys = c.data.as_ref().map(|d| d.len() as u32);
    ConfigMapItem {
        name: c.metadata.name.unwrap_or_default(),
        namespace: c.metadata.namespace.unwrap_or_else(|| ns.to_string()),
        keys,
        creation_time: format_creation_time(c.metadata.creation_timestamp.as_ref()),
    }
}

fn secret_to_item(s: Secret, ns: &str) -> SecretItem {
    let keys = s.data.as_ref().map(|d| d.len() as u32);
    SecretItem {
        name: s.metadata.name.unwrap_or_default(),
        namespace: s.metadata.namespace.unwrap_or_else(|| ns.to_string()),
        type_: s.type_.clone(),
        keys,
        creation_time: format_creation_time(s.metadata.creation_timestamp.as_ref()),
    }
}

fn serviceaccount_to_item(s: ServiceAccount, ns: &str) -> ServiceAccountItem {
    ServiceAccountItem {
        name: s.metadata.name.unwrap_or_default(),
        namespace: s.metadata.namespace.unwrap_or_else(|| ns.to_string()),
        creation_time: format_creation_time(s.metadata.creation_timestamp.as_ref()),
    }
}

fn role_to_item(r: Role, ns: &str) -> RoleItem {
    RoleItem {
        name: r.metadata.name.unwrap_or_default(),
        namespace: r.metadata.namespace.unwrap_or_else(|| ns.to_string()),
        creation_time: format_creation_time(r.metadata.creation_timestamp.as_ref()),
    }
}

fn rolebinding_to_item(r: RoleBinding, ns: &str) -> RoleBindingItem {
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
}

fn clusterrole_to_item(r: ClusterRole) -> ClusterRoleItem {
    ClusterRoleItem {
        name: r.metadata.name.unwrap_or_default(),
        creation_time: format_creation_time(r.metadata.creation_timestamp.as_ref()),
    }
}

fn clusterrolebinding_to_item(r: ClusterRoleBinding) -> ClusterRoleBindingItem {
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
}

fn daemonset_to_item(d: DaemonSet, ns: &str, pod_rollup: WorkloadPodRollup) -> DaemonSetItem {
    let desired = d.status.as_ref().map(|s| s.desired_number_scheduled);
    let ready = d.status.map(|s| s.number_ready);
    let label_selector = d.spec.as_ref().map(|s| &s.selector).and_then(|sel| label_selector_to_string(Some(sel)));
    DaemonSetItem {
        name: d.metadata.name.unwrap_or_default(),
        namespace: d.metadata.namespace.unwrap_or_else(|| ns.to_string()),
        desired: desired.or(Some(0)),
        ready: ready.or(Some(0)),
        creation_time: format_creation_time(d.metadata.creation_timestamp.as_ref()),
        label_selector,
        pod_rollup,
    }
}

fn pvc_to_item(p: PersistentVolumeClaim, ns: &str) -> PersistentVolumeClaimItem {
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
}

fn pv_to_item(p: PersistentVolume) -> PersistentVolumeItem {
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
}

fn storageclass_to_item(s: StorageClass) -> StorageClassItem {
    StorageClassItem {
        name: s.metadata.name.unwrap_or_default(),
        provisioner: Some(s.provisioner.clone()),
        allow_volume_expansion: s.allow_volume_expansion,
        creation_time: format_creation_time(s.metadata.creation_timestamp.as_ref()),
    }
}

fn endpoints_to_item(e: Endpoints, ns: &str) -> EndpointsItem {
    let subsets = e.subsets.as_ref().map(|s| s.len() as u32);
    EndpointsItem {
        name: e.metadata.name.unwrap_or_default(),
        namespace: e.metadata.namespace.unwrap_or_else(|| ns.to_string()),
        subsets,
        creation_time: format_creation_time(e.metadata.creation_timestamp.as_ref()),
    }
}

fn endpointslice_to_item(e: EndpointSlice, ns: &str) -> EndpointSliceItem {
    let endpoints = Some(e.endpoints.len() as u32);
    EndpointSliceItem {
        name: e.metadata.name.unwrap_or_default(),
        namespace: e.metadata.namespace.unwrap_or_else(|| ns.to_string()),
        address_type: Some(e.address_type.clone()),
        endpoints,
        creation_time: format_creation_time(e.metadata.creation_timestamp.as_ref()),
    }
}

/// 启动资源 Watch；若已有该 env 的 watch 则先停止。label_selector 参与 Watch 构建。
pub async fn start_watch(
    app: AppHandle,
    client_store: KubeClientStore,
    watch_store: Arc<WatchStore>,
    env_id: String,
    kind: String,
    namespace: Option<String>,
    label_selector: Option<String>,
    watch_token: Option<String>,
) -> Result<(), String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = client_store.get_or_build(&env).await.map_err(|e| {
        let msg = e.to_string();
        debug_log::log_list_err("watch/client", Some(&env_id), &msg, LogLevel::Error);
        msg
    })?;
    let ns = namespace
        .as_deref()
        .or_else(|| env.default_namespace())
        .map(String::from)
        .or_else(|| Some(ALL_NAMESPACES_SENTINEL.to_string()));
    let label_sel = label_selector.filter(|s| !s.trim().is_empty());
    let watch_token = watch_token.unwrap_or_default();

    let env_id_clone = env_id.clone();
    let handle = tokio::spawn(async move {
        run_watch(app, client, env_id_clone, kind, ns, label_sel, watch_token).await;
    });

    watch_store.insert(env_id, handle.abort_handle()).await;
    Ok(())
}

async fn run_watch(
    app: AppHandle,
    client: Client,
    env_id: String,
    kind: String,
    ns: Option<String>,
    label_selector: Option<String>,
    watch_token: String,
) {
    match kind.as_str() {
        "pods" => run_simple_watch_ns::<Pod, _>(app, client, env_id, ns, label_selector, watch_token, "pods", pod_to_item).await,
        "deployments" => run_watch_deployments(app, client, env_id, ns, label_selector, watch_token).await,
        "services" => run_simple_watch_ns::<Service, _>(app, client, env_id, ns, label_selector, watch_token, "services", service_to_item).await,
        "namespaces" => run_simple_watch_cluster::<Namespace, _>(app, client, env_id, label_selector, watch_token, "namespaces", namespace_to_item).await,
        "nodes" => run_simple_watch_cluster::<Node, _>(app, client, env_id, label_selector, watch_token, "nodes", node_to_item).await,
        "statefulsets" => run_watch_statefulsets(app, client, env_id, ns, label_selector, watch_token).await,
        "configmaps" => run_simple_watch_ns::<ConfigMap, _>(app, client, env_id, ns, label_selector, watch_token, "configmaps", configmap_to_item).await,
        "secrets" => run_simple_watch_ns::<Secret, _>(app, client, env_id, ns, label_selector, watch_token, "secrets", secret_to_item).await,
        "serviceaccounts" => run_simple_watch_ns::<ServiceAccount, _>(app, client, env_id, ns, label_selector, watch_token, "serviceaccounts", serviceaccount_to_item).await,
        "roles" => run_simple_watch_ns::<Role, _>(app, client, env_id, ns, label_selector, watch_token, "roles", role_to_item).await,
        "rolebindings" => run_simple_watch_ns::<RoleBinding, _>(app, client, env_id, ns, label_selector, watch_token, "rolebindings", rolebinding_to_item).await,
        "clusterroles" => run_simple_watch_cluster::<ClusterRole, _>(app, client, env_id, label_selector, watch_token, "clusterroles", clusterrole_to_item).await,
        "clusterrolebindings" => run_simple_watch_cluster::<ClusterRoleBinding, _>(app, client, env_id, label_selector, watch_token, "clusterrolebindings", clusterrolebinding_to_item).await,
        "daemonsets" => run_watch_daemonsets(app, client, env_id, ns, label_selector, watch_token).await,
        "persistentvolumeclaims" => run_simple_watch_ns::<PersistentVolumeClaim, _>(app, client, env_id, ns, label_selector, watch_token, "persistentvolumeclaims", pvc_to_item).await,
        "persistentvolumes" => run_simple_watch_cluster::<PersistentVolume, _>(app, client, env_id, label_selector, watch_token, "persistentvolumes", pv_to_item).await,
        "storageclasses" => run_simple_watch_cluster::<StorageClass, _>(app, client, env_id, label_selector, watch_token, "storageclasses", storageclass_to_item).await,
        "endpoints" => run_simple_watch_ns::<Endpoints, _>(app, client, env_id, ns, label_selector, watch_token, "endpoints", endpoints_to_item).await,
        "endpointslices" => run_simple_watch_ns::<EndpointSlice, _>(app, client, env_id, ns, label_selector, watch_token, "endpointslices", endpointslice_to_item).await,
        _ => {
            let _ = app.emit(
                WATCH_EVENT,
                serde_json::json!({
                    "envId": env_id,
                    "watchToken": watch_token,
                    "error": format!("unsupported watch kind: {}", kind)
                }),
            );
        }
    }
}

/// 通用命名空间范围 watcher：创建 Api<K>，监听事件，发射 Tauri 事件。
async fn run_simple_watch_ns<K, I>(
    app: AppHandle,
    client: Client,
    env_id: String,
    ns: Option<String>,
    label_selector: Option<String>,
    watch_token: String,
    kind_str: &'static str,
    to_item: impl Fn(K, &str) -> I,
)
where
    K: Resource<Scope = NamespaceResourceScope> + Clone + std::fmt::Debug + Send + Sync + serde::de::DeserializeOwned + 'static,
    K::DynamicType: Default,
    I: serde::Serialize + Clone,
{
    let all_namespaces = ns.as_deref() == Some(ALL_NAMESPACES_SENTINEL);
    let ns = ns.unwrap_or_else(|| "default".to_string());
    let api: Api<K> = if all_namespaces {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), &ns)
    };
    let mut config = WatcherConfig::default();
    if let Some(ref sel) = label_selector {
        config = config.labels(sel);
    }
    let mut stream = Box::pin(watcher(api, config));
    let mut items: HashMap<String, I> = HashMap::new();
    while let Some(ev) = stream.next().await {
        match ev {
            Ok(kube::runtime::watcher::Event::Applied(obj)) => {
                items.insert(obj.name_any(), to_item(obj, &ns));
            }
            Ok(kube::runtime::watcher::Event::Deleted(obj)) => {
                items.remove(&obj.name_any());
            }
            Ok(kube::runtime::watcher::Event::Restarted(objs)) => {
                items.clear();
                for obj in objs {
                    items.insert(obj.name_any(), to_item(obj, &ns));
                }
            }
            Err(e) => {
                let msg = e.to_string();
                debug_log::log_list_err(kind_str, Some(&env_id), &msg, LogLevel::Error);
                let _ = app.emit(WATCH_EVENT, serde_json::json!({
                    "envId": env_id,
                    "watchToken": watch_token,
                    "error": msg
                }));
                break;
            }
        }
        let payload = serde_json::json!({
            "envId": env_id,
            "watchToken": watch_token,
            "kind": kind_str,
            "items": items.values().cloned().collect::<Vec<_>>()
        });
        if app.emit(WATCH_EVENT, payload).is_err() {
            break;
        }
    }
}

/// 通用集群范围 watcher：使用 Api::all，监听事件，发射 Tauri 事件。
async fn run_simple_watch_cluster<K, I>(
    app: AppHandle,
    client: Client,
    env_id: String,
    label_selector: Option<String>,
    watch_token: String,
    kind_str: &'static str,
    to_item: impl Fn(K) -> I,
)
where
    K: Resource + Clone + std::fmt::Debug + Send + Sync + serde::de::DeserializeOwned + 'static,
    K::DynamicType: Default,
    I: serde::Serialize + Clone,
{
    let api: Api<K> = Api::all(client.clone());
    let mut config = WatcherConfig::default();
    if let Some(ref sel) = label_selector {
        config = config.labels(sel);
    }
    let mut stream = Box::pin(watcher(api, config));
    let mut items: HashMap<String, I> = HashMap::new();
    while let Some(ev) = stream.next().await {
        match ev {
            Ok(kube::runtime::watcher::Event::Applied(obj)) => {
                items.insert(obj.name_any(), to_item(obj));
            }
            Ok(kube::runtime::watcher::Event::Deleted(obj)) => {
                items.remove(&obj.name_any());
            }
            Ok(kube::runtime::watcher::Event::Restarted(objs)) => {
                items.clear();
                for obj in objs {
                    items.insert(obj.name_any(), to_item(obj));
                }
            }
            Err(e) => {
                let msg = e.to_string();
                debug_log::log_list_err(kind_str, Some(&env_id), &msg, LogLevel::Error);
                let _ = app.emit(WATCH_EVENT, serde_json::json!({
                    "envId": env_id,
                    "watchToken": watch_token,
                    "error": msg
                }));
                break;
            }
        }
        let payload = serde_json::json!({
            "envId": env_id,
            "watchToken": watch_token,
            "kind": kind_str,
            "items": items.values().cloned().collect::<Vec<_>>()
        });
        if app.emit(WATCH_EVENT, payload).is_err() {
            break;
        }
    }
}

fn workload_watch_cache_key<R: ResourceExt>(obj: &R, all_namespaces: bool, fallback_ns: &str) -> String {
    if all_namespaces {
        format!(
            "{}/{}",
            obj.namespace().unwrap_or_else(|| fallback_ns.to_string()),
            obj.name_any()
        )
    } else {
        obj.name_any()
    }
}

async fn run_watch_deployments(
    app: AppHandle,
    client: Client,
    env_id: String,
    ns: Option<String>,
    label_selector: Option<String>,
    watch_token: String,
) {
    let all_namespaces = ns.as_deref() == Some(ALL_NAMESPACES_SENTINEL);
    let ns = ns.unwrap_or_else(|| "default".to_string());
    let dep_api: Api<Deployment> = if all_namespaces {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), &ns)
    };
    let pod_api: Api<Pod> = if all_namespaces {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), &ns)
    };
    let mut dep_config = WatcherConfig::default();
    let mut pod_config = WatcherConfig::default();
    if let Some(ref sel) = label_selector {
        dep_config = dep_config.labels(sel);
        pod_config = pod_config.labels(sel);
    }

    enum DepOrPod {
        Dep(Result<kube::runtime::watcher::Event<Deployment>, kube::runtime::watcher::Error>),
        Pod(Result<kube::runtime::watcher::Event<Pod>, kube::runtime::watcher::Error>),
    }

    let dep_stream = watcher(dep_api, dep_config).map(DepOrPod::Dep);
    let pod_stream = watcher(pod_api, pod_config).map(DepOrPod::Pod);
    let mut merged = Box::pin(stream::select(dep_stream, pod_stream));

    let mut deps: HashMap<String, Deployment> = HashMap::new();
    let mut pods: HashMap<String, Pod> = HashMap::new();

    while let Some(branch) = merged.next().await {
        let mut fatal: Option<String> = None;
        match branch {
            DepOrPod::Dep(Ok(ev)) => match ev {
                kube::runtime::watcher::Event::Applied(obj) => {
                    let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                    deps.insert(key, obj);
                }
                kube::runtime::watcher::Event::Deleted(obj) => {
                    let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                    deps.remove(&key);
                }
                kube::runtime::watcher::Event::Restarted(list) => {
                    deps.clear();
                    for obj in list {
                        let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                        deps.insert(key, obj);
                    }
                }
            },
            DepOrPod::Pod(Ok(ev)) => match ev {
                kube::runtime::watcher::Event::Applied(obj) => {
                    let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                    pods.insert(key, obj);
                }
                kube::runtime::watcher::Event::Deleted(obj) => {
                    let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                    pods.remove(&key);
                }
                kube::runtime::watcher::Event::Restarted(list) => {
                    pods.clear();
                    for obj in list {
                        let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                        pods.insert(key, obj);
                    }
                }
            },
            DepOrPod::Dep(Err(e)) | DepOrPod::Pod(Err(e)) => {
                fatal = Some(e.to_string());
            }
        }

        if let Some(msg) = fatal {
            debug_log::log_list_err("deployments/watch", Some(&env_id), &msg, LogLevel::Error);
            let _ = app.emit(WATCH_EVENT, serde_json::json!({
                "envId": env_id,
                "watchToken": watch_token,
                "error": msg
            }));
            break;
        }

        let pod_vec: Vec<Pod> = pods.values().cloned().collect();
        let list: Vec<DeploymentItem> = deps
            .values()
            .map(|d| {
                let wns = d.metadata.namespace.as_deref().unwrap_or(&ns);
                let pod_rollup = d
                    .spec
                    .as_ref()
                    .map(|s| compute_workload_pod_rollup(&pod_vec, wns, &s.selector))
                    .unwrap_or_default();
                deployment_to_item(d.clone(), &ns, pod_rollup)
            })
            .collect();
        let payload = serde_json::json!({
            "envId": env_id,
            "watchToken": watch_token,
            "kind": "deployments",
            "items": list
        });
        if app.emit(WATCH_EVENT, payload).is_err() {
            break;
        }
    }
}

async fn run_watch_statefulsets(
    app: AppHandle,
    client: Client,
    env_id: String,
    ns: Option<String>,
    label_selector: Option<String>,
    watch_token: String,
) {
    let all_namespaces = ns.as_deref() == Some(ALL_NAMESPACES_SENTINEL);
    let ns = ns.unwrap_or_else(|| "default".to_string());
    let sts_api: Api<StatefulSet> = if all_namespaces {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), &ns)
    };
    let pod_api: Api<Pod> = if all_namespaces {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), &ns)
    };
    let mut sts_config = WatcherConfig::default();
    let mut pod_config = WatcherConfig::default();
    if let Some(ref sel) = label_selector {
        sts_config = sts_config.labels(sel);
        pod_config = pod_config.labels(sel);
    }

    enum StsOrPod {
        Sts(Result<kube::runtime::watcher::Event<StatefulSet>, kube::runtime::watcher::Error>),
        Pod(Result<kube::runtime::watcher::Event<Pod>, kube::runtime::watcher::Error>),
    }

    let sts_stream = watcher(sts_api, sts_config).map(StsOrPod::Sts);
    let pod_stream = watcher(pod_api, pod_config).map(StsOrPod::Pod);
    let mut merged = Box::pin(stream::select(sts_stream, pod_stream));

    let mut statefulsets: HashMap<String, StatefulSet> = HashMap::new();
    let mut pods: HashMap<String, Pod> = HashMap::new();

    while let Some(branch) = merged.next().await {
        let mut fatal: Option<String> = None;
        match branch {
            StsOrPod::Sts(Ok(ev)) => match ev {
                kube::runtime::watcher::Event::Applied(obj) => {
                    let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                    statefulsets.insert(key, obj);
                }
                kube::runtime::watcher::Event::Deleted(obj) => {
                    let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                    statefulsets.remove(&key);
                }
                kube::runtime::watcher::Event::Restarted(list) => {
                    statefulsets.clear();
                    for obj in list {
                        let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                        statefulsets.insert(key, obj);
                    }
                }
            },
            StsOrPod::Pod(Ok(ev)) => match ev {
                kube::runtime::watcher::Event::Applied(obj) => {
                    let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                    pods.insert(key, obj);
                }
                kube::runtime::watcher::Event::Deleted(obj) => {
                    let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                    pods.remove(&key);
                }
                kube::runtime::watcher::Event::Restarted(list) => {
                    pods.clear();
                    for obj in list {
                        let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                        pods.insert(key, obj);
                    }
                }
            },
            StsOrPod::Sts(Err(e)) | StsOrPod::Pod(Err(e)) => {
                fatal = Some(e.to_string());
            }
        }

        if let Some(msg) = fatal {
            debug_log::log_list_err("statefulsets/watch", Some(&env_id), &msg, LogLevel::Error);
            let _ = app.emit(WATCH_EVENT, serde_json::json!({
                "envId": env_id,
                "watchToken": watch_token,
                "error": msg
            }));
            break;
        }

        let pod_vec: Vec<Pod> = pods.values().cloned().collect();
        let list: Vec<StatefulSetItem> = statefulsets
            .values()
            .map(|s| {
                let wns = s.metadata.namespace.as_deref().unwrap_or(&ns);
                let pod_rollup = s
                    .spec
                    .as_ref()
                    .map(|sp| compute_workload_pod_rollup(&pod_vec, wns, &sp.selector))
                    .unwrap_or_default();
                statefulset_to_item(s.clone(), &ns, pod_rollup)
            })
            .collect();
        let payload = serde_json::json!({
            "envId": env_id,
            "watchToken": watch_token,
            "kind": "statefulsets",
            "items": list
        });
        if app.emit(WATCH_EVENT, payload).is_err() {
            break;
        }
    }
}

async fn run_watch_daemonsets(
    app: AppHandle,
    client: Client,
    env_id: String,
    ns: Option<String>,
    label_selector: Option<String>,
    watch_token: String,
) {
    let all_namespaces = ns.as_deref() == Some(ALL_NAMESPACES_SENTINEL);
    let ns = ns.unwrap_or_else(|| "default".to_string());
    let ds_api: Api<DaemonSet> = if all_namespaces {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), &ns)
    };
    let pod_api: Api<Pod> = if all_namespaces {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), &ns)
    };
    let mut ds_config = WatcherConfig::default();
    let mut pod_config = WatcherConfig::default();
    if let Some(ref sel) = label_selector {
        ds_config = ds_config.labels(sel);
        pod_config = pod_config.labels(sel);
    }

    enum DsOrPod {
        Ds(Result<kube::runtime::watcher::Event<DaemonSet>, kube::runtime::watcher::Error>),
        Pod(Result<kube::runtime::watcher::Event<Pod>, kube::runtime::watcher::Error>),
    }

    let ds_stream = watcher(ds_api, ds_config).map(DsOrPod::Ds);
    let pod_stream = watcher(pod_api, pod_config).map(DsOrPod::Pod);
    let mut merged = Box::pin(stream::select(ds_stream, pod_stream));

    let mut daemonsets: HashMap<String, DaemonSet> = HashMap::new();
    let mut pods: HashMap<String, Pod> = HashMap::new();

    while let Some(branch) = merged.next().await {
        let mut fatal: Option<String> = None;
        match branch {
            DsOrPod::Ds(Ok(ev)) => match ev {
                kube::runtime::watcher::Event::Applied(obj) => {
                    let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                    daemonsets.insert(key, obj);
                }
                kube::runtime::watcher::Event::Deleted(obj) => {
                    let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                    daemonsets.remove(&key);
                }
                kube::runtime::watcher::Event::Restarted(list) => {
                    daemonsets.clear();
                    for obj in list {
                        let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                        daemonsets.insert(key, obj);
                    }
                }
            },
            DsOrPod::Pod(Ok(ev)) => match ev {
                kube::runtime::watcher::Event::Applied(obj) => {
                    let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                    pods.insert(key, obj);
                }
                kube::runtime::watcher::Event::Deleted(obj) => {
                    let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                    pods.remove(&key);
                }
                kube::runtime::watcher::Event::Restarted(list) => {
                    pods.clear();
                    for obj in list {
                        let key = workload_watch_cache_key(&obj, all_namespaces, &ns);
                        pods.insert(key, obj);
                    }
                }
            },
            DsOrPod::Ds(Err(e)) | DsOrPod::Pod(Err(e)) => {
                fatal = Some(e.to_string());
            }
        }

        if let Some(msg) = fatal {
            debug_log::log_list_err("daemonsets/watch", Some(&env_id), &msg, LogLevel::Error);
            let _ = app.emit(WATCH_EVENT, serde_json::json!({
                "envId": env_id,
                "watchToken": watch_token,
                "error": msg
            }));
            break;
        }

        let pod_vec: Vec<Pod> = pods.values().cloned().collect();
        let list: Vec<DaemonSetItem> = daemonsets
            .values()
            .map(|d| {
                let wns = d.metadata.namespace.as_deref().unwrap_or(&ns);
                let pod_rollup = d
                    .spec
                    .as_ref()
                    .map(|s| compute_workload_pod_rollup(&pod_vec, wns, &s.selector))
                    .unwrap_or_default();
                daemonset_to_item(d.clone(), &ns, pod_rollup)
            })
            .collect();
        let payload = serde_json::json!({
            "envId": env_id,
            "watchToken": watch_token,
            "kind": "daemonsets",
            "items": list
        });
        if app.emit(WATCH_EVENT, payload).is_err() {
            break;
        }
    }
}
