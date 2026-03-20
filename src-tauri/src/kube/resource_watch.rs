//! 资源 Watch：按 kind/namespace/label 建立 Watch 流，通过 Tauri 事件推送增量到前端。
//! 使用 kube::runtime::watcher 自动处理重连与 resourceVersion。

use crate::config::LogLevel;
use crate::debug_log;
use crate::env::EnvService;
use crate::kube::resources::{
    format_creation_time, label_selector_to_string, SubjectRef,
    ClusterRoleBindingItem, ClusterRoleItem, ConfigMapItem, DaemonSetItem, DeploymentItem,
    EndpointSliceItem, EndpointsItem, NamespaceItem, NodeItem, PersistentVolumeClaimItem,
    PersistentVolumeItem, PodItem, RoleBindingItem, RoleItem, SecretItem, ServiceAccountItem,
    ServiceItem, StatefulSetItem, StorageClassItem,
};
use crate::kube::KubeClientStore;
use futures::StreamExt;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, StatefulSet};
use k8s_openapi::api::core::v1::{
    ConfigMap, Endpoints, Namespace, Node, PersistentVolume, PersistentVolumeClaim, Pod, Secret,
    Service, ServiceAccount,
};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding, Role, RoleBinding};
use k8s_openapi::api::storage::v1::StorageClass;
use kube::api::ResourceExt;
use kube::runtime::watcher::{watcher, Config as WatcherConfig};
use kube::{Api, Client};
use serde::Serialize;
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

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "items")]
#[allow(non_camel_case_types, dead_code)]
pub enum WatchPayload {
    namespaces(Vec<NamespaceItem>),
    nodes(Vec<NodeItem>),
    pods(Vec<PodItem>),
    deployments(Vec<DeploymentItem>),
    services(Vec<ServiceItem>),
    statefulsets(Vec<StatefulSetItem>),
    configmaps(Vec<ConfigMapItem>),
    secrets(Vec<SecretItem>),
    serviceaccounts(Vec<ServiceAccountItem>),
    roles(Vec<RoleItem>),
    rolebindings(Vec<RoleBindingItem>),
    clusterroles(Vec<ClusterRoleItem>),
    clusterrolebindings(Vec<ClusterRoleBindingItem>),
    daemonsets(Vec<DaemonSetItem>),
    persistentvolumeclaims(Vec<PersistentVolumeClaimItem>),
    persistentvolumes(Vec<PersistentVolumeItem>),
    storageclasses(Vec<StorageClassItem>),
    endpoints(Vec<EndpointsItem>),
    endpointslices(Vec<EndpointSliceItem>),
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

fn deployment_to_item(d: Deployment, ns: &str) -> DeploymentItem {
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
    NodeItem {
        name: n.metadata.name.unwrap_or_default(),
        status,
        internal_ip,
        creation_time: format_creation_time(n.metadata.creation_timestamp.as_ref()),
    }
}

#[allow(dead_code)]
fn statefulset_to_item(s: StatefulSet, ns: &str) -> StatefulSetItem {
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
    }
}

#[allow(dead_code)]
fn configmap_to_item(c: ConfigMap, ns: &str) -> ConfigMapItem {
    let keys = c.data.as_ref().map(|d| d.len() as u32);
    ConfigMapItem {
        name: c.metadata.name.unwrap_or_default(),
        namespace: c.metadata.namespace.unwrap_or_else(|| ns.to_string()),
        keys,
        creation_time: format_creation_time(c.metadata.creation_timestamp.as_ref()),
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
fn serviceaccount_to_item(s: ServiceAccount, ns: &str) -> ServiceAccountItem {
    ServiceAccountItem {
        name: s.metadata.name.unwrap_or_default(),
        namespace: s.metadata.namespace.unwrap_or_else(|| ns.to_string()),
        creation_time: format_creation_time(s.metadata.creation_timestamp.as_ref()),
    }
}

#[allow(dead_code)]
fn role_to_item(r: Role, ns: &str) -> RoleItem {
    RoleItem {
        name: r.metadata.name.unwrap_or_default(),
        namespace: r.metadata.namespace.unwrap_or_else(|| ns.to_string()),
        creation_time: format_creation_time(r.metadata.creation_timestamp.as_ref()),
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
fn clusterrole_to_item(r: ClusterRole) -> ClusterRoleItem {
    ClusterRoleItem {
        name: r.metadata.name.unwrap_or_default(),
        creation_time: format_creation_time(r.metadata.creation_timestamp.as_ref()),
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
fn daemonset_to_item(d: DaemonSet, ns: &str) -> DaemonSetItem {
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
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
fn storageclass_to_item(s: StorageClass) -> StorageClassItem {
    StorageClassItem {
        name: s.metadata.name.unwrap_or_default(),
        provisioner: Some(s.provisioner.clone()),
        creation_time: format_creation_time(s.metadata.creation_timestamp.as_ref()),
    }
}

#[allow(dead_code)]
fn endpoints_to_item(e: Endpoints, ns: &str) -> EndpointsItem {
    let subsets = e.subsets.as_ref().map(|s| s.len() as u32);
    EndpointsItem {
        name: e.metadata.name.unwrap_or_default(),
        namespace: e.metadata.namespace.unwrap_or_else(|| ns.to_string()),
        subsets,
        creation_time: format_creation_time(e.metadata.creation_timestamp.as_ref()),
    }
}

#[allow(dead_code)]
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
        "pods" => run_watch_pods(app, client, env_id, ns, label_selector, watch_token).await,
        "deployments" => run_watch_deployments(app, client, env_id, ns, label_selector, watch_token).await,
        "services" => run_watch_services(app, client, env_id, ns, label_selector, watch_token).await,
        "namespaces" => run_watch_namespaces(app, client, env_id, label_selector, watch_token).await,
        "nodes" => run_watch_nodes(app, client, env_id, label_selector, watch_token).await,
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

async fn run_watch_pods(
    app: AppHandle,
    client: Client,
    env_id: String,
    ns: Option<String>,
    label_selector: Option<String>,
    watch_token: String,
) {
    let all_namespaces = ns.as_deref() == Some(ALL_NAMESPACES_SENTINEL);
    let ns = ns.unwrap_or_else(|| "default".to_string());
    let api: Api<Pod> = if all_namespaces {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), &ns)
    };
    let mut config = WatcherConfig::default();
    if let Some(ref sel) = label_selector {
        config = config.labels(sel);
    }
    let mut stream = Box::pin(watcher(api, config));
    let mut items: HashMap<String, PodItem> = HashMap::new();

    while let Some(ev) = stream.next().await {
        match ev {
            Ok(kube::runtime::watcher::Event::Applied(obj)) => {
                let key = obj.name_any();
                items.insert(key, pod_to_item(obj, &ns));
            }
            Ok(kube::runtime::watcher::Event::Deleted(obj)) => {
                items.remove(&obj.name_any());
            }
            Ok(kube::runtime::watcher::Event::Restarted(objs)) => {
                items.clear();
                for obj in objs {
                    let key = obj.name_any();
                    items.insert(key, pod_to_item(obj, &ns));
                }
            }
            Err(e) => {
                let msg = e.to_string();
                debug_log::log_list_err("pods/watch", Some(&env_id), &msg, LogLevel::Error);
                let _ = app.emit(WATCH_EVENT, serde_json::json!({
                    "envId": env_id,
                    "watchToken": watch_token,
                    "error": msg
                }));
                break;
            }
        }
        let list: Vec<PodItem> = items.values().cloned().collect();
        let payload = serde_json::json!({
            "envId": env_id,
            "watchToken": watch_token,
            "kind": "pods",
            "items": list
        });
        if app.emit(WATCH_EVENT, payload).is_err() {
            break;
        }
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
    let api: Api<Deployment> = if all_namespaces {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), &ns)
    };
    let mut config = WatcherConfig::default();
    if let Some(ref sel) = label_selector {
        config = config.labels(sel);
    }
    let mut stream = Box::pin(watcher(api, config));
    let mut items: HashMap<String, DeploymentItem> = HashMap::new();

    while let Some(ev) = stream.next().await {
        match ev {
            Ok(kube::runtime::watcher::Event::Applied(obj)) => {
                let key = obj.name_any();
                items.insert(key, deployment_to_item(obj, &ns));
            }
            Ok(kube::runtime::watcher::Event::Deleted(obj)) => {
                items.remove(&obj.name_any());
            }
            Ok(kube::runtime::watcher::Event::Restarted(objs)) => {
                items.clear();
                for obj in objs {
                    let key = obj.name_any();
                    items.insert(key, deployment_to_item(obj, &ns));
                }
            }
            Err(e) => {
                let msg = e.to_string();
                debug_log::log_list_err("deployments/watch", Some(&env_id), &msg, LogLevel::Error);
                let _ = app.emit(WATCH_EVENT, serde_json::json!({
                    "envId": env_id,
                    "watchToken": watch_token,
                    "error": msg
                }));
                break;
            }
        }
        let list: Vec<DeploymentItem> = items.values().cloned().collect();
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

async fn run_watch_services(
    app: AppHandle,
    client: Client,
    env_id: String,
    ns: Option<String>,
    label_selector: Option<String>,
    watch_token: String,
) {
    let all_namespaces = ns.as_deref() == Some(ALL_NAMESPACES_SENTINEL);
    let ns = ns.unwrap_or_else(|| "default".to_string());
    let api: Api<Service> = if all_namespaces {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), &ns)
    };
    let mut config = WatcherConfig::default();
    if let Some(ref sel) = label_selector {
        config = config.labels(sel);
    }
    let mut stream = Box::pin(watcher(api, config));
    let mut items: HashMap<String, ServiceItem> = HashMap::new();

    while let Some(ev) = stream.next().await {
        match ev {
            Ok(kube::runtime::watcher::Event::Applied(obj)) => {
                let key = obj.name_any();
                items.insert(key, service_to_item(obj, &ns));
            }
            Ok(kube::runtime::watcher::Event::Deleted(obj)) => {
                items.remove(&obj.name_any());
            }
            Ok(kube::runtime::watcher::Event::Restarted(objs)) => {
                items.clear();
                for obj in objs {
                    let key = obj.name_any();
                    items.insert(key, service_to_item(obj, &ns));
                }
            }
            Err(e) => {
                let msg = e.to_string();
                debug_log::log_list_err("services/watch", Some(&env_id), &msg, LogLevel::Error);
                let _ = app.emit(WATCH_EVENT, serde_json::json!({
                    "envId": env_id,
                    "watchToken": watch_token,
                    "error": msg
                }));
                break;
            }
        }
        let list: Vec<ServiceItem> = items.values().cloned().collect();
        let payload = serde_json::json!({
            "envId": env_id,
            "watchToken": watch_token,
            "kind": "services",
            "items": list
        });
        if app.emit(WATCH_EVENT, payload).is_err() {
            break;
        }
    }
}

async fn run_watch_namespaces(
    app: AppHandle,
    client: Client,
    env_id: String,
    label_selector: Option<String>,
    watch_token: String,
) {
    let api: Api<Namespace> = Api::all(client.clone());
    let mut config = WatcherConfig::default();
    if let Some(ref sel) = label_selector {
        config = config.labels(sel);
    }
    let mut stream = Box::pin(watcher(api, config));
    let mut items: HashMap<String, NamespaceItem> = HashMap::new();

    while let Some(ev) = stream.next().await {
        match ev {
            Ok(kube::runtime::watcher::Event::Applied(obj)) => {
                let key = obj.metadata.name.clone().unwrap_or_default();
                items.insert(key.clone(), namespace_to_item(obj));
            }
            Ok(kube::runtime::watcher::Event::Deleted(obj)) => {
                items.remove(&obj.metadata.name.unwrap_or_default());
            }
            Ok(kube::runtime::watcher::Event::Restarted(objs)) => {
                items.clear();
                for obj in objs {
                    let key = obj.metadata.name.clone().unwrap_or_default();
                    items.insert(key, namespace_to_item(obj));
                }
            }
            Err(e) => {
                let msg = e.to_string();
                debug_log::log_list_err("namespaces/watch", Some(&env_id), &msg, LogLevel::Error);
                let _ = app.emit(WATCH_EVENT, serde_json::json!({
                    "envId": env_id,
                    "watchToken": watch_token,
                    "error": msg
                }));
                break;
            }
        }
        let list: Vec<NamespaceItem> = items.values().cloned().collect();
        let payload = serde_json::json!({
            "envId": env_id,
            "watchToken": watch_token,
            "kind": "namespaces",
            "items": list
        });
        if app.emit(WATCH_EVENT, payload).is_err() {
            break;
        }
    }
}

async fn run_watch_nodes(
    app: AppHandle,
    client: Client,
    env_id: String,
    label_selector: Option<String>,
    watch_token: String,
) {
    let api: Api<Node> = Api::all(client.clone());
    let mut config = WatcherConfig::default();
    if let Some(ref sel) = label_selector {
        config = config.labels(sel);
    }
    let mut stream = Box::pin(watcher(api, config));
    let mut items: HashMap<String, NodeItem> = HashMap::new();

    while let Some(ev) = stream.next().await {
        match ev {
            Ok(kube::runtime::watcher::Event::Applied(obj)) => {
                let key = obj.name_any();
                items.insert(key, node_to_item(obj));
            }
            Ok(kube::runtime::watcher::Event::Deleted(obj)) => {
                items.remove(&obj.name_any());
            }
            Ok(kube::runtime::watcher::Event::Restarted(objs)) => {
                items.clear();
                for obj in objs {
                    let key = obj.name_any();
                    items.insert(key, node_to_item(obj));
                }
            }
            Err(e) => {
                let msg = e.to_string();
                debug_log::log_list_err("nodes/watch", Some(&env_id), &msg, LogLevel::Error);
                let _ = app.emit(WATCH_EVENT, serde_json::json!({
                    "envId": env_id,
                    "watchToken": watch_token,
                    "error": msg
                }));
                break;
            }
        }
        let list: Vec<NodeItem> = items.values().cloned().collect();
        let payload = serde_json::json!({
            "envId": env_id,
            "watchToken": watch_token,
            "kind": "nodes",
            "items": list
        });
        if app.emit(WATCH_EVENT, payload).is_err() {
            break;
        }
    }
}
