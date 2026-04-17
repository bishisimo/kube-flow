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

// ── 宏定义 ─────────────────────────────────────────────────────────────────

macro_rules! kube_list_cluster {
    ($fn_name:ident, $item_ty:ty, $list_fn:ident, $kind_str:literal) => {
        #[tauri::command]
        pub async fn $fn_name(
            store: State<'_, KubeClientStore>,
            env_id: String,
            label_selector: Option<String>,
        ) -> CommandResult<Vec<$item_ty>> {
            let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
            with_list_log($kind_str, &env_id, $list_fn(&client, label_selector.as_deref())).await
        }
    };
}

macro_rules! kube_list_namespaced {
    ($fn_name:ident, $item_ty:ty, $list_fn:ident, $kind_str:literal) => {
        #[tauri::command]
        pub async fn $fn_name(
            store: State<'_, KubeClientStore>,
            env_id: String,
            namespace: Option<String>,
            label_selector: Option<String>,
        ) -> CommandResult<Vec<$item_ty>> {
            let (env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
            let ns = namespace.as_deref().or_else(|| env.default_namespace());
            with_list_log($kind_str, &env_id, $list_fn(&client, ns, label_selector.as_deref())).await
        }
    };
}

// ── 集群级（无 namespace）──────────────────────────────────────────────────

kube_list_cluster!(kube_list_namespaces, NamespaceItem, list_namespaces, "Namespace");
kube_list_cluster!(kube_list_cluster_roles, ClusterRoleItem, list_cluster_roles, "ClusterRole");
kube_list_cluster!(kube_list_cluster_role_bindings, ClusterRoleBindingItem, list_cluster_role_bindings, "ClusterRoleBinding");
kube_list_cluster!(kube_list_persistent_volumes, PersistentVolumeItem, list_persistent_volumes, "PersistentVolume");
kube_list_cluster!(kube_list_storage_classes, StorageClassItem, list_storage_classes, "StorageClass");
kube_list_cluster!(kube_list_ingress_classes, IngressClassItem, list_ingress_classes, "IngressClass");
kube_list_cluster!(kube_list_priority_classes, PriorityClassItem, list_priority_classes, "PriorityClass");

/// Node 列表：额外读取 GPU 资源名配置，不走通用宏。
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

// ── 命名空间级 ─────────────────────────────────────────────────────────────

kube_list_namespaced!(kube_list_pods, PodItem, list_pods, "Pod");
kube_list_namespaced!(kube_list_deployments, DeploymentItem, list_deployments, "Deployment");
kube_list_namespaced!(kube_list_services, ServiceItem, list_services, "Service");
kube_list_namespaced!(kube_list_stateful_sets, StatefulSetItem, list_stateful_sets, "StatefulSet");
kube_list_namespaced!(kube_list_daemon_sets, DaemonSetItem, list_daemon_sets, "DaemonSet");
kube_list_namespaced!(kube_list_replica_sets, ReplicaSetItem, list_replica_sets, "ReplicaSet");
kube_list_namespaced!(kube_list_jobs, JobItem, list_jobs, "Job");
kube_list_namespaced!(kube_list_cron_jobs, CronJobItem, list_cron_jobs, "CronJob");
kube_list_namespaced!(kube_list_config_maps, ConfigMapItem, list_config_maps, "ConfigMap");
kube_list_namespaced!(kube_list_secrets, SecretItem, list_secrets, "Secret");
kube_list_namespaced!(kube_list_service_accounts, ServiceAccountItem, list_service_accounts, "ServiceAccount");
kube_list_namespaced!(kube_list_roles, RoleItem, list_roles, "Role");
kube_list_namespaced!(kube_list_role_bindings, RoleBindingItem, list_role_bindings, "RoleBinding");
kube_list_namespaced!(kube_list_persistent_volume_claims, PersistentVolumeClaimItem, list_persistent_volume_claims, "PersistentVolumeClaim");
kube_list_namespaced!(kube_list_endpoints, EndpointsItem, list_endpoints, "Endpoints");
kube_list_namespaced!(kube_list_endpoint_slices, EndpointSliceItem, list_endpoint_slices, "EndpointSlice");
kube_list_namespaced!(kube_list_ingresses, IngressItem, list_ingresses, "Ingress");
kube_list_namespaced!(kube_list_network_policies, NetworkPolicyItem, list_network_policies, "NetworkPolicy");
kube_list_namespaced!(kube_list_resource_quotas, ResourceQuotaItem, list_resource_quotas, "ResourceQuota");
kube_list_namespaced!(kube_list_limit_ranges, LimitRangeItem, list_limit_ranges, "LimitRange");
kube_list_namespaced!(kube_list_horizontal_pod_autoscalers, HorizontalPodAutoscalerItem, list_horizontal_pod_autoscalers, "HorizontalPodAutoscaler");
kube_list_namespaced!(kube_list_pod_disruption_budgets, PodDisruptionBudgetItem, list_pod_disruption_budgets, "PodDisruptionBudget");

/// Pod-for-workload 列表：接收 kind/name 而非 label_selector，不走通用宏。
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

// ── CRD 动态资源 ───────────────────────────────────────────────────────────

/// CRD 实例列表：额外接收 api_version/kind，不走通用宏。
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
