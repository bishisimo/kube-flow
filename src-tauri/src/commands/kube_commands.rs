//! K8s 资源与客户端 Tauri 命令：需传入 env 或 env_id，从 store 取 Client。

use crate::config::{app_settings_config_path, AppSettingsConfig, LogLevel, ResourceDeployStrategy};
use crate::debug_log;
use crate::env::EnvService;
use crate::kube::{
    apply_resource_yaml, delete_dynamic_resource, delete_resource, deploy_resource_yaml, describe_dynamic_resource,
    describe_resource, get_dynamic_resource_yaml, get_pod_container_names,
    get_pod_logs, get_related_targets, get_resource_topology, get_resource_yaml, list_crd_instances,
    patch_container_images,
    run_pod_exec, run_pod_log_stream, start_watch, KubeClientStore, PodExecStore, PodLogStreamStore,
    DynamicCrdInstanceItem, RelatedTarget, ResourceAliasCacheStore, ResourceAliasRefreshResult,
    ResolvedAliasTarget, ResourceTopology, WatchStore,
    list_cluster_role_bindings, list_cluster_roles, list_config_maps, list_cron_jobs,
    list_daemon_sets, list_deployments, list_endpoint_slices, list_endpoints,
    list_horizontal_pod_autoscalers, list_ingress_classes, list_ingresses, list_jobs,
    list_limit_ranges, list_namespaces, list_network_policies, list_nodes,
    list_persistent_volume_claims, list_persistent_volumes, list_pod_disruption_budgets,
    list_pods, list_pods_for_workload, list_priority_classes, list_replica_sets, list_resource_quotas,
    list_role_bindings, list_roles, list_secrets, list_service_accounts, list_services,
    list_stateful_sets, list_storage_classes,
    ClusterRoleBindingItem, ClusterRoleItem, ConfigMapItem, CronJobItem, DaemonSetItem,
    DeploymentItem, EndpointSliceItem, EndpointsItem, HorizontalPodAutoscalerItem,
    IngressClassItem, IngressItem, JobItem, LimitRangeItem, NamespaceItem, NetworkPolicyItem,
    NodeItem, PersistentVolumeClaimItem, PersistentVolumeItem, PodDisruptionBudgetItem,
    PodItem, PriorityClassItem, ReplicaSetItem, ResourceQuotaItem, RoleBindingItem, RoleItem,
    SecretItem, ServiceAccountItem, ServiceItem, StatefulSetItem, StorageClassItem,
};
use std::sync::Arc;
use tauri::{AppHandle, State};
use uuid::Uuid;

async fn with_list_log<T, E>(
    resource: &str,
    env_id: &str,
    future: impl std::future::Future<Output = Result<Vec<T>, E>>,
) -> Result<Vec<T>, String>
where
    E: std::fmt::Display,
{
    match future.await {
        Ok(items) => {
            debug_log::log_list_ok(resource, Some(env_id), items.len() as u32, LogLevel::Info);
            Ok(items)
        }
        Err(e) => {
            debug_log::log_list_err(resource, Some(env_id), &e.to_string(), LogLevel::Error);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn kube_list_namespaces(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> Result<Vec<NamespaceItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let sel = label_selector.as_deref();
    with_list_log("Namespace", &env_id, list_namespaces(&client, sel)).await
}

#[tauri::command]
pub async fn kube_list_nodes(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> Result<Vec<NodeItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let sel = label_selector.as_deref();
    let settings_path = app_settings_config_path().ok_or_else(|| "app data dir not available".to_string())?;
    let app_settings = AppSettingsConfig::load(&settings_path).map_err(|e| e.to_string())?;
    let gpu_resource_names = app_settings.gpu_resource_names();
    with_list_log("Node", &env_id, list_nodes(&client, sel, &gpu_resource_names)).await
}

#[tauri::command]
pub async fn kube_list_pods(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<PodItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("Pod", &env_id, list_pods(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_pods_for_workload(
    store: State<'_, KubeClientStore>,
    env_id: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<Vec<PodItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref();
    list_pods_for_workload(&client, &kind, &name, ns).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kube_list_deployments(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<DeploymentItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("Deployment", &env_id, list_deployments(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_services(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<ServiceItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("Service", &env_id, list_services(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_stateful_sets(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<StatefulSetItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("StatefulSet", &env_id, list_stateful_sets(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_config_maps(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<ConfigMapItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("ConfigMap", &env_id, list_config_maps(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_secrets(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<SecretItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("Secret", &env_id, list_secrets(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_service_accounts(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<ServiceAccountItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("ServiceAccount", &env_id, list_service_accounts(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_roles(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<RoleItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("Role", &env_id, list_roles(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_role_bindings(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<RoleBindingItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("RoleBinding", &env_id, list_role_bindings(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_cluster_roles(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> Result<Vec<ClusterRoleItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let sel = label_selector.as_deref();
    with_list_log("ClusterRole", &env_id, list_cluster_roles(&client, sel)).await
}

#[tauri::command]
pub async fn kube_list_cluster_role_bindings(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> Result<Vec<ClusterRoleBindingItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let sel = label_selector.as_deref();
    with_list_log("ClusterRoleBinding", &env_id, list_cluster_role_bindings(&client, sel)).await
}

#[tauri::command]
pub async fn kube_list_daemon_sets(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<DaemonSetItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("DaemonSet", &env_id, list_daemon_sets(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_persistent_volume_claims(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<PersistentVolumeClaimItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("PersistentVolumeClaim", &env_id, list_persistent_volume_claims(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_persistent_volumes(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> Result<Vec<PersistentVolumeItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let sel = label_selector.as_deref();
    with_list_log("PersistentVolume", &env_id, list_persistent_volumes(&client, sel)).await
}

#[tauri::command]
pub async fn kube_list_storage_classes(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> Result<Vec<StorageClassItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let sel = label_selector.as_deref();
    with_list_log("StorageClass", &env_id, list_storage_classes(&client, sel)).await
}

#[tauri::command]
pub async fn kube_list_endpoints(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<EndpointsItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("Endpoints", &env_id, list_endpoints(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_endpoint_slices(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<EndpointSliceItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("EndpointSlice", &env_id, list_endpoint_slices(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_replica_sets(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<ReplicaSetItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("ReplicaSet", &env_id, list_replica_sets(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_jobs(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<JobItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("Job", &env_id, list_jobs(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_cron_jobs(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<CronJobItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("CronJob", &env_id, list_cron_jobs(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_ingresses(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<IngressItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("Ingress", &env_id, list_ingresses(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_ingress_classes(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> Result<Vec<IngressClassItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let sel = label_selector.as_deref();
    with_list_log("IngressClass", &env_id, list_ingress_classes(&client, sel)).await
}

#[tauri::command]
pub async fn kube_list_network_policies(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<NetworkPolicyItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("NetworkPolicy", &env_id, list_network_policies(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_resource_quotas(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<ResourceQuotaItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("ResourceQuota", &env_id, list_resource_quotas(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_limit_ranges(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<LimitRangeItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("LimitRange", &env_id, list_limit_ranges(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_priority_classes(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> Result<Vec<PriorityClassItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let sel = label_selector.as_deref();
    with_list_log("PriorityClass", &env_id, list_priority_classes(&client, sel)).await
}

#[tauri::command]
pub async fn kube_list_horizontal_pod_autoscalers(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<HorizontalPodAutoscalerItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("HorizontalPodAutoscaler", &env_id, list_horizontal_pod_autoscalers(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_list_pod_disruption_budgets(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<PodDisruptionBudgetItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    let sel = label_selector.as_deref();
    with_list_log("PodDisruptionBudget", &env_id, list_pod_disruption_budgets(&client, ns, sel)).await
}

#[tauri::command]
pub async fn kube_describe_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<crate::kube::DescribeResult, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref();
    describe_resource(&client, &kind, &name, ns).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kube_get_related_targets(
    store: State<'_, KubeClientStore>,
    env_id: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<Vec<RelatedTarget>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref();
    get_related_targets(&client, &kind, &name, ns).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kube_get_resource_topology(
    store: State<'_, KubeClientStore>,
    env_id: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<ResourceTopology, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref();
    get_resource_topology(&client, &kind, &name, ns).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kube_get_pod_containers(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: String,
    pod_name: String,
) -> Result<Vec<String>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    get_pod_container_names(&client, &namespace, &pod_name).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn kube_pod_log_stream_start(
    app: AppHandle,
    store: State<'_, KubeClientStore>,
    stream_store: State<'_, Arc<PodLogStreamStore>>,
    env_id: String,
    namespace: String,
    pod_name: String,
    container: Option<String>,
    tail_lines: Option<i64>,
    since_seconds: Option<i64>,
    timestamps: Option<bool>,
    previous: Option<bool>,
) -> Result<String, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let stream_id = Uuid::new_v4().to_string();
    let stream_id_clone = stream_id.clone();
    let app_handle = app.clone();
    let task = tokio::spawn(async move {
        run_pod_log_stream(
            app_handle,
            stream_id_clone,
            client,
            namespace,
            pod_name,
            container,
            tail_lines,
            since_seconds,
            timestamps.unwrap_or(false),
            previous.unwrap_or(false),
        )
        .await
    });
    stream_store.insert(stream_id.clone(), task.abort_handle()).await;
    Ok(stream_id)
}

#[tauri::command]
pub async fn kube_pod_log_stream_stop(
    stream_store: State<'_, Arc<PodLogStreamStore>>,
    stream_id: String,
) -> Result<(), String> {
    stream_store.stop(&stream_id).await;
    Ok(())
}

#[tauri::command]
pub async fn kube_pod_exec_start(
    app: AppHandle,
    store: State<'_, KubeClientStore>,
    exec_store: State<'_, Arc<PodExecStore>>,
    env_id: String,
    namespace: String,
    pod_name: String,
    container: Option<String>,
) -> Result<String, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let stream_id = Uuid::new_v4().to_string();
    let stream_id_clone = stream_id.clone();
    let app_handle = app.clone();
    let exec_store_clone = exec_store.inner().clone();
    tokio::spawn(async move {
        run_pod_exec(
            app_handle,
            stream_id_clone,
            client,
            namespace,
            pod_name,
            container,
            exec_store_clone,
        )
        .await
    });
    Ok(stream_id)
}

#[tauri::command]
pub async fn kube_pod_exec_stdin(
    exec_store: State<'_, Arc<PodExecStore>>,
    stream_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    exec_store.send_stdin(&stream_id, data).await
}

#[tauri::command]
pub async fn kube_pod_exec_resize(
    exec_store: State<'_, Arc<PodExecStore>>,
    stream_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    exec_store.send_resize(&stream_id, cols, rows).await
}

#[tauri::command]
pub async fn kube_pod_exec_stop(
    exec_store: State<'_, Arc<PodExecStore>>,
    stream_id: String,
) -> Result<(), String> {
    exec_store.stop(&stream_id).await;
    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn kube_pod_logs(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: String,
    pod_name: String,
    container: Option<String>,
    tail_lines: Option<i64>,
    since_seconds: Option<i64>,
    timestamps: Option<bool>,
    previous: Option<bool>,
) -> Result<String, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    get_pod_logs(
        &client,
        &namespace,
        &pod_name,
        container.as_deref(),
        tail_lines,
        since_seconds,
        timestamps.unwrap_or(false),
        previous.unwrap_or(false),
    )
    .await
}

#[tauri::command]
pub async fn kube_get_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<String, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref();
    get_resource_yaml(&client, &kind, &name, ns).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kube_list_crd_instances(
    store: State<'_, KubeClientStore>,
    env_id: String,
    api_version: String,
    kind: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> Result<Vec<DynamicCrdInstanceItem>, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref();
    let sel = label_selector.as_deref();
    with_list_log(
        &format!("CRD:{kind}"),
        &env_id,
        list_crd_instances(&client, &api_version, &kind, ns, sel),
    )
    .await
}

#[tauri::command]
pub async fn kube_get_dynamic_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    api_version: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<String, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref();
    get_dynamic_resource_yaml(&client, &api_version, &kind, &name, ns).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kube_describe_dynamic_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    api_version: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<crate::kube::DescribeResult, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref();
    describe_dynamic_resource(&client, &api_version, &kind, &name, ns)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kube_delete_dynamic_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    api_version: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<(), String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref();
    delete_dynamic_resource(&client, &api_version, &kind, &name, ns)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kube_delete_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> Result<(), String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref();
    delete_resource(&client, &kind, &name, ns).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kube_apply_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    yaml: String,
) -> Result<(), String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    apply_resource_yaml(&client, &yaml).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kube_deploy_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    yaml: String,
    strategy: String,
) -> Result<(), String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let strategy = ResourceDeployStrategy::from_str(&strategy);
    deploy_resource_yaml(&client, &yaml, strategy)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kube_patch_container_images(
    store: State<'_, KubeClientStore>,
    env_id: String,
    kind: String,
    name: String,
    namespace: Option<String>,
    patches: Vec<crate::kube::ContainerImagePatch>,
) -> Result<(), String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    let ns = namespace.as_deref();
    patch_container_images(&client, &kind, &name, ns, &patches).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kube_start_watch(
    app: AppHandle,
    store: State<'_, KubeClientStore>,
    watch_store: State<'_, Arc<WatchStore>>,
    env_id: String,
    kind: String,
    namespace: Option<String>,
    label_selector: Option<String>,
    watch_token: Option<String>,
) -> Result<(), String> {
    start_watch(
        app,
        store.inner().clone(),
        Arc::clone(watch_store.inner()),
        env_id,
        kind,
        namespace,
        label_selector,
        watch_token,
    )
    .await
}

#[tauri::command]
pub async fn kube_stop_watch(
    watch_store: State<'_, Arc<WatchStore>>,
    env_id: String,
) -> Result<(), String> {
    watch_store.stop(&env_id).await;
    Ok(())
}

#[tauri::command]
pub fn kube_get_tunnel_local_port(store: State<'_, KubeClientStore>, env_id: String) -> Option<u16> {
    store.get_tunnel_local_port(&env_id)
}

#[tauri::command]
pub async fn kube_remove_client(
    store: State<'_, KubeClientStore>,
    watch_store: State<'_, Arc<WatchStore>>,
    alias_store: State<'_, Arc<ResourceAliasCacheStore>>,
    env_id: String,
) -> Result<(), String> {
    watch_store.stop(&env_id).await;
    alias_store.remove_env(&env_id).await;
    store.remove(&env_id).await;
    Ok(())
}

/// 按当前集群重建资源别名索引（shortNames、plural、kind、singular），写入按环境缓存。
#[tauri::command]
pub async fn kube_refresh_resource_aliases(
    store: State<'_, KubeClientStore>,
    alias_store: State<'_, Arc<ResourceAliasCacheStore>>,
    env_id: String,
) -> Result<ResourceAliasRefreshResult, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;
    let client = store.get_or_build(&env).await.map_err(|e| e.to_string())?;
    alias_store
        .refresh(&env_id, &client)
        .await
        .map_err(|e| e.to_string())
}

/// 在已刷新的发现缓存中解析别名；`preferred_group` 非空时优先匹配该 API 组。
#[tauri::command]
pub async fn kube_resolve_resource_alias(
    alias_store: State<'_, Arc<ResourceAliasCacheStore>>,
    env_id: String,
    query: String,
    preferred_group: Option<String>,
) -> Result<Vec<ResolvedAliasTarget>, String> {
    alias_store
        .resolve(&env_id, &query, preferred_group.as_deref())
        .await
}
