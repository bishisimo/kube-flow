import type { ResolvedAliasTarget } from "../../api/kube";
import type { ResourceKind } from "../../constants/resourceKinds";

/** 工作台内置资源在 discovery 中的 group + Kind（与集群 API 一致）。 */
const BUILTIN_GVK_ENTRIES: { id: ResourceKind; kind: string; group: string }[] = [
  { id: "namespaces", kind: "Namespace", group: "" },
  { id: "nodes", kind: "Node", group: "" },
  { id: "pods", kind: "Pod", group: "" },
  { id: "deployments", kind: "Deployment", group: "apps" },
  { id: "replicasets", kind: "ReplicaSet", group: "apps" },
  { id: "statefulsets", kind: "StatefulSet", group: "apps" },
  { id: "daemonsets", kind: "DaemonSet", group: "apps" },
  { id: "jobs", kind: "Job", group: "batch" },
  { id: "cronjobs", kind: "CronJob", group: "batch" },
  { id: "services", kind: "Service", group: "" },
  { id: "endpoints", kind: "Endpoints", group: "" },
  { id: "endpointslices", kind: "EndpointSlice", group: "discovery.k8s.io" },
  { id: "ingresses", kind: "Ingress", group: "networking.k8s.io" },
  { id: "ingressclasses", kind: "IngressClass", group: "networking.k8s.io" },
  { id: "networkpolicies", kind: "NetworkPolicy", group: "networking.k8s.io" },
  { id: "configmaps", kind: "ConfigMap", group: "" },
  { id: "secrets", kind: "Secret", group: "" },
  { id: "persistentvolumeclaims", kind: "PersistentVolumeClaim", group: "" },
  { id: "persistentvolumes", kind: "PersistentVolume", group: "" },
  { id: "storageclasses", kind: "StorageClass", group: "storage.k8s.io" },
  { id: "serviceaccounts", kind: "ServiceAccount", group: "" },
  { id: "roles", kind: "Role", group: "rbac.authorization.k8s.io" },
  { id: "rolebindings", kind: "RoleBinding", group: "rbac.authorization.k8s.io" },
  { id: "clusterroles", kind: "ClusterRole", group: "rbac.authorization.k8s.io" },
  { id: "clusterrolebindings", kind: "ClusterRoleBinding", group: "rbac.authorization.k8s.io" },
  { id: "resourcequotas", kind: "ResourceQuota", group: "" },
  { id: "limitranges", kind: "LimitRange", group: "" },
  { id: "priorityclasses", kind: "PriorityClass", group: "scheduling.k8s.io" },
  { id: "horizontalpodautoscalers", kind: "HorizontalPodAutoscaler", group: "autoscaling" },
  { id: "poddisruptionbudgets", kind: "PodDisruptionBudget", group: "policy" },
];

const BUILTIN_GVK_SET = new Set(BUILTIN_GVK_ENTRIES.map((e) => `${e.group}\x1f${e.kind}`));

/** 是否为工作台已内置的 K8s 资源（应用 CRD 搜索时应排除，避免与内置列表重复）。 */
export function isWorkbenchBuiltinTarget(t: ResolvedAliasTarget): boolean {
  return BUILTIN_GVK_SET.has(`${t.group}\x1f${t.kind}`);
}

export function extensionStableKey(t: ResolvedAliasTarget): string {
  return `${t.group}\x1f${t.api_version}\x1f${t.kind}\x1f${t.plural}`;
}
