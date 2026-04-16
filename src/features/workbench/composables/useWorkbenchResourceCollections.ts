import { ref, type Ref } from "vue";
import type { ResourceKind } from "../../../constants/resourceKinds";
import { WORKBENCH_ALL_NAMESPACES_SENTINEL } from "../constants";
import type {
  ClusterRoleBindingItem,
  ClusterRoleItem,
  ConfigMapItem,
  CronJobItem,
  DaemonSetItem,
  DeploymentItem,
  DynamicCrdInstanceItem,
  EndpointSliceItem,
  EndpointsItem,
  HorizontalPodAutoscalerItem,
  IngressClassItem,
  IngressItem,
  JobItem,
  LimitRangeItem,
  NamespaceItem,
  NetworkPolicyItem,
  NodeItem,
  PersistentVolumeClaimItem,
  PersistentVolumeItem,
  PodDisruptionBudgetItem,
  PodItem,
  PriorityClassItem,
  ReplicaSetItem,
  ResourceQuotaItem,
  RoleBindingItem,
  RoleItem,
  SecretItem,
  ServiceAccountItem,
  ServiceItem,
  StatefulSetItem,
  StorageClassItem,
} from "../../../api/kube";

export type ResourceCacheEntry = {
  envId: string;
  kind: ResourceKind;
  namespace: string | null;
  labelSelector: string | null;
  items: unknown[];
  updatedAt: number;
};

export type UseWorkbenchResourceCollectionsOptions = {
  listLoading: Ref<boolean>;
  listError: Ref<string | null>;
  envSwitching: Ref<boolean>;
};

/**
 * 工作台各资源类型的列表数据、内存缓存与写入分发。
 * 与连接状态、Watch 编排解耦，由视图层传入列表区 UI 状态 ref。
 */
export function useWorkbenchResourceCollections(options: UseWorkbenchResourceCollectionsOptions) {
  const dynamicCrdItems = ref<DynamicCrdInstanceItem[]>([]);
  const namespaceOptions = ref<NamespaceItem[]>([]);
  const pods = ref<PodItem[]>([]);
  const deployments = ref<DeploymentItem[]>([]);
  const services = ref<ServiceItem[]>([]);
  const statefulSets = ref<StatefulSetItem[]>([]);
  const configMaps = ref<ConfigMapItem[]>([]);
  const secrets = ref<SecretItem[]>([]);
  const serviceAccounts = ref<ServiceAccountItem[]>([]);
  const roles = ref<RoleItem[]>([]);
  const roleBindings = ref<RoleBindingItem[]>([]);
  const clusterRoles = ref<ClusterRoleItem[]>([]);
  const clusterRoleBindings = ref<ClusterRoleBindingItem[]>([]);
  const daemonSets = ref<DaemonSetItem[]>([]);
  const nodes = ref<NodeItem[]>([]);
  const persistentVolumeClaims = ref<PersistentVolumeClaimItem[]>([]);
  const persistentVolumes = ref<PersistentVolumeItem[]>([]);
  const storageClasses = ref<StorageClassItem[]>([]);
  const endpoints = ref<EndpointsItem[]>([]);
  const endpointSlices = ref<EndpointSliceItem[]>([]);
  const replicaSets = ref<ReplicaSetItem[]>([]);
  const jobs = ref<JobItem[]>([]);
  const cronJobs = ref<CronJobItem[]>([]);
  const ingresses = ref<IngressItem[]>([]);
  const ingressClasses = ref<IngressClassItem[]>([]);
  const networkPolicies = ref<NetworkPolicyItem[]>([]);
  const resourceQuotas = ref<ResourceQuotaItem[]>([]);
  const limitRanges = ref<LimitRangeItem[]>([]);
  const priorityClasses = ref<PriorityClassItem[]>([]);
  const horizontalPodAutoscalers = ref<HorizontalPodAutoscalerItem[]>([]);
  const podDisruptionBudgets = ref<PodDisruptionBudgetItem[]>([]);

  const namespaceCache = new Map<string, NamespaceItem[]>();
  const resourceCache = new Map<string, ResourceCacheEntry>();

  function buildResourceCacheKey(
    envId: string,
    kind: ResourceKind,
    namespace: string | null,
    labelSelector: string | null
  ): string {
    return `${envId}::${kind}::${namespace ?? WORKBENCH_ALL_NAMESPACES_SENTINEL}::${labelSelector ?? ""}`;
  }

  function clearResourceCollections() {
    dynamicCrdItems.value = [];
    namespaceOptions.value = [];
    pods.value = [];
    deployments.value = [];
    replicaSets.value = [];
    jobs.value = [];
    cronJobs.value = [];
    services.value = [];
    statefulSets.value = [];
    configMaps.value = [];
    secrets.value = [];
    serviceAccounts.value = [];
    roles.value = [];
    roleBindings.value = [];
    clusterRoles.value = [];
    clusterRoleBindings.value = [];
    daemonSets.value = [];
    nodes.value = [];
    persistentVolumeClaims.value = [];
    persistentVolumes.value = [];
    storageClasses.value = [];
    endpoints.value = [];
    endpointSlices.value = [];
    ingresses.value = [];
    ingressClasses.value = [];
    networkPolicies.value = [];
    resourceQuotas.value = [];
    limitRanges.value = [];
    priorityClasses.value = [];
    horizontalPodAutoscalers.value = [];
    podDisruptionBudgets.value = [];
  }

  function setResourceItems(kind: ResourceKind, items: unknown[]) {
    switch (kind) {
      case "namespaces":
        namespaceOptions.value = items as NamespaceItem[];
        break;
      case "nodes":
        nodes.value = items as NodeItem[];
        break;
      case "pods":
        pods.value = items as PodItem[];
        break;
      case "deployments":
        deployments.value = items as DeploymentItem[];
        break;
      case "services":
        services.value = items as ServiceItem[];
        break;
      case "statefulsets":
        statefulSets.value = items as StatefulSetItem[];
        break;
      case "configmaps":
        configMaps.value = items as ConfigMapItem[];
        break;
      case "secrets":
        secrets.value = items as SecretItem[];
        break;
      case "serviceaccounts":
        serviceAccounts.value = items as ServiceAccountItem[];
        break;
      case "roles":
        roles.value = items as RoleItem[];
        break;
      case "rolebindings":
        roleBindings.value = items as RoleBindingItem[];
        break;
      case "clusterroles":
        clusterRoles.value = items as ClusterRoleItem[];
        break;
      case "clusterrolebindings":
        clusterRoleBindings.value = items as ClusterRoleBindingItem[];
        break;
      case "daemonsets":
        daemonSets.value = items as DaemonSetItem[];
        break;
      case "persistentvolumeclaims":
        persistentVolumeClaims.value = items as PersistentVolumeClaimItem[];
        break;
      case "persistentvolumes":
        persistentVolumes.value = items as PersistentVolumeItem[];
        break;
      case "storageclasses":
        storageClasses.value = items as StorageClassItem[];
        break;
      case "endpoints":
        endpoints.value = items as EndpointsItem[];
        break;
      case "endpointslices":
        endpointSlices.value = items as EndpointSliceItem[];
        break;
      case "replicasets":
        replicaSets.value = items as ReplicaSetItem[];
        break;
      case "jobs":
        jobs.value = items as JobItem[];
        break;
      case "cronjobs":
        cronJobs.value = items as CronJobItem[];
        break;
      case "ingresses":
        ingresses.value = items as IngressItem[];
        break;
      case "ingressclasses":
        ingressClasses.value = items as IngressClassItem[];
        break;
      case "networkpolicies":
        networkPolicies.value = items as NetworkPolicyItem[];
        break;
      case "resourcequotas":
        resourceQuotas.value = items as ResourceQuotaItem[];
        break;
      case "limitranges":
        limitRanges.value = items as LimitRangeItem[];
        break;
      case "priorityclasses":
        priorityClasses.value = items as PriorityClassItem[];
        break;
      case "horizontalpodautoscalers":
        horizontalPodAutoscalers.value = items as HorizontalPodAutoscalerItem[];
        break;
      case "poddisruptionbudgets":
        podDisruptionBudgets.value = items as PodDisruptionBudgetItem[];
        break;
    }
  }

  function cacheCurrentView(
    envId: string,
    kind: ResourceKind,
    namespace: string | null,
    labelSelector: string | null,
    items: unknown[]
  ) {
    resourceCache.set(buildResourceCacheKey(envId, kind, namespace, labelSelector), {
      envId,
      kind,
      namespace,
      labelSelector,
      items: [...items],
      updatedAt: Date.now(),
    });
  }

  function applyCachedView(
    envId: string,
    kind: ResourceKind,
    namespace: string | null,
    labelSelector: string | null
  ): boolean {
    const cachedNamespaces = namespaceCache.get(envId);
    const entry = resourceCache.get(buildResourceCacheKey(envId, kind, namespace, labelSelector));
    if (!cachedNamespaces && !entry) return false;
    clearResourceCollections();
    if (cachedNamespaces) {
      namespaceOptions.value = [...cachedNamespaces];
    }
    if (entry) {
      setResourceItems(kind, [...entry.items]);
    }
    options.envSwitching.value = false;
    options.listLoading.value = false;
    options.listError.value = null;
    return true;
  }

  return {
    dynamicCrdItems,
    namespaceOptions,
    pods,
    deployments,
    services,
    statefulSets,
    configMaps,
    secrets,
    serviceAccounts,
    roles,
    roleBindings,
    clusterRoles,
    clusterRoleBindings,
    daemonSets,
    nodes,
    persistentVolumeClaims,
    persistentVolumes,
    storageClasses,
    endpoints,
    endpointSlices,
    replicaSets,
    jobs,
    cronJobs,
    ingresses,
    ingressClasses,
    networkPolicies,
    resourceQuotas,
    limitRanges,
    priorityClasses,
    horizontalPodAutoscalers,
    podDisruptionBudgets,
    namespaceCache,
    resourceCache,
    buildResourceCacheKey,
    clearResourceCollections,
    setResourceItems,
    cacheCurrentView,
    applyCachedView,
  };
}
