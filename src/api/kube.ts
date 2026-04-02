/**
 * K8s 资源相关 Tauri 命令封装。
 */
import { invoke } from "@tauri-apps/api/core";

export interface NamespaceItem {
  name: string;
  creation_time?: string | null;
}

export interface NodeItem {
  name: string;
  status?: string | null;
  internal_ip?: string | null;
  creation_time?: string | null;
}

export interface PodItem {
  name: string;
  namespace: string;
  phase?: string | null;
  container_status?: string | null;
  pod_ip?: string | null;
  node_name?: string | null;
  creation_time?: string | null;
}


export interface WorkloadPodRollup {
  running_ready?: number | null;
  pending?: number | null;
  succeeded?: number | null;
  failed?: number | null;
  abnormal?: number | null;
  last_container_restart?: string | null;
}

export interface DeploymentItem {
  name: string;
  namespace: string;
  replicas?: number | null;
  ready?: number | null;
  creation_time?: string | null;
  label_selector?: string | null;
  pod_rollup?: WorkloadPodRollup | null;
}

export interface ServiceItem {
  name: string;
  namespace: string;
  service_type?: string | null;
  cluster_ip?: string | null;
  /** 端口摘要，如 "80/TCP, 443/TCP" */
  ports?: string | null;
  creation_time?: string | null;
}

export interface StatefulSetItem {
  name: string;
  namespace: string;
  replicas?: number | null;
  ready?: number | null;
  creation_time?: string | null;
  label_selector?: string | null;
  pod_rollup?: WorkloadPodRollup | null;
}

export interface ConfigMapItem {
  name: string;
  namespace: string;
  keys?: number | null;
  creation_time?: string | null;
}

export interface SecretItem {
  name: string;
  namespace: string;
  type_?: string | null;
  keys?: number | null;
  creation_time?: string | null;
}

export interface ServiceAccountItem {
  name: string;
  namespace: string;
  creation_time?: string | null;
}

export interface RoleItem {
  name: string;
  namespace: string;
  creation_time?: string | null;
}

export interface SubjectRef {
  kind: string;
  name: string;
  namespace?: string | null;
}

export interface RoleBindingItem {
  name: string;
  namespace: string;
  role_ref?: string | null;
  role_ref_kind?: string | null;
  role_ref_name?: string | null;
  subjects?: number | null;
  subjects_list?: SubjectRef[] | null;
  creation_time?: string | null;
}

export interface ClusterRoleItem {
  name: string;
  creation_time?: string | null;
}

export interface ClusterRoleBindingItem {
  name: string;
  role_ref?: string | null;
  role_ref_kind?: string | null;
  role_ref_name?: string | null;
  subjects?: number | null;
  subjects_list?: SubjectRef[] | null;
  creation_time?: string | null;
}

export interface DaemonSetItem {
  name: string;
  namespace: string;
  desired?: number | null;
  ready?: number | null;
  creation_time?: string | null;
  label_selector?: string | null;
  pod_rollup?: WorkloadPodRollup | null;
}

export interface PersistentVolumeClaimItem {
  name: string;
  namespace: string;
  status?: string | null;
  capacity?: string | null;
  /** 绑定的 PV 名称（Bound 后有值） */
  volume?: string | null;
  /** StorageClass 名称 */
  storage_class?: string | null;
  creation_time?: string | null;
}

export interface PersistentVolumeItem {
  name: string;
  capacity?: string | null;
  status?: string | null;
  creation_time?: string | null;
}

export interface StorageClassItem {
  name: string;
  provisioner?: string | null;
  creation_time?: string | null;
}

export interface EndpointsItem {
  name: string;
  namespace: string;
  subsets?: number | null;
  creation_time?: string | null;
}

export interface EndpointSliceItem {
  name: string;
  namespace: string;
  address_type?: string | null;
  endpoints?: number | null;
  creation_time?: string | null;
}

export interface ReplicaSetItem {
  name: string;
  namespace: string;
  replicas?: number | null;
  ready?: number | null;
  creation_time?: string | null;
  label_selector?: string | null;
}

export interface JobItem {
  name: string;
  namespace: string;
  completions?: string | null;
  duration?: string | null;
  creation_time?: string | null;
}

export interface CronJobItem {
  name: string;
  namespace: string;
  schedule?: string | null;
  last_schedule?: string | null;
  creation_time?: string | null;
}

export interface IngressItem {
  name: string;
  namespace: string;
  class?: string | null;
  hosts?: string | null;
  creation_time?: string | null;
}

export interface IngressClassItem {
  name: string;
  controller?: string | null;
  creation_time?: string | null;
}

export interface NetworkPolicyItem {
  name: string;
  namespace: string;
  creation_time?: string | null;
}

export interface ResourceQuotaItem {
  name: string;
  namespace: string;
  creation_time?: string | null;
}

export interface LimitRangeItem {
  name: string;
  namespace: string;
  creation_time?: string | null;
}

export interface PriorityClassItem {
  name: string;
  value?: number | null;
  creation_time?: string | null;
}

export interface HorizontalPodAutoscalerItem {
  name: string;
  namespace: string;
  reference?: string | null;
  replicas?: string | null;
  creation_time?: string | null;
}

export interface PodDisruptionBudgetItem {
  name: string;
  namespace: string;
  min_available?: string | null;
  max_unavailable?: string | null;
  allowed_disruptions?: number | null;
  creation_time?: string | null;
}

export function kubeListNamespaces(
  envId: string,
  labelSelector?: string | null
): Promise<NamespaceItem[]> {
  return invoke("kube_list_namespaces", { envId, labelSelector: labelSelector ?? null });
}

export function kubeListNodes(
  envId: string,
  labelSelector?: string | null
): Promise<NodeItem[]> {
  return invoke("kube_list_nodes", { envId, labelSelector: labelSelector ?? null });
}

export function kubeListPods(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<PodItem[]> {
  return invoke("kube_list_pods", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

/** 列出 Workload（Deployment/StatefulSet/DaemonSet）管理的 Pods */
export function kubeListPodsForWorkload(
  envId: string,
  kind: string,
  name: string,
  namespace?: string | null
): Promise<PodItem[]> {
  return invoke("kube_list_pods_for_workload", {
    envId,
    kind,
    name,
    namespace: namespace ?? null,
  });
}

export function kubeListDeployments(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<DeploymentItem[]> {
  return invoke("kube_list_deployments", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListServices(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<ServiceItem[]> {
  return invoke("kube_list_services", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListStatefulSets(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<StatefulSetItem[]> {
  return invoke("kube_list_stateful_sets", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListConfigMaps(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<ConfigMapItem[]> {
  return invoke("kube_list_config_maps", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListSecrets(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<SecretItem[]> {
  return invoke("kube_list_secrets", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListServiceAccounts(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<ServiceAccountItem[]> {
  return invoke("kube_list_service_accounts", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListRoles(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<RoleItem[]> {
  return invoke("kube_list_roles", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListRoleBindings(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<RoleBindingItem[]> {
  return invoke("kube_list_role_bindings", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListClusterRoles(
  envId: string,
  labelSelector?: string | null
): Promise<ClusterRoleItem[]> {
  return invoke("kube_list_cluster_roles", { envId, labelSelector: labelSelector ?? null });
}

export function kubeListClusterRoleBindings(
  envId: string,
  labelSelector?: string | null
): Promise<ClusterRoleBindingItem[]> {
  return invoke("kube_list_cluster_role_bindings", {
    envId,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListDaemonSets(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<DaemonSetItem[]> {
  return invoke("kube_list_daemon_sets", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListPersistentVolumeClaims(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<PersistentVolumeClaimItem[]> {
  return invoke("kube_list_persistent_volume_claims", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListPersistentVolumes(
  envId: string,
  labelSelector?: string | null
): Promise<PersistentVolumeItem[]> {
  return invoke("kube_list_persistent_volumes", {
    envId,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListStorageClasses(
  envId: string,
  labelSelector?: string | null
): Promise<StorageClassItem[]> {
  return invoke("kube_list_storage_classes", {
    envId,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListEndpoints(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<EndpointsItem[]> {
  return invoke("kube_list_endpoints", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListEndpointSlices(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<EndpointSliceItem[]> {
  return invoke("kube_list_endpoint_slices", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListReplicaSets(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<ReplicaSetItem[]> {
  return invoke("kube_list_replica_sets", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListJobs(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<JobItem[]> {
  return invoke("kube_list_jobs", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListCronJobs(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<CronJobItem[]> {
  return invoke("kube_list_cron_jobs", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListIngresses(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<IngressItem[]> {
  return invoke("kube_list_ingresses", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListIngressClasses(
  envId: string,
  labelSelector?: string | null
): Promise<IngressClassItem[]> {
  return invoke("kube_list_ingress_classes", {
    envId,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListNetworkPolicies(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<NetworkPolicyItem[]> {
  return invoke("kube_list_network_policies", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListResourceQuotas(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<ResourceQuotaItem[]> {
  return invoke("kube_list_resource_quotas", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListLimitRanges(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<LimitRangeItem[]> {
  return invoke("kube_list_limit_ranges", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListPriorityClasses(
  envId: string,
  labelSelector?: string | null
): Promise<PriorityClassItem[]> {
  return invoke("kube_list_priority_classes", {
    envId,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListHorizontalPodAutoscalers(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<HorizontalPodAutoscalerItem[]> {
  return invoke("kube_list_horizontal_pod_autoscalers", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export function kubeListPodDisruptionBudgets(
  envId: string,
  namespace?: string | null,
  labelSelector?: string | null
): Promise<PodDisruptionBudgetItem[]> {
  return invoke("kube_list_pod_disruption_budgets", {
    envId,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
  });
}

export interface RelatedTarget {
  target_kind: string;
  label: string;
  namespace?: string | null;
  label_selector?: string | null;
  resource_name?: string | null;
}

export function kubeGetRelatedTargets(
  envId: string,
  kind: string,
  name: string,
  namespace?: string | null
): Promise<RelatedTarget[]> {
  return invoke("kube_get_related_targets", {
    envId,
    kind,
    name,
    namespace: namespace ?? null,
  });
}

export interface TopologyItem {
  kind: string;
  name: string;
  namespace?: string | null;
  target_kind: string;
  label: string;
  label_selector?: string | null;
  resource_name?: string | null;
  is_concrete: boolean;
  /** 关系类型：volumes、envFrom、selector、owner、runs-on、service-name、roleRef 等 */
  relation_type?: string | null;
}

export interface ResourceTopology {
  upstream: TopologyItem[];
  downstream: TopologyItem[];
}

export function kubeGetResourceTopology(
  envId: string,
  kind: string,
  name: string,
  namespace?: string | null
): Promise<ResourceTopology> {
  return invoke("kube_get_resource_topology", {
    envId,
    kind,
    name,
    namespace: namespace ?? null,
  });
}

export interface DescribeResult {
  markdown: string;
}

export function kubeDescribeResource(
  envId: string,
  kind: string,
  name: string,
  namespace?: string | null
): Promise<DescribeResult> {
  return invoke("kube_describe_resource", {
    envId,
    kind,
    name,
    namespace: namespace ?? null,
  });
}

export function kubeGetResource(
  envId: string,
  kind: string,
  name: string,
  namespace?: string | null
): Promise<string> {
  return invoke("kube_get_resource", {
    envId,
    kind,
    name,
    namespace: namespace ?? null,
  });
}

export function kubeGetPodContainers(
  envId: string,
  namespace: string,
  podName: string
): Promise<string[]> {
  return invoke("kube_get_pod_containers", {
    envId,
    namespace,
    podName,
  });
}

export function kubePodLogStreamStart(
  envId: string,
  namespace: string,
  podName: string,
  options?: {
    container?: string | null;
    tailLines?: number | null;
    sinceSeconds?: number | null;
    timestamps?: boolean;
    previous?: boolean;
  }
): Promise<string> {
  return invoke("kube_pod_log_stream_start", {
    envId,
    namespace,
    podName,
    container: options?.container ?? null,
    tailLines: options?.tailLines ?? null,
    sinceSeconds: options?.sinceSeconds ?? null,
    timestamps: options?.timestamps ?? false,
    previous: options?.previous ?? false,
  });
}

export function kubePodLogStreamStop(streamId: string): Promise<void> {
  return invoke("kube_pod_log_stream_stop", { streamId });
}

export function kubePodExecStart(
  envId: string,
  namespace: string,
  podName: string,
  container?: string | null
): Promise<string> {
  return invoke("kube_pod_exec_start", {
    envId,
    namespace,
    podName,
    container: container ?? null,
  });
}

export function kubePodExecStdin(streamId: string, data: number[]): Promise<void> {
  return invoke("kube_pod_exec_stdin", { streamId, data });
}

export function kubePodExecResize(
  streamId: string,
  cols: number,
  rows: number
): Promise<void> {
  return invoke("kube_pod_exec_resize", { streamId, cols, rows });
}

export function kubePodExecStop(streamId: string): Promise<void> {
  return invoke("kube_pod_exec_stop", { streamId });
}

export function kubePodLogs(
  envId: string,
  namespace: string,
  podName: string,
  options?: {
    container?: string | null;
    tailLines?: number | null;
    sinceSeconds?: number | null;
    timestamps?: boolean;
    previous?: boolean;
  }
): Promise<string> {
  return invoke("kube_pod_logs", {
    envId,
    namespace,
    podName,
    container: options?.container ?? null,
    tailLines: options?.tailLines ?? null,
    sinceSeconds: options?.sinceSeconds ?? null,
    timestamps: options?.timestamps ?? false,
    previous: options?.previous ?? false,
  });
}

export function kubeApplyResource(envId: string, yaml: string): Promise<void> {
  return invoke("kube_apply_resource", { envId, yaml });
}

export function kubeDeployResource(
  envId: string,
  yaml: string,
  strategy: "create_replace" | "apply"
): Promise<void> {
  return invoke("kube_deploy_resource", { envId, yaml, strategy });
}

export function kubeDeleteResource(
  envId: string,
  kind: string,
  name: string,
  namespace?: string | null
): Promise<void> {
  return invoke("kube_delete_resource", {
    envId,
    kind,
    name,
    namespace: namespace ?? null,
  });
}

export interface ContainerImagePatch {
  container_name: string;
  new_image: string;
}

export function kubePatchContainerImages(
  envId: string,
  kind: string,
  name: string,
  namespace: string | null,
  patches: ContainerImagePatch[]
): Promise<void> {
  return invoke("kube_patch_container_images", {
    envId,
    kind,
    name,
    namespace,
    patches,
  });
}

/** 若该环境为 SSH 隧道且隧道已建立，返回本地映射端口；否则返回 null。 */
export function kubeGetTunnelLocalPort(envId: string): Promise<number | null> {
  return invoke<number | null>("kube_get_tunnel_local_port", { envId });
}

export function kubeRemoveClient(envId: string): Promise<void> {
  return invoke("kube_remove_client", { envId });
}

/** 启动资源 Watch，labelSelector 参与 Watch 构建 */
export function kubeStartWatch(
  envId: string,
  kind: string,
  namespace?: string | null,
  labelSelector?: string | null,
  watchToken?: string
): Promise<void> {
  return invoke("kube_start_watch", {
    envId,
    kind,
    namespace: namespace ?? null,
    labelSelector: labelSelector ?? null,
    watchToken: watchToken ?? null,
  });
}

/** 停止资源 Watch */
export function kubeStopWatch(envId: string): Promise<void> {
  return invoke("kube_stop_watch", { envId });
}
