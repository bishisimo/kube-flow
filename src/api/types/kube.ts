/**
 * K8s 资源相关类型定义。
 */

export interface NamespaceItem {
  name: string;
  creation_time?: string | null;
}

export interface NodeItem {
  name: string;
  status?: string | null;
  taint_count?: number | null;
  internal_ip?: string | null;
  cpu_total?: string | null;
  memory_total?: string | null;
  gpu_total?: string | null;
  cpu_requests?: string | null;
  memory_requests?: string | null;
  gpu_requests?: string | null;
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
  allow_volume_expansion?: boolean | null;
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

export interface ResourceRef {
  kind: string;
  namespace?: string | null;
  name: string;
  /** 图内对「无 API 具名、仅按 label 聚合」目标的键，与集群对象名无关 */
  set_id?: string | null;
}

export interface ResourceGraphNode {
  resource_ref: ResourceRef;
  depth: number;
  is_concrete: boolean;
  label_selector?: string | null;
  /** 人可读摘要（如 Pod 数），不当作资源名 */
  display_label?: string | null;
}

export interface ResourceGraphEdge {
  from: ResourceRef;
  to: ResourceRef;
  relation_type: string;
  label_selector?: string | null;
  to_display?: string | null;
}

export interface ResourceGraph {
  root: ResourceRef;
  nodes: ResourceGraphNode[];
  edges: ResourceGraphEdge[];
}

export interface DescribeResult {
  markdown: string;
}

export interface ContainerImagePatch {
  container_name: string;
  new_image: string;
}

export interface ResolvedAliasTarget {
  group: string;
  version: string;
  api_version: string;
  kind: string;
  plural: string;
  namespaced: boolean;
  /** apiserver 短名；单字符搜索仅匹配此列表 */
  short_names?: string[];
  singular?: string | null;
}

export interface ResourceAliasRefreshResult {
  resource_count: number;
  alias_key_count: number;
}

export interface DynamicCrdInstanceItem {
  name: string;
  namespace?: string | null;
  creation_time?: string | null;
}
