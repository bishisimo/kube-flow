/**
 * K8s 资源相关 Tauri 命令封装。
 */
import { invoke } from "@tauri-apps/api/core";

export type {
  NamespaceItem,
  NodeItem,
  PodItem,
  WorkloadPodRollup,
  DeploymentItem,
  ServiceItem,
  StatefulSetItem,
  ConfigMapItem,
  SecretItem,
  ServiceAccountItem,
  RoleItem,
  SubjectRef,
  RoleBindingItem,
  ClusterRoleItem,
  ClusterRoleBindingItem,
  DaemonSetItem,
  PersistentVolumeClaimItem,
  PersistentVolumeItem,
  StorageClassItem,
  EndpointsItem,
  EndpointSliceItem,
  ReplicaSetItem,
  JobItem,
  CronJobItem,
  IngressItem,
  IngressClassItem,
  NetworkPolicyItem,
  ResourceQuotaItem,
  LimitRangeItem,
  PriorityClassItem,
  HorizontalPodAutoscalerItem,
  PodDisruptionBudgetItem,
  ResourceRef,
  ResourceGraphNode,
  ResourceGraphEdge,
  ResourceGraph,
  DescribeResult,
  ContainerImagePatch,
  ResolvedAliasTarget,
  ResourceAliasRefreshResult,
  DynamicCrdInstanceItem,
} from "./types/kube";

import type {
  NamespaceItem,
  NodeItem,
  PodItem,
  DeploymentItem,
  ServiceItem,
  StatefulSetItem,
  ConfigMapItem,
  SecretItem,
  ServiceAccountItem,
  RoleItem,
  RoleBindingItem,
  ClusterRoleItem,
  ClusterRoleBindingItem,
  DaemonSetItem,
  PersistentVolumeClaimItem,
  PersistentVolumeItem,
  StorageClassItem,
  EndpointsItem,
  EndpointSliceItem,
  ReplicaSetItem,
  JobItem,
  CronJobItem,
  IngressItem,
  IngressClassItem,
  NetworkPolicyItem,
  ResourceQuotaItem,
  LimitRangeItem,
  PriorityClassItem,
  HorizontalPodAutoscalerItem,
  PodDisruptionBudgetItem,
  ResourceGraph,
  DescribeResult,
  ContainerImagePatch,
  ResolvedAliasTarget,
  ResourceAliasRefreshResult,
  DynamicCrdInstanceItem,
} from "./types/kube";

// ─── Generic list factories ───────────────────────────────────────────────────

/** 命名空间级资源列表：(envId, namespace?, labelSelector?) => Promise<T[]> */
function kubeListNamespaced<T>(command: string) {
  return (
    envId: string,
    namespace?: string | null,
    labelSelector?: string | null
  ): Promise<T[]> =>
    invoke<T[]>(command, {
      envId,
      namespace: namespace ?? null,
      labelSelector: labelSelector ?? null,
    });
}

/** 集群级资源列表：(envId, labelSelector?) => Promise<T[]> */
function kubeListCluster<T>(command: string) {
  return (envId: string, labelSelector?: string | null): Promise<T[]> =>
    invoke<T[]>(command, { envId, labelSelector: labelSelector ?? null });
}

// ─── List functions ───────────────────────────────────────────────────────────

export const kubeListNamespaces = kubeListCluster<NamespaceItem>("kube_list_namespaces");
export const kubeListNodes = kubeListCluster<NodeItem>("kube_list_nodes");
export const kubeListPods = kubeListNamespaced<PodItem>("kube_list_pods");
export const kubeListDeployments = kubeListNamespaced<DeploymentItem>("kube_list_deployments");
export const kubeListServices = kubeListNamespaced<ServiceItem>("kube_list_services");
export const kubeListStatefulSets = kubeListNamespaced<StatefulSetItem>("kube_list_stateful_sets");
export const kubeListConfigMaps = kubeListNamespaced<ConfigMapItem>("kube_list_config_maps");
export const kubeListSecrets = kubeListNamespaced<SecretItem>("kube_list_secrets");
export const kubeListServiceAccounts = kubeListNamespaced<ServiceAccountItem>("kube_list_service_accounts");
export const kubeListRoles = kubeListNamespaced<RoleItem>("kube_list_roles");
export const kubeListRoleBindings = kubeListNamespaced<RoleBindingItem>("kube_list_role_bindings");
export const kubeListClusterRoles = kubeListCluster<ClusterRoleItem>("kube_list_cluster_roles");
export const kubeListClusterRoleBindings = kubeListCluster<ClusterRoleBindingItem>("kube_list_cluster_role_bindings");
export const kubeListDaemonSets = kubeListNamespaced<DaemonSetItem>("kube_list_daemon_sets");
export const kubeListPersistentVolumeClaims = kubeListNamespaced<PersistentVolumeClaimItem>("kube_list_persistent_volume_claims");
export const kubeListPersistentVolumes = kubeListCluster<PersistentVolumeItem>("kube_list_persistent_volumes");
export const kubeListStorageClasses = kubeListCluster<StorageClassItem>("kube_list_storage_classes");
export const kubeListEndpoints = kubeListNamespaced<EndpointsItem>("kube_list_endpoints");
export const kubeListEndpointSlices = kubeListNamespaced<EndpointSliceItem>("kube_list_endpoint_slices");
export const kubeListReplicaSets = kubeListNamespaced<ReplicaSetItem>("kube_list_replica_sets");
export const kubeListJobs = kubeListNamespaced<JobItem>("kube_list_jobs");
export const kubeListCronJobs = kubeListNamespaced<CronJobItem>("kube_list_cron_jobs");
export const kubeListIngresses = kubeListNamespaced<IngressItem>("kube_list_ingresses");
export const kubeListIngressClasses = kubeListCluster<IngressClassItem>("kube_list_ingress_classes");
export const kubeListNetworkPolicies = kubeListNamespaced<NetworkPolicyItem>("kube_list_network_policies");
export const kubeListResourceQuotas = kubeListNamespaced<ResourceQuotaItem>("kube_list_resource_quotas");
export const kubeListLimitRanges = kubeListNamespaced<LimitRangeItem>("kube_list_limit_ranges");
export const kubeListPriorityClasses = kubeListCluster<PriorityClassItem>("kube_list_priority_classes");
export const kubeListHorizontalPodAutoscalers = kubeListNamespaced<HorizontalPodAutoscalerItem>("kube_list_horizontal_pod_autoscalers");
export const kubeListPodDisruptionBudgets = kubeListNamespaced<PodDisruptionBudgetItem>("kube_list_pod_disruption_budgets");

// ─── Special list (non-generic) ───────────────────────────────────────────────

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

// ─── Get / Describe / Topology ───────────────────────────────────────────────

export function kubeGetResourceGraph(
  envId: string,
  kind: string,
  name: string,
  namespace?: string | null
): Promise<ResourceGraph> {
  return invoke("kube_get_resource_graph", {
    envId,
    kind,
    name,
    namespace: namespace ?? null,
  });
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

// ─── Pod operations ───────────────────────────────────────────────────────────

export function kubeGetPodContainers(
  envId: string,
  namespace: string,
  podName: string
): Promise<string[]> {
  return invoke("kube_get_pod_containers", { envId, namespace, podName });
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

export function kubePodExecResize(streamId: string, cols: number, rows: number): Promise<void> {
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

// ─── Resource mutations ───────────────────────────────────────────────────────

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

export function kubePatchContainerImages(
  envId: string,
  kind: string,
  name: string,
  namespace: string | null,
  patches: ContainerImagePatch[]
): Promise<void> {
  return invoke("kube_patch_container_images", { envId, kind, name, namespace, patches });
}

// ─── Tunnel ───────────────────────────────────────────────────────────────────

/** 若该环境为 SSH 隧道且隧道已建立，返回本地映射端口；否则返回 null。 */
export function kubeGetTunnelLocalPort(envId: string): Promise<number | null> {
  return invoke<number | null>("kube_get_tunnel_local_port", { envId });
}

// ─── Resource aliases ─────────────────────────────────────────────────────────

export function kubeRefreshResourceAliases(envId: string): Promise<ResourceAliasRefreshResult> {
  return invoke("kube_refresh_resource_aliases", { envId });
}

export function kubeResolveResourceAlias(
  envId: string,
  query: string,
  preferredGroup?: string | null
): Promise<ResolvedAliasTarget[]> {
  return invoke("kube_resolve_resource_alias", {
    envId,
    query,
    preferredGroup: preferredGroup?.trim() || null,
  });
}

// ─── Dynamic / CRD resources ─────────────────────────────────────────────────

export function kubeListCrdInstances(
  envId: string,
  params: {
    apiVersion: string;
    kind: string;
    namespace?: string | null;
    labelSelector?: string | null;
  }
): Promise<DynamicCrdInstanceItem[]> {
  return invoke("kube_list_crd_instances", {
    envId,
    apiVersion: params.apiVersion,
    kind: params.kind,
    namespace: params.namespace ?? null,
    labelSelector: params.labelSelector?.trim() || null,
  });
}

export function kubeGetDynamicResource(
  envId: string,
  apiVersion: string,
  kind: string,
  name: string,
  namespace?: string | null
): Promise<string> {
  return invoke("kube_get_dynamic_resource", {
    envId,
    apiVersion,
    kind,
    name,
    namespace: namespace ?? null,
  });
}

export function kubeDescribeDynamicResource(
  envId: string,
  apiVersion: string,
  kind: string,
  name: string,
  namespace?: string | null
): Promise<DescribeResult> {
  return invoke("kube_describe_dynamic_resource", {
    envId,
    apiVersion,
    kind,
    name,
    namespace: namespace ?? null,
  });
}

export function kubeDeleteDynamicResource(
  envId: string,
  apiVersion: string,
  kind: string,
  name: string,
  namespace?: string | null
): Promise<void> {
  return invoke("kube_delete_dynamic_resource", {
    envId,
    apiVersion,
    kind,
    name,
    namespace: namespace ?? null,
  });
}

// ─── Client lifecycle ─────────────────────────────────────────────────────────

export function kubeRemoveClient(envId: string): Promise<void> {
  return invoke("kube_remove_client", { envId });
}

// ─── Watch ────────────────────────────────────────────────────────────────────

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
