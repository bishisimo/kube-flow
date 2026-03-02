//! K8s 客户端构建与缓存、资源列表等；按环境 id 复用 Client；SSH 隧道支持。

mod client;
pub mod resource_apply;
pub mod resource_delete;
pub mod resource_watch;
pub mod resource_describe;
pub mod resource_get;
pub mod resource_exec;
pub mod resource_log;
pub mod resource_patch;
pub mod related_targets;
pub mod resource_topology;
pub mod resources;
mod tunnel;

pub use client::KubeClientStore;
pub use resource_watch::{start_watch, WatchStore};
pub use resource_apply::apply_resource_yaml;
pub use resource_delete::delete_resource;
pub use resource_patch::{patch_container_images, ContainerImagePatch};
pub use resource_describe::{describe_resource, DescribeResult};
pub use resource_get::get_resource_yaml;
pub use resource_exec::{run_pod_exec, PodExecStore};
pub use resource_log::{
    get_pod_container_names, get_pod_logs, run_pod_log_stream, PodLogStreamStore,
};
pub use related_targets::{get_related_targets, RelatedTarget};
pub use resource_topology::{get_resource_topology, ResourceTopology};
pub use resources::{
    list_cluster_role_bindings, list_cluster_roles, list_config_maps, list_cron_jobs,
    list_daemon_sets, list_deployments, list_endpoint_slices, list_endpoints,
    list_pods_for_workload,
    list_horizontal_pod_autoscalers, list_ingress_classes, list_ingresses, list_jobs,
    list_limit_ranges, list_namespaces, list_network_policies, list_nodes,
    list_persistent_volume_claims, list_persistent_volumes, list_pod_disruption_budgets,
    list_pods, list_priority_classes, list_replica_sets, list_resource_quotas,
    list_role_bindings, list_roles, list_secrets,     list_service_accounts, list_services,
    list_stateful_sets, list_storage_classes,
    ClusterRoleBindingItem, ClusterRoleItem, ConfigMapItem, CronJobItem, DaemonSetItem,
    DeploymentItem, EndpointSliceItem, EndpointsItem, HorizontalPodAutoscalerItem,
    IngressClassItem, IngressItem, JobItem, LimitRangeItem, NamespaceItem, NetworkPolicyItem,
    NodeItem, PersistentVolumeClaimItem, PersistentVolumeItem, PodDisruptionBudgetItem,
    PodItem, PriorityClassItem, ReplicaSetItem, ResourceQuotaItem, RoleBindingItem, RoleItem,
    SecretItem, ServiceAccountItem, ServiceItem, StatefulSetItem, StorageClassItem,
};
