//! K8s 资源列表命令（kube_list_*）：按资源类型分组，统一使用 with_list_log 包装。

use super::super::kube_command_context::{self, err_str, CommandResult};
use super::with_list_log;
use crate::kube::{
    list_cluster_role_bindings, list_cluster_roles, list_config_maps, list_cron_jobs,
    list_daemon_sets, list_deployments, list_endpoint_slices, list_endpoints,
    list_horizontal_pod_autoscalers, list_ingress_classes, list_ingresses, list_jobs,
    list_limit_ranges, list_namespaces, list_network_policies, list_nodes,
    list_persistent_volume_claims, list_persistent_volumes, list_pod_disruption_budgets,
    list_pods, list_pods_for_workload, list_priority_classes, list_replica_sets,
    list_resource_quotas, list_role_bindings, list_roles, list_secrets, list_service_accounts,
    list_services, list_stateful_sets, list_storage_classes,
    ClusterRoleBindingItem, ClusterRoleItem, ConfigMapItem, CronJobItem, DaemonSetItem,
    DeploymentItem, EndpointSliceItem, EndpointsItem, HorizontalPodAutoscalerItem,
    IngressClassItem, IngressItem, JobItem, LimitRangeItem, NamespaceItem, NetworkPolicyItem,
    NodeItem, PersistentVolumeClaimItem, PersistentVolumeItem, PodDisruptionBudgetItem,
    PodItem, PriorityClassItem, ReplicaSetItem, ResourceQuotaItem, RoleBindingItem, RoleItem,
    SecretItem, ServiceAccountItem, ServiceItem, StatefulSetItem, StorageClassItem,
    DynamicCrdInstanceItem, KubeClientStore,
    list_crd_instances,
};
use tauri::State;

// ── 集群级（无 namespace）──────────────────────────────────────────────────

#[tauri::command]
pub async fn kube_list_namespaces(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> CommandResult<Vec<NamespaceItem>> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    with_list_log("Namespace", &env_id, list_namespaces(&client, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_nodes(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> CommandResult<Vec<NodeItem>> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let gpu_resource_names = kube_command_context::load_app_settings()?.gpu_resource_names();
    with_list_log(
        "Node",
        &env_id,
        list_nodes(&client, label_selector.as_deref(), &gpu_resource_names),
    )
    .await
}

#[tauri::command]
pub async fn kube_list_cluster_roles(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> CommandResult<Vec<ClusterRoleItem>> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    with_list_log("ClusterRole", &env_id, list_cluster_roles(&client, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_cluster_role_bindings(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> CommandResult<Vec<ClusterRoleBindingItem>> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    with_list_log("ClusterRoleBinding", &env_id, list_cluster_role_bindings(&client, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_persistent_volumes(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> CommandResult<Vec<PersistentVolumeItem>> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    with_list_log("PersistentVolume", &env_id, list_persistent_volumes(&client, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_storage_classes(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> CommandResult<Vec<StorageClassItem>> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    with_list_log("StorageClass", &env_id, list_storage_classes(&client, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_ingress_classes(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> CommandResult<Vec<IngressClassItem>> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    with_list_log("IngressClass", &env_id, list_ingress_classes(&client, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_priority_classes(
    store: State<'_, KubeClientStore>,
    env_id: String,
    label_selector: Option<String>,
) -> CommandResult<Vec<PriorityClassItem>> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    with_list_log("PriorityClass", &env_id, list_priority_classes(&client, label_selector.as_deref())).await
}

// ── 命名空间级 ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn kube_list_pods(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<PodItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("Pod", &env_id, list_pods(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_pods_for_workload(
    store: State<'_, KubeClientStore>,
    env_id: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> CommandResult<Vec<PodItem>> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    list_pods_for_workload(&client, &kind, &name, namespace.as_deref())
        .await
        .map_err(err_str)
}

#[tauri::command]
pub async fn kube_list_deployments(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<DeploymentItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("Deployment", &env_id, list_deployments(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_services(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<ServiceItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("Service", &env_id, list_services(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_stateful_sets(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<StatefulSetItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("StatefulSet", &env_id, list_stateful_sets(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_daemon_sets(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<DaemonSetItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("DaemonSet", &env_id, list_daemon_sets(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_replica_sets(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<ReplicaSetItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("ReplicaSet", &env_id, list_replica_sets(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_jobs(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<JobItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("Job", &env_id, list_jobs(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_cron_jobs(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<CronJobItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("CronJob", &env_id, list_cron_jobs(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_config_maps(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<ConfigMapItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("ConfigMap", &env_id, list_config_maps(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_secrets(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<SecretItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("Secret", &env_id, list_secrets(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_service_accounts(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<ServiceAccountItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("ServiceAccount", &env_id, list_service_accounts(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_roles(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<RoleItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("Role", &env_id, list_roles(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_role_bindings(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<RoleBindingItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("RoleBinding", &env_id, list_role_bindings(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_persistent_volume_claims(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<PersistentVolumeClaimItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("PersistentVolumeClaim", &env_id, list_persistent_volume_claims(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_endpoints(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<EndpointsItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("Endpoints", &env_id, list_endpoints(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_endpoint_slices(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<EndpointSliceItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("EndpointSlice", &env_id, list_endpoint_slices(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_ingresses(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<IngressItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("Ingress", &env_id, list_ingresses(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_network_policies(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<NetworkPolicyItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("NetworkPolicy", &env_id, list_network_policies(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_resource_quotas(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<ResourceQuotaItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("ResourceQuota", &env_id, list_resource_quotas(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_limit_ranges(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<LimitRangeItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("LimitRange", &env_id, list_limit_ranges(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_horizontal_pod_autoscalers(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<HorizontalPodAutoscalerItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("HorizontalPodAutoscaler", &env_id, list_horizontal_pod_autoscalers(&client, ns, label_selector.as_deref())).await
}

#[tauri::command]
pub async fn kube_list_pod_disruption_budgets(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<PodDisruptionBudgetItem>> {
    let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let ns = namespace.as_deref().or_else(|| env.default_namespace());
    with_list_log("PodDisruptionBudget", &env_id, list_pod_disruption_budgets(&client, ns, label_selector.as_deref())).await
}

// ── CRD 动态资源 ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn kube_list_crd_instances(
    store: State<'_, KubeClientStore>,
    env_id: String,
    api_version: String,
    kind: String,
    namespace: Option<String>,
    label_selector: Option<String>,
) -> CommandResult<Vec<DynamicCrdInstanceItem>> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    with_list_log(
        &format!("CRD:{kind}"),
        &env_id,
        list_crd_instances(&client, &api_version, &kind, namespace.as_deref(), label_selector.as_deref()),
    )
    .await
}
