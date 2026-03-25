import {
  kubeListClusterRoleBindings,
  kubeListClusterRoles,
  kubeListConfigMaps,
  kubeListCronJobs,
  kubeListDaemonSets,
  kubeListDeployments,
  kubeListEndpointSlices,
  kubeListEndpoints,
  kubeListHorizontalPodAutoscalers,
  kubeListIngressClasses,
  kubeListIngresses,
  kubeListJobs,
  kubeListLimitRanges,
  kubeListNamespaces,
  kubeListNetworkPolicies,
  kubeListNodes,
  kubeListPersistentVolumeClaims,
  kubeListPersistentVolumes,
  kubeListPodDisruptionBudgets,
  kubeListPods,
  kubeListPriorityClasses,
  kubeListReplicaSets,
  kubeListResourceQuotas,
  kubeListRoleBindings,
  kubeListRoles,
  kubeListSecrets,
  kubeListServiceAccounts,
  kubeListServices,
  kubeListStatefulSets,
  kubeListStorageClasses,
} from "../api/kube";
import type { ResourceKind } from "../constants/resourceKinds";

export interface ResourceRegistryEntry {
  watch: boolean;
  clusterScoped: boolean;
  fetchList: (envId: string, namespace: string | null, labelSelector: string | null) => Promise<unknown[]>;
}

export const RESOURCE_REGISTRY: Record<ResourceKind, ResourceRegistryEntry> = {
  namespaces: {
    watch: true,
    clusterScoped: true,
    fetchList: (envId, _ns, labelSelector) => kubeListNamespaces(envId, labelSelector),
  },
  nodes: {
    watch: true,
    clusterScoped: true,
    fetchList: (envId, _ns, labelSelector) => kubeListNodes(envId, labelSelector),
  },
  pods: {
    watch: true,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListPods(envId, namespace, labelSelector),
  },
  deployments: {
    watch: true,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListDeployments(envId, namespace, labelSelector),
  },
  replicasets: {
    watch: false,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListReplicaSets(envId, namespace, labelSelector),
  },
  statefulsets: {
    watch: true,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListStatefulSets(envId, namespace, labelSelector),
  },
  daemonsets: {
    watch: true,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListDaemonSets(envId, namespace, labelSelector),
  },
  jobs: {
    watch: false,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListJobs(envId, namespace, labelSelector),
  },
  cronjobs: {
    watch: false,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListCronJobs(envId, namespace, labelSelector),
  },
  services: {
    watch: true,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListServices(envId, namespace, labelSelector),
  },
  endpoints: {
    watch: true,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListEndpoints(envId, namespace, labelSelector),
  },
  endpointslices: {
    watch: true,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListEndpointSlices(envId, namespace, labelSelector),
  },
  ingresses: {
    watch: false,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListIngresses(envId, namespace, labelSelector),
  },
  ingressclasses: {
    watch: false,
    clusterScoped: true,
    fetchList: (envId, _ns, labelSelector) => kubeListIngressClasses(envId, labelSelector),
  },
  networkpolicies: {
    watch: false,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListNetworkPolicies(envId, namespace, labelSelector),
  },
  configmaps: {
    watch: true,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListConfigMaps(envId, namespace, labelSelector),
  },
  secrets: {
    watch: true,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListSecrets(envId, namespace, labelSelector),
  },
  persistentvolumeclaims: {
    watch: true,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListPersistentVolumeClaims(envId, namespace, labelSelector),
  },
  persistentvolumes: {
    watch: true,
    clusterScoped: true,
    fetchList: (envId, _ns, labelSelector) => kubeListPersistentVolumes(envId, labelSelector),
  },
  storageclasses: {
    watch: true,
    clusterScoped: true,
    fetchList: (envId, _ns, labelSelector) => kubeListStorageClasses(envId, labelSelector),
  },
  serviceaccounts: {
    watch: true,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListServiceAccounts(envId, namespace, labelSelector),
  },
  roles: {
    watch: true,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListRoles(envId, namespace, labelSelector),
  },
  rolebindings: {
    watch: true,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListRoleBindings(envId, namespace, labelSelector),
  },
  clusterroles: {
    watch: true,
    clusterScoped: true,
    fetchList: (envId, _ns, labelSelector) => kubeListClusterRoles(envId, labelSelector),
  },
  clusterrolebindings: {
    watch: true,
    clusterScoped: true,
    fetchList: (envId, _ns, labelSelector) => kubeListClusterRoleBindings(envId, labelSelector),
  },
  resourcequotas: {
    watch: false,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListResourceQuotas(envId, namespace, labelSelector),
  },
  limitranges: {
    watch: false,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListLimitRanges(envId, namespace, labelSelector),
  },
  priorityclasses: {
    watch: false,
    clusterScoped: true,
    fetchList: (envId, _ns, labelSelector) => kubeListPriorityClasses(envId, labelSelector),
  },
  horizontalpodautoscalers: {
    watch: false,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListHorizontalPodAutoscalers(envId, namespace, labelSelector),
  },
  poddisruptionbudgets: {
    watch: false,
    clusterScoped: false,
    fetchList: (envId, namespace, labelSelector) => kubeListPodDisruptionBudgets(envId, namespace, labelSelector),
  },
};

export function resourceSupportsWatch(kind: ResourceKind): boolean {
  return RESOURCE_REGISTRY[kind].watch;
}

export function resourceIsClusterScoped(kind: ResourceKind): boolean {
  return RESOURCE_REGISTRY[kind].clusterScoped;
}

export function fetchResourceList(
  kind: ResourceKind,
  envId: string,
  namespace: string | null,
  labelSelector: string | null
): Promise<unknown[]> {
  return RESOURCE_REGISTRY[kind].fetchList(envId, namespace, labelSelector);
}
