//! K8s 客户端构建与缓存、资源列表等；按环境 id 复用 Client；SSH 隧道支持。

mod client;
mod resource_alias_cache;
pub mod resource_apply;
pub mod resource_delete;
pub mod resource_describe;
mod resource_dynamic;
pub mod file_transfer;
pub mod resource_exec;
pub mod resource_file;
pub mod resource_get;
pub mod resource_graph;
pub mod resource_log;
pub mod resource_patch;
pub mod resource_replicas;
pub mod resource_watch;
pub mod resources;
pub mod session_store;
mod tunnel;

pub use client::KubeClientStore;
pub use resource_alias_cache::{
    ResolvedAliasTarget, ResourceAliasCacheStore, ResourceAliasRefreshResult,
};
pub use resource_apply::{apply_resource_yaml, deploy_resource_yaml};
pub use resource_delete::delete_resource;
pub use resource_describe::{describe_dynamic_resource, describe_resource, DescribeResult};
pub use resource_dynamic::{
    delete_dynamic_resource, get_dynamic_resource_yaml, list_crd_instances, DynamicCrdInstanceItem,
};
pub use file_transfer::FileTransferStore;
pub use resource_exec::{run_pod_exec, PodExecStore};
pub use resource_file::{download_file_from_pod, upload_file_to_pod};
pub use resource_get::get_resource_yaml;
pub use resource_graph::{build_graph, ResourceGraph};
pub use resource_log::{
    get_pod_container_names, get_pod_logs, run_pod_log_stream, PodLogStreamStore,
};
pub use resource_patch::{patch_container_images, patch_resource_strategic, ContainerImagePatch};
pub use resource_replicas::{restart_workload, resume_workload, stop_workload};
pub use resource_watch::{start_watch, WatchStore};
pub use resources::{
    list_cluster_role_bindings, list_cluster_roles, list_config_maps, list_cron_jobs,
    list_daemon_sets, list_deployments, list_endpoint_slices, list_endpoints,
    list_horizontal_pod_autoscalers, list_ingress_classes, list_ingresses, list_jobs,
    list_limit_ranges, list_namespaces, list_network_policies, list_nodes,
    list_persistent_volume_claims, list_persistent_volumes, list_pod_disruption_budgets, list_pods,
    list_pods_for_workload, list_priority_classes, list_replica_sets, list_resource_quotas,
    list_role_bindings, list_roles, list_secrets, list_service_accounts, list_services,
    list_stateful_sets, list_storage_classes, ClusterRoleBindingItem, ClusterRoleItem,
    ConfigMapItem, CronJobItem, DaemonSetItem, DeploymentItem, EndpointSliceItem, EndpointsItem,
    HorizontalPodAutoscalerItem, IngressClassItem, IngressItem, JobItem, LimitRangeItem,
    NamespaceItem, NetworkPolicyItem, NodeItem, PersistentVolumeClaimItem, PersistentVolumeItem,
    PodDisruptionBudgetItem, PodItem, PriorityClassItem, ReplicaSetItem, ResourceQuotaItem,
    RoleBindingItem, RoleItem, SecretItem, ServiceAccountItem, ServiceItem, StatefulSetItem,
    StorageClassItem,
};
