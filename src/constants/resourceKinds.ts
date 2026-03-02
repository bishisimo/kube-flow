/** 资源类型：Namespaces 与 Pods/Svc 等并列；含集群级 ClusterRole/ClusterRoleBinding。 */
export type ResourceKind =
  | "namespaces"
  | "nodes"
  | "pods"
  | "deployments"
  | "replicasets"
  | "statefulsets"
  | "daemonsets"
  | "jobs"
  | "cronjobs"
  | "services"
  | "endpoints"
  | "endpointslices"
  | "ingresses"
  | "ingressclasses"
  | "networkpolicies"
  | "configmaps"
  | "secrets"
  | "persistentvolumeclaims"
  | "persistentvolumes"
  | "storageclasses"
  | "serviceaccounts"
  | "roles"
  | "rolebindings"
  | "clusterroles"
  | "clusterrolebindings"
  | "resourcequotas"
  | "limitranges"
  | "priorityclasses"
  | "horizontalpodautoscalers"
  | "poddisruptionbudgets";

/** 9 类分组：工作负载、服务与网络、配置、存储、权限、集群、自动伸缩、其他 */
export const RESOURCE_GROUPS: {
  id: string;
  label: string;
  kinds: { id: ResourceKind; label: string }[];
}[] = [
  {
    id: "workloads",
    label: "工作负载",
    kinds: [
      { id: "pods", label: "Pod" },
      { id: "deployments", label: "Deployment" },
      { id: "replicasets", label: "ReplicaSet" },
      { id: "statefulsets", label: "StatefulSet" },
      { id: "daemonsets", label: "DaemonSet" },
      { id: "jobs", label: "Job" },
      { id: "cronjobs", label: "CronJob" },
    ],
  },
  {
    id: "networking",
    label: "服务与网络",
    kinds: [
      { id: "services", label: "Service" },
      { id: "endpoints", label: "Endpoints" },
      { id: "endpointslices", label: "EndpointSlice" },
      { id: "ingresses", label: "Ingress" },
      { id: "ingressclasses", label: "IngressClass" },
      { id: "networkpolicies", label: "NetworkPolicy" },
    ],
  },
  {
    id: "config",
    label: "配置",
    kinds: [
      { id: "configmaps", label: "ConfigMap" },
      { id: "secrets", label: "Secret" },
    ],
  },
  {
    id: "storage",
    label: "存储",
    kinds: [
      { id: "persistentvolumeclaims", label: "PersistentVolumeClaim" },
      { id: "persistentvolumes", label: "PersistentVolume" },
      { id: "storageclasses", label: "StorageClass" },
    ],
  },
  {
    id: "rbac",
    label: "权限与安全",
    kinds: [
      { id: "serviceaccounts", label: "ServiceAccount" },
      { id: "roles", label: "Role" },
      { id: "rolebindings", label: "RoleBinding" },
      { id: "clusterroles", label: "ClusterRole" },
      { id: "clusterrolebindings", label: "ClusterRoleBinding" },
    ],
  },
  {
    id: "cluster",
    label: "集群与资源",
    kinds: [
      { id: "namespaces", label: "Namespace" },
      { id: "nodes", label: "Node" },
      { id: "resourcequotas", label: "ResourceQuota" },
      { id: "limitranges", label: "LimitRange" },
      { id: "priorityclasses", label: "PriorityClass" },
    ],
  },
  {
    id: "autoscaling",
    label: "自动伸缩",
    kinds: [{ id: "horizontalpodautoscalers", label: "HorizontalPodAutoscaler" }],
  },
  {
    id: "others",
    label: "其他",
    kinds: [{ id: "poddisruptionbudgets", label: "PodDisruptionBudget" }],
  },
];

/** 扁平列表，供 Main.vue 等使用 */
export const RESOURCE_KINDS_FLAT = RESOURCE_GROUPS.flatMap((g) => g.kinds);
