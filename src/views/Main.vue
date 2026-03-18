<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { useEnvStore } from "../stores/env";
import EnvBar from "../components/EnvBar.vue";
import { RESOURCE_GROUPS, RESOURCE_KINDS_FLAT, type ResourceKind } from "../constants/resourceKinds";

const RESOURCE_KINDS = RESOURCE_KINDS_FLAT;
import ResourceDetailDrawer from "../components/ResourceDetailDrawer.vue";
import ChangeImageModal from "../components/ChangeImageModal.vue";
import DeleteConfirmModal from "../components/DeleteConfirmModal.vue";
import { listen } from "@tauri-apps/api/event";
import { kubeDeleteResource, kubeRemoveClient } from "../api/kube";
import { useConnectionStore, isConnectionError } from "../stores/connection";
import { useSshAuthStore } from "../stores/sshAuth";
import { useShellStore } from "../stores/shell";
import {
  kubeListNamespaces,
  kubeStartWatch,
  kubeStopWatch,
  type NamespaceItem,
  kubeListNodes,
  kubeListPods,
  kubeListDeployments,
  kubeListReplicaSets,
  kubeListJobs,
  kubeListCronJobs,
  kubeListServices,
  kubeListStatefulSets,
  kubeListDaemonSets,
  kubeListConfigMaps,
  kubeListSecrets,
  kubeListServiceAccounts,
  kubeListPersistentVolumeClaims,
  kubeListPersistentVolumes,
  kubeListStorageClasses,
  kubeListEndpoints,
  kubeListEndpointSlices,
  kubeListIngresses,
  kubeListIngressClasses,
  kubeListNetworkPolicies,
  kubeListResourceQuotas,
  kubeListLimitRanges,
  kubeListPriorityClasses,
  kubeListHorizontalPodAutoscalers,
  kubeListPodDisruptionBudgets,
  kubeListRoles,
  kubeListRoleBindings,
  kubeListClusterRoles,
  kubeListClusterRoleBindings,
  type PodItem,
  type DeploymentItem,
  type ReplicaSetItem,
  type JobItem,
  type CronJobItem,
  type ServiceItem,
  type StatefulSetItem,
  type DaemonSetItem,
  type ConfigMapItem,
  type SecretItem,
  type ServiceAccountItem,
  type NodeItem,
  type PersistentVolumeClaimItem,
  type PersistentVolumeItem,
  type StorageClassItem,
  type EndpointsItem,
  type EndpointSliceItem,
  type IngressItem,
  type IngressClassItem,
  type NetworkPolicyItem,
  type ResourceQuotaItem,
  type LimitRangeItem,
  type PriorityClassItem,
  type HorizontalPodAutoscalerItem,
  type PodDisruptionBudgetItem,
  type RoleItem,
  type RoleBindingItem,
  type ClusterRoleItem,
  type ClusterRoleBindingItem,
} from "../api/kube";
import { defaultNamespace } from "../api/env";

const { openedEnvs, currentEnv, currentId, touchEnv, loadEnvironments, getEnvViewState, setEnvViewState } = useEnvStore();
const { pendingOpen, requestSwitchToShell } = useShellStore();
const {
  getProgress,
  getState,
  getError,
  setDisconnected,
  setConnecting,
  setConnected,
  setupConnectionProgressListener,
} = useConnectionStore();

const sshAuth = useSshAuthStore();

const ENV_BAR_COLLAPSED_KEY = "kube-flow:env-bar-collapsed";
const envBarCollapsed = ref(
  (() => {
    try {
      return localStorage.getItem(ENV_BAR_COLLAPSED_KEY) === "1";
    } catch {
      return true;
    }
  })()
);
function setEnvBarCollapsed(v: boolean) {
  envBarCollapsed.value = v;
  try {
    localStorage.setItem(ENV_BAR_COLLAPSED_KEY, v ? "1" : "0");
  } catch {}
}

const selectedNamespace = ref<string | null>(null);
const selectedKind = ref<ResourceKind>("namespaces");

/** 钻取来源：从某资源跳转到关联列表时保留，用于面包屑与侧栏点击时清除 */
const drillFrom = ref<{ kind: string; name: string; namespace: string | null } | null>(null);

/** 有效的 ResourceKind 集合，用于校验从存储恢复的值 */
const VALID_KINDS = new Set<string>(RESOURCE_KINDS.map((k) => k.id));

function restoreEnvViewState(envId: string) {
  const stored = getEnvViewState(envId);
  if (stored) {
    selectedNamespace.value = stored.namespace;
    selectedKind.value = (VALID_KINDS.has(stored.kind) ? stored.kind : "namespaces") as ResourceKind;
  } else {
    selectedNamespace.value = null;
    selectedKind.value = "namespaces";
  }
  drillFrom.value = null;
}

/** 选择资源类型并退出钻取（侧栏/下拉点击） */
function selectKindAndClearDrill(kind: ResourceKind) {
  selectedKind.value = kind;
  kindDropdownOpen.value = false;
  drillFrom.value = null;
  labelSelector.value = "";
  nameFilter.value = "";
  nodeFilter.value = "all";
  selectedRowKeys.value = new Set();
  batchDeleteMode.value = false;
}

/** 仅清除钻取上下文并刷新（面包屑点击 namespace 时） */
function clearDrillAndReload() {
  drillFrom.value = null;
  labelSelector.value = "";
  nameFilter.value = "";
  nodeFilter.value = "all";
  kindDropdownOpen.value = false;
  selectedRowKeys.value = new Set();
  batchDeleteMode.value = false;
  loadList();
}

/** 面包屑点击「资源名」：跳转到该类型资源列表并预填名称筛选，保留 drillFrom 以维持面包屑 */
function onBreadcrumbResourceNameClick() {
  if (!drillFrom.value) return;
  const kindId = API_KIND_TO_ID[drillFrom.value.kind] ?? "services";
  selectedKind.value = kindId;
  selectedNamespace.value = drillFrom.value.namespace;
  nameFilter.value = drillFrom.value.name;
  nodeFilter.value = "all";
  labelSelector.value = "";
  kindDropdownOpen.value = false;
  loadList();
}

/** 面包屑点击「资源类型」：跳转到该类型资源列表（全量） */
function onBreadcrumbKindClick() {
  if (!drillFrom.value) return;
  const kindId = API_KIND_TO_ID[drillFrom.value.kind] ?? "services";
  selectedKind.value = kindId;
  selectedNamespace.value = drillFrom.value.namespace;
  drillFrom.value = null;
  labelSelector.value = "";
  nameFilter.value = "";
  nodeFilter.value = "all";
  kindDropdownOpen.value = false;
  loadList();
}

function saveEnvViewState(envId: string) {
  setEnvViewState(envId, {
    namespace: selectedNamespace.value,
    kind: selectedKind.value,
  });
}

const listLoading = ref(false);
const listError = ref<string | null>(null);
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
const nsDropdownOpen = ref(false);
const kindDropdownOpen = ref(false);
const nsDropdownRef = ref<HTMLElement | null>(null);
const kindDropdownRef = ref<HTMLElement | null>(null);
const nsFilter = ref("");
const kindFilter = ref("");
/** 按名称筛选：前端过滤，支持模糊匹配（包含） */
const nameFilter = ref("");
/** 按 Node 筛选：仅在 Pods 视图生效，默认 all（不过滤） */
const nodeFilter = ref("all");
/** 按 label 筛选：传给 K8s API，格式如 app=nginx 或 env in (prod,staging) */
const labelSelector = ref("");
/** Watch 实时更新：开启后通过 Tauri 事件接收增量，仅部分 kind 支持 */
const watchEnabled = ref(true);
/** 排序：默认按创建时间倒序 */
const sortBy = ref<string>("creationTime");
const sortOrder = ref<"asc" | "desc">("desc");

/** 当前支持 Watch 的资源类型 */
const WATCH_SUPPORTED_KINDS: Set<ResourceKind> = new Set([
  "namespaces",
  "nodes",
  "pods",
  "deployments",
  "services",
]);

function onDocClick(e: MouseEvent) {
  const target = e.target as Node;
  if (nsDropdownOpen.value && nsDropdownRef.value && !nsDropdownRef.value.contains(target)) nsDropdownOpen.value = false;
  if (kindDropdownOpen.value && kindDropdownRef.value && !kindDropdownRef.value.contains(target)) kindDropdownOpen.value = false;
  if (actionMenuVisible.value && !actionMenuRef.value?.contains(target)) closeActionMenu();
}

const filteredNamespaceOptions = computed(() => {
  const q = nsFilter.value.trim().toLowerCase();
  if (!q) return namespaceOptions.value;
  return namespaceOptions.value.filter((n) => n.name.toLowerCase().includes(q));
});

/** 按分组过滤后的资源类型，供下拉使用 */
const filteredKindGroups = computed(() => {
  const q = kindFilter.value.trim().toLowerCase();
  if (!q) return RESOURCE_GROUPS;
  return RESOURCE_GROUPS.map((g) => ({
    ...g,
    kinds: g.kinds.filter((k) => k.label.toLowerCase().includes(q)),
  })).filter((g) => g.kinds.length > 0);
});

/** Pods 视图可选 Node 列表（来自当前 Pods 数据，去重并排序） */
const podNodeOptions = computed(() => {
  const values = new Set<string>();
  for (const p of pods.value) {
    const v = (p.node_name ?? "").trim();
    if (v) values.add(v);
  }
  return Array.from(values).sort((a, b) => a.localeCompare(b));
});

const selectedKindLabel = computed(() => RESOURCE_KINDS.find((k) => k.id === selectedKind.value)?.label ?? selectedKind.value);

/** API Kind（如 Service）到 ResourceKind id（如 services）的映射 */
const API_KIND_TO_ID: Record<string, ResourceKind> = Object.fromEntries(
  RESOURCE_KINDS.map((k) => [k.label, k.id])
) as Record<string, ResourceKind>;

/** 集群级资源，无需 namespace */
const CLUSTER_SCOPED_KINDS: Set<ResourceKind> = new Set([
  "namespaces",
  "nodes",
  "persistentvolumes",
  "storageclasses",
  "ingressclasses",
  "priorityclasses",
  "clusterroles",
  "clusterrolebindings",
]);

const selectedResource = ref<{
  kind: string;
  name: string;
  namespace: string | null;
} | null>(null);
const detailDrawerVisible = ref(false);
const detailDrawerInitialTab = ref<string | null>(null);
const changeImageModalVisible = ref(false);
const deleteConfirmVisible = ref(false);
const deleteConfirmResources = ref<{ kind: string; name: string; namespace: string | null }[]>([]);
const deleteConfirmDeleting = ref(false);
const deleteConfirmError = ref<string | null>(null);
const selectedRowKeys = ref<Set<string>>(new Set());
/** 批量删除模式：为 true 时显示复选框列 */
const batchDeleteMode = ref(false);

/** 支持修改镜像的资源类型 */
const IMAGE_PATCH_KINDS = new Set(["Deployment", "StatefulSet", "DaemonSet"]);

/** 操作菜单：点击行时显示，区分资源管理与资源钻取 */
const actionMenuVisible = ref(false);
const actionMenuPosition = ref({ x: 0, y: 0 });
const actionMenuRef = ref<HTMLElement | null>(null);

function selectResourceFromRow(row: Record<string, unknown>) {
  const name = row.name as string | undefined;
  if (!name) return null;
  const kindLabel = RESOURCE_KINDS.find((k) => k.id === selectedKind.value)?.label ?? "";
  if (!kindLabel) return null;
  const isCluster = CLUSTER_SCOPED_KINDS.has(selectedKind.value);
  return {
    kind: kindLabel,
    name,
    namespace: isCluster ? null : ((row.ns as string) ?? effectiveNamespace.value ?? null),
  };
}

/** 行唯一标识：集群级用 name，命名空间级用 ns/name */
function getRowKey(row: Record<string, unknown>): string {
  const name = row.name as string | undefined;
  if (!name) return "";
  const isCluster = CLUSTER_SCOPED_KINDS.has(selectedKind.value);
  return isCluster ? name : `${(row.ns as string) ?? effectiveNamespace.value ?? "default"}/${name}`;
}

function onRowContextMenu(row: Record<string, unknown>, e: MouseEvent) {
  const resource = selectResourceFromRow(row);
  if (!resource) return;
  selectedResource.value = resource;
  actionMenuPosition.value = { x: e.clientX, y: e.clientY };
  actionMenuVisible.value = true;
}

function onRowClick(row: Record<string, unknown>) {
  const resource = selectResourceFromRow(row);
  if (resource) {
    selectedResource.value = resource;
    detailDrawerInitialTab.value = null;
    detailDrawerVisible.value = true;
  }
}

function closeActionMenu() {
  actionMenuVisible.value = false;
}

function openResourceDetail() {
  detailDrawerInitialTab.value = null;
  detailDrawerVisible.value = true;
  closeActionMenu();
}

function openPodLogs() {
  detailDrawerInitialTab.value = "logs";
  detailDrawerVisible.value = true;
  closeActionMenu();
}

const SHELL_WORKLOAD_KINDS = new Set(["Pod", "Deployment", "StatefulSet", "DaemonSet"]);

function openPodShell() {
  const r = selectedResource.value;
  if (!r || !SHELL_WORKLOAD_KINDS.has(r.kind) || !currentId.value || !currentEnv.value) return;
  const ns = r.namespace ?? "default";
  if (r.kind === "Pod") {
    pendingOpen.value = {
      envId: currentId.value,
      envName: currentEnv.value.display_name,
      namespace: ns,
      podName: r.name,
    };
  } else {
    pendingOpen.value = {
      envId: currentId.value,
      envName: currentEnv.value.display_name,
      namespace: ns,
      workloadKind: r.kind,
      workloadName: r.name,
    };
  }
  requestSwitchToShell();
  closeActionMenu();
}

function openEditConfig() {
  detailDrawerInitialTab.value = "editConfig";
  detailDrawerVisible.value = true;
  closeActionMenu();
}

function openTopology() {
  detailDrawerInitialTab.value = "topology";
  detailDrawerVisible.value = true;
  closeActionMenu();
}

function openChangeImageModal() {
  changeImageModalVisible.value = true;
  closeActionMenu();
}

function openDeleteConfirm() {
  if (!selectedResource.value) return;
  deleteConfirmResources.value = [selectedResource.value];
  deleteConfirmError.value = null;
  deleteConfirmVisible.value = true;
  closeActionMenu();
}

function enterBatchDeleteMode() {
  batchDeleteMode.value = true;
  selectedRowKeys.value = new Set();
}

function exitBatchDeleteMode() {
  batchDeleteMode.value = false;
  selectedRowKeys.value = new Set();
}

function openBatchDeleteConfirm() {
  const kindLabel = RESOURCE_KINDS.find((k) => k.id === selectedKind.value)?.label ?? "";
  if (!kindLabel || selectedRowKeys.value.size === 0) return;
  const resources = tableRows.value
    .filter((row) => selectedRowKeys.value.has(getRowKey(row)))
    .map((row) => selectResourceFromRow(row))
    .filter((r): r is { kind: string; name: string; namespace: string | null } => r !== null);
  if (resources.length === 0) return;
  deleteConfirmResources.value = resources;
  deleteConfirmError.value = null;
  deleteConfirmVisible.value = true;
}

async function onDeleteConfirm() {
  const envId = currentId.value;
  if (!envId || deleteConfirmResources.value.length === 0) return;
  deleteConfirmDeleting.value = true;
  deleteConfirmError.value = null;
  const failed: string[] = [];
  for (const r of deleteConfirmResources.value) {
    try {
      await kubeDeleteResource(envId, r.kind, r.name, r.namespace);
    } catch (e) {
      failed.push(`${r.namespace ? `${r.namespace}/` : ""}${r.name}: ${e instanceof Error ? e.message : String(e)}`);
    }
  }
  deleteConfirmDeleting.value = false;
  if (failed.length > 0) {
    deleteConfirmError.value = failed.join("；");
    return;
  }
  deleteConfirmVisible.value = false;
  selectedRowKeys.value = new Set();
  batchDeleteMode.value = false;
  loadList();
}

function toggleRowSelection(row: Record<string, unknown>) {
  const key = getRowKey(row);
  if (!key) return;
  const next = new Set(selectedRowKeys.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  selectedRowKeys.value = next;
}

function toggleSelectAll() {
  const rows = tableRows.value as Record<string, unknown>[];
  const keys = rows.map(getRowKey).filter(Boolean);
  const allSelected = keys.length > 0 && keys.every((k) => selectedRowKeys.value.has(k));
  selectedRowKeys.value = allSelected ? new Set() : new Set(keys);
}

function closeDetailDrawer() {
  detailDrawerVisible.value = false;
  detailDrawerInitialTab.value = null;
  selectedResource.value = null;
}

function onTopologyNavigate(payload: {
  targetKind: string;
  namespace: string | null;
  labelSelector?: string | null;
  resourceName?: string | null;
}) {
  const source = selectedResource.value;
  if (!source) return;
  selectedKind.value = payload.targetKind as ResourceKind;
  selectedNamespace.value = payload.namespace;
  labelSelector.value = payload.labelSelector ?? "";
  nameFilter.value = payload.resourceName ?? "";
  drillFrom.value = { kind: source.kind, name: source.name, namespace: source.namespace };
  kindDropdownOpen.value = false;
  detailDrawerVisible.value = false;
  loadList();
}


/** 判断单元格是否可点击钻取 */
function isCellDrillable(colKey: string, row: Record<string, unknown>): boolean {
  if (colKey === "replicas" && row.labelSelector) return true;
  if (colKey === "roleRef" && row.roleRefName && row.roleRef !== "-") return true;
  if (colKey === "subjects" && Array.isArray(row.subjectsList) && (row.subjectsList as unknown[]).length > 0) return true;
  if (selectedKind.value === "persistentvolumeclaims") {
    if (colKey === "volume") {
      const v = row.volume;
      return typeof v === "string" && v !== "" && v !== "-";
    }
    if (colKey === "storageClass") {
      const s = row.storageClass;
      return typeof s === "string" && s !== "" && s !== "-";
    }
  }
  return false;
}

/** RoleBinding/ClusterRoleBinding roleRef 点击：跳转 Role 或 ClusterRole */
function onRoleRefClick(row: Record<string, unknown>) {
  const kind = row.roleRefKind as string;
  const name = row.roleRefName as string;
  if (!kind || !name) return;
  if (kind === "Role") {
    selectedKind.value = "roles";
    selectedNamespace.value = (row.ns as string) ?? effectiveNamespace.value;
    nameFilter.value = name;
  } else {
    selectedKind.value = "clusterroles";
    selectedNamespace.value = null;
    nameFilter.value = name;
  }
  labelSelector.value = "";
  drillFrom.value = {
    kind: selectedKind.value === "roles" ? "RoleBinding" : "ClusterRoleBinding",
    name: String(row.name),
    namespace: (row.ns as string) ?? null,
  };
  kindDropdownOpen.value = false;
  detailDrawerVisible.value = false;
  loadList();
}

function getSubjectsList(row: Record<string, unknown>): { kind: string; name: string; namespace?: string | null }[] {
  const list = row.subjectsList;
  return Array.isArray(list) ? (list as { kind: string; name: string; namespace?: string | null }[]) : [];
}

function getSubjectLabel(s: { kind: string; name: string; namespace?: string | null }, row: Record<string, unknown>): string {
  const ns = s.namespace ?? (row.ns as string) ?? "default";
  return `${ns}/${s.name}`;
}

/** RoleBinding/ClusterRoleBinding subject(SA) 点击：跳转 ServiceAccount */
function onSubjectClick(row: Record<string, unknown>, subject: { kind: string; name: string; namespace?: string | null }) {
  if (subject.kind !== "ServiceAccount") return;
  const sourceKind = selectedKind.value === "clusterrolebindings" ? "ClusterRoleBinding" : "RoleBinding";
  selectedKind.value = "serviceaccounts";
  selectedNamespace.value = subject.namespace ?? (row.ns as string) ?? effectiveNamespace.value;
  nameFilter.value = subject.name;
  labelSelector.value = "";
  drillFrom.value = { kind: sourceKind, name: String(row.name), namespace: (row.ns as string) ?? null };
  kindDropdownOpen.value = false;
  detailDrawerVisible.value = false;
  loadList();
}

/** PVC 单元格点击：volume 跳转 PV，storageClass 跳转 StorageClass */
function onPvcCellClick(row: Record<string, unknown>, colKey: string) {
  if (colKey === "volume") {
    const name = row.volume as string;
    if (!name || name === "-") return;
    selectedKind.value = "persistentvolumes";
    selectedNamespace.value = null;
    labelSelector.value = "";
    nameFilter.value = name;
    drillFrom.value = { kind: "PersistentVolumeClaim", name: String(row.name), namespace: (row.ns as string) ?? null };
  } else if (colKey === "storageClass") {
    const name = row.storageClass as string;
    if (!name || name === "-") return;
    selectedKind.value = "storageclasses";
    selectedNamespace.value = null;
    labelSelector.value = "";
    nameFilter.value = name;
    drillFrom.value = { kind: "PersistentVolumeClaim", name: String(row.name), namespace: (row.ns as string) ?? null };
  }
  kindDropdownOpen.value = false;
  detailDrawerVisible.value = false;
  loadList();
}

/** 副本数点击：跳转到 Pods 并筛选该 workload 管理的 Pod */
function onReplicasClick(row: Record<string, unknown>) {
  const ls = row.labelSelector as string | null | undefined;
  if (!ls || !row.name) return;
  const kindLabel = RESOURCE_KINDS.find((k) => k.id === selectedKind.value)?.label ?? "";
  if (!kindLabel) return;
  selectedKind.value = "pods";
  selectedNamespace.value = (row.ns as string) ?? effectiveNamespace.value;
  labelSelector.value = ls;
  nameFilter.value = "";
  drillFrom.value = { kind: kindLabel, name: String(row.name), namespace: (row.ns as string) ?? null };
  kindDropdownOpen.value = false;
  detailDrawerVisible.value = false;
  loadList();
}

/** Namespace 列表行点击：快速切换到该命名空间并切到 Pods 视图（双击保留） */
function onNamespaceRowClick(name: string) {
  selectedNamespace.value = name;
  selectedKind.value = "pods";
  nsDropdownOpen.value = false;
}

const effectiveNamespace = computed(() => {
  const ns = selectedNamespace.value;
  if (ns) return ns;
  return (currentEnv.value && defaultNamespace(currentEnv.value)) ?? "default";
});

const rawTableRows = computed(() => {
  switch (selectedKind.value) {
    case "namespaces":
      return namespaceOptions.value.map((n) => ({ name: n.name, creationTime: n.creation_time ?? "-" }));
    case "nodes":
      return nodes.value.map((n) => ({
        name: n.name,
        status: n.status ?? "-",
        internalIp: n.internal_ip ?? "-",
        creationTime: n.creation_time ?? "-",
      }));
    case "pods":
      return pods.value.map((p) => ({
        name: p.name,
        ns: p.namespace,
        phase: p.phase ?? "-",
        containerStatus: p.container_status ?? "-",
        node: p.node_name ?? "-",
        creationTime: p.creation_time ?? "-",
      }));
    case "deployments":
      return deployments.value.map((d) => ({
        name: d.name,
        ns: d.namespace,
        replicas: `${d.ready ?? 0}/${d.replicas ?? 0}`,
        creationTime: d.creation_time ?? "-",
        labelSelector: d.label_selector ?? null,
      }));
    case "services":
      return services.value.map((s) => ({
        name: s.name,
        ns: s.namespace,
        type: s.service_type ?? "-",
        clusterIp: s.cluster_ip ?? "-",
        ports: s.ports ?? "-",
        creationTime: s.creation_time ?? "-",
      }));
    case "statefulsets":
      return statefulSets.value.map((st) => ({
        name: st.name,
        ns: st.namespace,
        replicas: `${st.ready ?? 0}/${st.replicas ?? 0}`,
        creationTime: st.creation_time ?? "-",
        labelSelector: st.label_selector ?? null,
      }));
    case "configmaps":
      return configMaps.value.map((c) => ({
        name: c.name,
        ns: c.namespace,
        keys: c.keys != null ? String(c.keys) : "-",
        creationTime: c.creation_time ?? "-",
      }));
    case "secrets":
      return secrets.value.map((s) => ({
        name: s.name,
        ns: s.namespace,
        type: s.type_ ?? "-",
        keys: s.keys != null ? String(s.keys) : "-",
        creationTime: s.creation_time ?? "-",
      }));
    case "serviceaccounts":
      return serviceAccounts.value.map((s) => ({
        name: s.name,
        ns: s.namespace,
        creationTime: s.creation_time ?? "-",
      }));
    case "roles":
      return roles.value.map((r) => ({
        name: r.name,
        ns: r.namespace,
        creationTime: r.creation_time ?? "-",
      }));
    case "rolebindings":
      return roleBindings.value.map((r) => ({
        name: r.name,
        ns: r.namespace,
        roleRef: r.role_ref ?? "-",
        roleRefKind: r.role_ref_kind ?? null,
        roleRefName: r.role_ref_name ?? null,
        subjects: r.subjects != null ? String(r.subjects) : "-",
        subjectsList: r.subjects_list ?? null,
        creationTime: r.creation_time ?? "-",
      }));
    case "clusterroles":
      return clusterRoles.value.map((r) => ({
        name: r.name,
        creationTime: r.creation_time ?? "-",
      }));
    case "clusterrolebindings":
      return clusterRoleBindings.value.map((r) => ({
        name: r.name,
        roleRef: r.role_ref ?? "-",
        roleRefKind: r.role_ref_kind ?? null,
        roleRefName: r.role_ref_name ?? null,
        subjects: r.subjects != null ? String(r.subjects) : "-",
        subjectsList: r.subjects_list ?? null,
        creationTime: r.creation_time ?? "-",
      }));
    case "daemonsets":
      return daemonSets.value.map((d) => ({
        name: d.name,
        ns: d.namespace,
        replicas: `${d.ready ?? 0}/${d.desired ?? 0}`,
        creationTime: d.creation_time ?? "-",
        labelSelector: d.label_selector ?? null,
      }));
    case "persistentvolumeclaims":
      return persistentVolumeClaims.value.map((p) => ({
        name: p.name,
        ns: p.namespace,
        status: p.status ?? "-",
        volume: p.volume ?? "-",
        capacity: p.capacity ?? "-",
        storageClass: p.storage_class ?? "-",
        creationTime: p.creation_time ?? "-",
      }));
    case "persistentvolumes":
      return persistentVolumes.value.map((p) => ({
        name: p.name,
        capacity: p.capacity ?? "-",
        status: p.status ?? "-",
        creationTime: p.creation_time ?? "-",
      }));
    case "storageclasses":
      return storageClasses.value.map((s) => ({
        name: s.name,
        provisioner: s.provisioner ?? "-",
        creationTime: s.creation_time ?? "-",
      }));
    case "endpoints":
      return endpoints.value.map((e) => ({
        name: e.name,
        ns: e.namespace,
        subsets: e.subsets != null ? String(e.subsets) : "-",
        creationTime: e.creation_time ?? "-",
      }));
    case "endpointslices":
      return endpointSlices.value.map((e) => ({
        name: e.name,
        ns: e.namespace,
        addressType: e.address_type ?? "-",
        endpoints: e.endpoints != null ? String(e.endpoints) : "-",
        creationTime: e.creation_time ?? "-",
      }));
    case "replicasets":
      return replicaSets.value.map((r) => ({
        name: r.name,
        ns: r.namespace,
        replicas: `${r.ready ?? 0}/${r.replicas ?? 0}`,
        creationTime: r.creation_time ?? "-",
        labelSelector: r.label_selector ?? null,
      }));
    case "jobs":
      return jobs.value.map((j) => ({
        name: j.name,
        ns: j.namespace,
        completions: j.completions ?? "-",
        duration: j.duration ?? "-",
        creationTime: j.creation_time ?? "-",
      }));
    case "cronjobs":
      return cronJobs.value.map((c) => ({
        name: c.name,
        ns: c.namespace,
        schedule: c.schedule ?? "-",
        lastSchedule: c.last_schedule ?? "-",
        creationTime: c.creation_time ?? "-",
      }));
    case "ingresses":
      return ingresses.value.map((i) => ({
        name: i.name,
        ns: i.namespace,
        class: i.class ?? "-",
        hosts: i.hosts ?? "-",
        creationTime: i.creation_time ?? "-",
      }));
    case "ingressclasses":
      return ingressClasses.value.map((i) => ({
        name: i.name,
        controller: i.controller ?? "-",
        creationTime: i.creation_time ?? "-",
      }));
    case "networkpolicies":
      return networkPolicies.value.map((n) => ({
        name: n.name,
        ns: n.namespace,
        creationTime: n.creation_time ?? "-",
      }));
    case "resourcequotas":
      return resourceQuotas.value.map((r) => ({
        name: r.name,
        ns: r.namespace,
        creationTime: r.creation_time ?? "-",
      }));
    case "limitranges":
      return limitRanges.value.map((l) => ({
        name: l.name,
        ns: l.namespace,
        creationTime: l.creation_time ?? "-",
      }));
    case "priorityclasses":
      return priorityClasses.value.map((p) => ({
        name: p.name,
        value: p.value != null ? String(p.value) : "-",
        creationTime: p.creation_time ?? "-",
      }));
    case "horizontalpodautoscalers":
      return horizontalPodAutoscalers.value.map((h) => ({
        name: h.name,
        ns: h.namespace,
        reference: h.reference ?? "-",
        replicas: h.replicas ?? "-",
        creationTime: h.creation_time ?? "-",
      }));
    case "poddisruptionbudgets":
      return podDisruptionBudgets.value.map((p) => ({
        name: p.name,
        ns: p.namespace,
        minAvailable: p.min_available ?? "-",
        maxUnavailable: p.max_unavailable ?? "-",
        allowedDisruptions: p.allowed_disruptions ?? "-",
        creationTime: p.creation_time ?? "-",
      }));
    default:
      return [];
  }
});

/** 可排序的列：名称、创建时间、状态（phase/status）、副本等 */
const SORTABLE_KEYS = new Set(["name", "creationTime", "phase", "status", "replicas", "value"]);

function compareForSort(a: unknown, b: unknown, order: "asc" | "desc"): number {
  const va = a == null || a === "-" ? "" : String(a);
  const vb = b == null || b === "-" ? "" : String(b);
  const cmp = va.localeCompare(vb, undefined, { numeric: true });
  return order === "asc" ? cmp : -cmp;
}

function sortRows<T extends Record<string, unknown>>(rows: T[], by: string, order: "asc" | "desc"): T[] {
  if (!by || !rows.length) return rows;
  return [...rows].sort((a, b) => {
    const va = a[by];
    const vb = b[by];
    if (by === "creationTime") {
      const da = va === "-" || !va ? "" : String(va);
      const db = vb === "-" || !vb ? "" : String(vb);
      const cmp = da.localeCompare(db);
      return order === "asc" ? cmp : -cmp;
    }
    return compareForSort(va, vb, order);
  });
}

function onSortColumn(key: string) {
  if (!SORTABLE_KEYS.has(key)) return;
  if (sortBy.value === key) {
    sortOrder.value = sortOrder.value === "asc" ? "desc" : "asc";
  } else {
    sortBy.value = key;
    sortOrder.value = key === "creationTime" ? "desc" : "asc";
  }
}

/** 应用名称/Node 筛选与排序 */
const tableRows = computed(() => {
  let raw = rawTableRows.value as Record<string, unknown>[];
  const q = nameFilter.value.trim().toLowerCase();
  if (q) raw = raw.filter((r) => String(r.name ?? "").toLowerCase().includes(q));
  if (selectedKind.value === "pods" && nodeFilter.value !== "all") {
    raw = raw.filter((r) => String(r.node ?? "") === nodeFilter.value);
  }
  const by = sortBy.value;
  const order = sortOrder.value;
  if (by && tableColumns.value.some((c) => c.key === by)) {
    return sortRows(raw, by, order);
  }
  return sortRows(raw, "creationTime", "desc");
});

const tableColumns = computed(() => {
  switch (selectedKind.value) {
    case "namespaces":
      return [
        { key: "name", label: "名称" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "nodes":
      return [
        { key: "name", label: "名称" },
        { key: "status", label: "状态" },
        { key: "internalIp", label: "Internal IP" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "pods":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "phase", label: "状态" },
        { key: "containerStatus", label: "容器启动" },
        { key: "node", label: "Node" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "deployments":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "replicas", label: "副本" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "services":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "type", label: "Type" },
        { key: "clusterIp", label: "Cluster IP" },
        { key: "ports", label: "Ports" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "statefulsets":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "replicas", label: "副本" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "configmaps":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "keys", label: "Keys" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "secrets":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "type", label: "Type" },
        { key: "keys", label: "Keys" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "serviceaccounts":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "roles":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "rolebindings":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "roleRef", label: "Role" },
        { key: "subjects", label: "Subjects" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "clusterroles":
      return [
        { key: "name", label: "名称" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "clusterrolebindings":
      return [
        { key: "name", label: "名称" },
        { key: "roleRef", label: "Role" },
        { key: "subjects", label: "Subjects" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "daemonsets":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "replicas", label: "Ready/Desired" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "persistentvolumeclaims":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "status", label: "Status" },
        { key: "volume", label: "Volume" },
        { key: "capacity", label: "Capacity" },
        { key: "storageClass", label: "StorageClass" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "persistentvolumes":
      return [
        { key: "name", label: "名称" },
        { key: "capacity", label: "Capacity" },
        { key: "status", label: "Status" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "storageclasses":
      return [
        { key: "name", label: "名称" },
        { key: "provisioner", label: "Provisioner" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "endpoints":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "subsets", label: "Subsets" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "endpointslices":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "addressType", label: "AddressType" },
        { key: "endpoints", label: "Endpoints" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "replicasets":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "replicas", label: "副本" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "jobs":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "completions", label: "完成" },
        { key: "duration", label: "耗时" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "cronjobs":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "schedule", label: "Schedule" },
        { key: "lastSchedule", label: "上次调度" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "ingresses":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "class", label: "Class" },
        { key: "hosts", label: "Hosts" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "ingressclasses":
      return [
        { key: "name", label: "名称" },
        { key: "controller", label: "Controller" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "networkpolicies":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "resourcequotas":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "limitranges":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "priorityclasses":
      return [
        { key: "name", label: "名称" },
        { key: "value", label: "Value" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "horizontalpodautoscalers":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "reference", label: "Target" },
        { key: "replicas", label: "副本" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "poddisruptionbudgets":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "minAvailable", label: "Min Available" },
        { key: "maxUnavailable", label: "Max Unavailable" },
        { key: "allowedDisruptions", label: "Allowed" },
        { key: "creationTime", label: "创建时间" },
      ];
    default:
      return [];
  }
});

async function loadList() {
  const id = currentId.value;
  if (!id) {
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
    return;
  }
  listLoading.value = true;
  listError.value = null;
  if (getState(id) !== "connected") setConnecting(id);
  const ns = selectedNamespace.value || (currentEnv.value && defaultNamespace(currentEnv.value)) || undefined;
  const labelSel = labelSelector.value.trim() || null;
  try {
    await touchEnv(id);
    await loadEnvironments();
    namespaceOptions.value = await kubeListNamespaces(id, labelSel);
    switch (selectedKind.value) {
      case "namespaces":
        break;
      case "nodes":
        nodes.value = await kubeListNodes(id, labelSel);
        break;
      case "pods":
        pods.value = await kubeListPods(id, ns ?? null, labelSel);
        break;
      case "deployments":
        deployments.value = await kubeListDeployments(id, ns ?? null, labelSel);
        break;
      case "services":
        services.value = await kubeListServices(id, ns ?? null, labelSel);
        break;
      case "statefulsets":
        statefulSets.value = await kubeListStatefulSets(id, ns ?? null, labelSel);
        break;
      case "configmaps":
        configMaps.value = await kubeListConfigMaps(id, ns ?? null, labelSel);
        break;
      case "secrets":
        secrets.value = await kubeListSecrets(id, ns ?? null, labelSel);
        break;
      case "serviceaccounts":
        serviceAccounts.value = await kubeListServiceAccounts(id, ns ?? null, labelSel);
        break;
      case "roles":
        roles.value = await kubeListRoles(id, ns ?? null, labelSel);
        break;
      case "rolebindings":
        roleBindings.value = await kubeListRoleBindings(id, ns ?? null, labelSel);
        break;
      case "clusterroles":
        clusterRoles.value = await kubeListClusterRoles(id, labelSel);
        break;
      case "clusterrolebindings":
        clusterRoleBindings.value = await kubeListClusterRoleBindings(id, labelSel);
        break;
      case "daemonsets":
        daemonSets.value = await kubeListDaemonSets(id, ns ?? null, labelSel);
        break;
      case "persistentvolumeclaims":
        persistentVolumeClaims.value = await kubeListPersistentVolumeClaims(id, ns ?? null, labelSel);
        break;
      case "persistentvolumes":
        persistentVolumes.value = await kubeListPersistentVolumes(id, labelSel);
        break;
      case "storageclasses":
        storageClasses.value = await kubeListStorageClasses(id, labelSel);
        break;
      case "endpoints":
        endpoints.value = await kubeListEndpoints(id, ns ?? null, labelSel);
        break;
      case "endpointslices":
        endpointSlices.value = await kubeListEndpointSlices(id, ns ?? null, labelSel);
        break;
      case "replicasets":
        replicaSets.value = await kubeListReplicaSets(id, ns ?? null, labelSel);
        break;
      case "jobs":
        jobs.value = await kubeListJobs(id, ns ?? null, labelSel);
        break;
      case "cronjobs":
        cronJobs.value = await kubeListCronJobs(id, ns ?? null, labelSel);
        break;
      case "ingresses":
        ingresses.value = await kubeListIngresses(id, ns ?? null, labelSel);
        break;
      case "ingressclasses":
        ingressClasses.value = await kubeListIngressClasses(id, labelSel);
        break;
      case "networkpolicies":
        networkPolicies.value = await kubeListNetworkPolicies(id, ns ?? null, labelSel);
        break;
      case "resourcequotas":
        resourceQuotas.value = await kubeListResourceQuotas(id, ns ?? null, labelSel);
        break;
      case "limitranges":
        limitRanges.value = await kubeListLimitRanges(id, ns ?? null, labelSel);
        break;
      case "priorityclasses":
        priorityClasses.value = await kubeListPriorityClasses(id, labelSel);
        break;
      case "horizontalpodautoscalers":
        horizontalPodAutoscalers.value = await kubeListHorizontalPodAutoscalers(id, ns ?? null, labelSel);
        break;
      case "poddisruptionbudgets":
        podDisruptionBudgets.value = await kubeListPodDisruptionBudgets(id, ns ?? null, labelSel);
        break;
    }
    setConnected(id);
  } catch (e: unknown) {
    const msg = extractErrorMessage(e);
    // SSH 认证错误：弹出密码输入框，用户输入后可重试
    const isAuthRequired = await sshAuth.checkAndHandle(msg, () => {
      loadList();
    });
    if (isAuthRequired) {
      setConnecting(id);
      return;
    }
    listError.value = msg;
    if (id && isConnectionError(msg)) {
      setDisconnected(id, msg);
    }
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
  } finally {
    listLoading.value = false;
  }
}

function extractErrorMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (e && typeof e === "object" && "message" in e && typeof (e as { message: unknown }).message === "string") {
    return (e as { message: string }).message;
  }
  return String(e);
}

function dismissError() {
  listError.value = null;
}

async function handleReconnect(envId: string) {
  await kubeRemoveClient(envId);
  setConnecting(envId);
  if (currentId.value === envId) {
    loadList();
  }
}

onMounted(() => {
  const id = currentId.value;
  if (id) restoreEnvViewState(id);
  loadList();
  document.addEventListener("click", onDocClick);
});
onUnmounted(() => {
  document.removeEventListener("click", onDocClick);
});
watch(selectedKind, () => {
  sortBy.value = "creationTime";
  sortOrder.value = "desc";
  nodeFilter.value = "all";
});
watch(currentId, (id) => {
  if (id) restoreEnvViewState(id);
});
watch([selectedNamespace, selectedKind], () => {
  const id = currentId.value;
  if (id) saveEnvViewState(id);
});
watch(
  [currentId, selectedNamespace, selectedKind],
  () => {
    const id = currentId.value;
    if (watchEnabled.value && WATCH_SUPPORTED_KINDS.has(selectedKind.value)) {
      applyWatch();
    } else {
      if (id) kubeStopWatch(id).catch(() => {});
      loadList();
    }
  },
  { immediate: true }
);

function applyWatch() {
  const id = currentId.value;
  if (!id) return;
  kubeStopWatch(id).catch(() => {});
  if (watchEnabled.value && WATCH_SUPPORTED_KINDS.has(selectedKind.value)) {
    const ns =
      selectedKind.value === "namespaces" || selectedKind.value === "nodes"
        ? null
        : effectiveNamespace.value;
    kubeStartWatch(id, selectedKind.value, ns, labelSelector.value.trim() || null).catch(async (e) => {
      const msg = extractErrorMessage(e);
      const isAuthRequired = await sshAuth.checkAndHandle(msg, () => applyWatch());
      if (isAuthRequired) {
        setConnecting(id);
        return;
      }
      listError.value = msg;
      if (isConnectionError(msg)) setDisconnected(id, msg);
    });
  }
}

watch(watchEnabled, () => {
  const id = currentId.value;
  if (!id) return;
  if (watchEnabled.value) {
    if (WATCH_SUPPORTED_KINDS.has(selectedKind.value)) {
      applyWatch();
    } else {
      loadList();
    }
  } else {
    kubeStopWatch(id).catch(() => {});
    loadList();
  }
});


let unlistenWatch: (() => void) | null = null;
let unlistenConnection: (() => void) | null = null;
onMounted(async () => {
  loadList();
  document.addEventListener("click", onDocClick);
  unlistenConnection = await setupConnectionProgressListener();
  unlistenWatch = await listen<{ kind?: string; items?: unknown[]; error?: string }>(
    "resource-watch-update",
    (ev) => {
      const payload = ev.payload;
      if (payload?.error) {
        listError.value = payload.error;
        const id = currentId.value;
        if (id && isConnectionError(payload.error)) setDisconnected(id, payload.error);
        return;
      }
      listError.value = null;
      listLoading.value = false;
      const kind = payload?.kind;
      const items = payload?.items ?? [];
      if (kind === "namespaces") namespaceOptions.value = items as NamespaceItem[];
      else if (kind === "nodes") nodes.value = items as NodeItem[];
      else if (kind === "pods") pods.value = items as PodItem[];
      else if (kind === "deployments") deployments.value = items as DeploymentItem[];
      else if (kind === "services") services.value = items as ServiceItem[];
    }
  );
});
onUnmounted(() => {
  document.removeEventListener("click", onDocClick);
  unlistenConnection?.();
  unlistenWatch?.();
  if (currentId.value) kubeStopWatch(currentId.value).catch(() => {});
});
</script>

<template>
  <div class="main-layout">
    <template v-if="openedEnvs.length">
      <EnvBar
        :collapsed="envBarCollapsed"
        :on-reconnect="handleReconnect"
        @toggle="setEnvBarCollapsed(!envBarCollapsed)"
      />
      <div class="content">
        <nav v-if="currentId" class="breadcrumb-bar" aria-label="导航">
          <template v-if="drillFrom">
            <button type="button" class="breadcrumb-seg" @click="selectedNamespace = drillFrom!.namespace; clearDrillAndReload()">
              {{ drillFrom!.namespace || "default" }}
            </button>
            <span class="breadcrumb-sep">›</span>
            <button type="button" class="breadcrumb-seg" @click="onBreadcrumbKindClick()">
              {{ drillFrom!.kind }}
            </button>
            <span class="breadcrumb-sep">›</span>
            <button
              v-if="(API_KIND_TO_ID[drillFrom!.kind] ?? 'services') !== selectedKind"
              type="button"
              class="breadcrumb-seg"
              @click="onBreadcrumbResourceNameClick()"
            >
              {{ drillFrom!.name }}
            </button>
            <template v-if="(API_KIND_TO_ID[drillFrom!.kind] ?? 'services') !== selectedKind">
              <span class="breadcrumb-sep">›</span>
              <span class="breadcrumb-seg breadcrumb-current">{{ selectedKindLabel }}</span>
            </template>
            <span v-else class="breadcrumb-seg breadcrumb-current">{{ drillFrom!.name }}</span>
          </template>
          <template v-else>
            <span class="breadcrumb-seg breadcrumb-current">{{ effectiveNamespace }}</span>
            <span class="breadcrumb-sep">›</span>
            <span class="breadcrumb-seg breadcrumb-current">{{ selectedKindLabel }}</span>
          </template>
        </nav>
        <header class="toolbar">
          <span v-if="currentEnv" class="env-name">{{ currentEnv.display_name }}</span>
          <div v-if="currentId" ref="nsDropdownRef" class="combobox-wrap">
            <button
              type="button"
              class="combobox-trigger"
              :class="{ open: nsDropdownOpen }"
              title="命名空间：输入筛选后选择"
              @click="nsDropdownOpen = !nsDropdownOpen; if (nsDropdownOpen) { nsFilter = ''; kindDropdownOpen = false }"
            >
              <span class="combobox-label">NS</span>
              <span class="combobox-value">{{ effectiveNamespace }}</span>
              <span class="combobox-arrow">▼</span>
            </button>
            <div v-show="nsDropdownOpen" class="combobox-menu">
              <input
                v-model="nsFilter"
                type="text"
                class="combobox-input"
                placeholder="输入筛选…"
                autocomplete="off"
              />
              <button type="button" class="combobox-item" :class="{ active: selectedNamespace === null }" @click="selectedNamespace = null; nsDropdownOpen = false">
                默认
              </button>
              <button
                v-for="n in filteredNamespaceOptions"
                :key="n.name"
                type="button"
                class="combobox-item"
                :class="{ active: selectedNamespace === n.name }"
                @click="selectedNamespace = n.name; nsDropdownOpen = false"
              >
                {{ n.name }}
              </button>
            </div>
          </div>
          <div ref="kindDropdownRef" class="combobox-wrap">
            <button
              type="button"
              class="combobox-trigger"
              :class="{ open: kindDropdownOpen }"
              title="资源类型：输入筛选后选择"
              @click="kindDropdownOpen = !kindDropdownOpen; if (kindDropdownOpen) { kindFilter = ''; nsDropdownOpen = false }"
            >
              <span class="combobox-label">资源</span>
              <span class="combobox-value">{{ selectedKindLabel }}</span>
              <span class="combobox-arrow">▼</span>
            </button>
            <div v-show="kindDropdownOpen" class="combobox-menu combobox-menu-grouped">
              <input
                v-model="kindFilter"
                type="text"
                class="combobox-input"
                placeholder="输入筛选…"
                autocomplete="off"
              />
              <template v-for="group in filteredKindGroups" :key="group.id">
                <div class="combobox-group-label">{{ group.label }}</div>
                <button
                  v-for="k in group.kinds"
                  :key="k.id"
                  type="button"
                  class="combobox-item"
                  :class="{ active: selectedKind === k.id }"
                  @click="selectKindAndClearDrill(k.id)"
                >
                  {{ k.label }}
                </button>
              </template>
            </div>
          </div>
          <input
            v-if="currentId"
            v-model="nameFilter"
            type="text"
            class="filter-input"
            placeholder="按名称筛选…"
            autocomplete="off"
            title="按名称包含匹配（前端过滤）"
          />
          <select
            v-if="currentId && selectedKind === 'pods'"
            v-model="nodeFilter"
            class="filter-input"
            title="按 Node 选项筛选"
          >
            <option value="all">Node: All</option>
            <option v-for="node in podNodeOptions" :key="node" :value="node">
              Node: {{ node }}
            </option>
          </select>
          <input
            v-if="currentId"
            v-model="labelSelector"
            type="text"
            class="filter-input filter-input-label"
            placeholder="Label 筛选，如 app=nginx"
            autocomplete="off"
            title="K8s label selector，如 app=nginx 或 env in (prod,staging)"
            @keyup.enter="watchEnabled && WATCH_SUPPORTED_KINDS.has(selectedKind) ? applyWatch() : loadList()"
          />
          <button
            v-if="currentId && WATCH_SUPPORTED_KINDS.has(selectedKind)"
            type="button"
            class="btn-watch"
            :class="{ active: watchEnabled }"
            :title="watchEnabled ? '关闭 Watch 实时更新' : '开启 Watch 实时更新'"
            @click="watchEnabled = !watchEnabled"
          >
            {{ watchEnabled ? "Watch 开" : "Watch" }}
          </button>
          <button type="button" class="btn-refresh" :disabled="listLoading" @click="loadList">
            {{ listLoading ? "刷新中…" : "刷新" }}
          </button>
          <template v-if="currentId">
            <button
              v-if="!batchDeleteMode"
              type="button"
              class="btn-secondary-outline"
              @click="enterBatchDeleteMode"
            >
              批量删除
            </button>
            <template v-else>
              <button type="button" class="btn-secondary-outline" @click="exitBatchDeleteMode">
                取消
              </button>
              <button
                type="button"
                class="btn-danger-outline"
                :disabled="selectedRowKeys.size === 0"
                @click="openBatchDeleteConfirm"
              >
                删除选中 ({{ selectedRowKeys.size }})
              </button>
            </template>
          </template>
        </header>
        <div v-if="currentId && getState(currentId) === 'disconnected'" class="disconnect-banner">
          <span class="disconnect-text">连接已断开</span>
          <span class="disconnect-detail">{{ getError(currentId) }}</span>
          <button type="button" class="btn-reconnect" @click="handleReconnect(currentId)">
            重连
          </button>
        </div>
        <div
          v-else-if="currentId && getState(currentId) === 'connecting' && getProgress(currentId)"
          class="connection-stepper"
        >
          <div class="stepper-title">连接中：{{ getProgress(currentId)?.stage_label }}</div>
          <div v-if="getProgress(currentId)?.detail" class="stepper-detail">
            {{ getProgress(currentId)?.detail }}
          </div>
        </div>
        <div v-else-if="listError" class="error-banner">
          {{ listError }}
          <button
            v-if="currentId && isConnectionError(listError)"
            type="button"
            class="btn-reconnect"
            @click="handleReconnect(currentId)"
          >
            重连
          </button>
          <button type="button" class="error-dismiss" @click="dismissError">关闭</button>
        </div>
        <div v-else-if="listLoading" class="loading-state">加载中…</div>
        <div v-else class="table-wrap">
          <table class="resource-table">
            <thead>
              <tr>
                <th v-if="batchDeleteMode" class="col-checkbox">
                  <input
                    type="checkbox"
                    :checked="tableRows.length > 0 && tableRows.every((r) => selectedRowKeys.has(getRowKey(r)))"
                    :indeterminate="selectedRowKeys.size > 0 && selectedRowKeys.size < tableRows.length"
                    @change="toggleSelectAll"
                  />
                </th>
                <th
                  v-for="col in tableColumns"
                  :key="col.key"
                  :class="{ sortable: SORTABLE_KEYS.has(col.key) }"
                  @click="SORTABLE_KEYS.has(col.key) && onSortColumn(col.key)"
                >
                  {{ col.label }}
                  <span
                    v-if="SORTABLE_KEYS.has(col.key) && sortBy === col.key"
                    class="sort-indicator"
                  >
                    {{ sortOrder === "asc" ? "↑" : "↓" }}
                  </span>
                </th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="(row, i) in tableRows"
                :key="i"
                class="row-clickable"
                @click="onRowClick(row)"
                @contextmenu.prevent="onRowContextMenu(row, $event)"
                @dblclick="selectedKind === 'namespaces' && row.name && onNamespaceRowClick(String(row.name))"
              >
                <td v-if="batchDeleteMode" class="col-checkbox" @click.stop>
                  <input
                    type="checkbox"
                    :checked="selectedRowKeys.has(getRowKey(row))"
                    @change="toggleRowSelection(row)"
                  />
                </td>
                <td
                  v-for="col in tableColumns"
                  :key="col.key"
                  :class="{ 'cell-drillable': isCellDrillable(col.key, row) && col.key !== 'subjects' }"
                  @click="
                    (e) => {
                      if (isCellDrillable(col.key, row) && col.key !== 'subjects') {
                        e.stopPropagation();
                        if (col.key === 'replicas') onReplicasClick(row);
                        else if (col.key === 'roleRef') onRoleRefClick(row);
                        else onPvcCellClick(row, col.key);
                      }
                    }
                  "
                >
                  <template v-if="col.key === 'subjects' && getSubjectsList(row).length > 0">
                    <template v-for="(s, i) in getSubjectsList(row)" :key="i">
                      <span
                        class="cell-link"
                        @click="(e: MouseEvent) => { e.stopPropagation(); onSubjectClick(row, s); }"
                      >
                        {{ getSubjectLabel(s, row) }}
                      </span>
                      <span v-if="i < getSubjectsList(row).length - 1">, </span>
                    </template>
                  </template>
                  <template v-else>
                    {{ row[col.key as keyof typeof row] }}
                  </template>
                </td>
              </tr>
            </tbody>
          </table>
          <p v-if="!tableRows.length" class="empty-table">暂无资源</p>
        </div>
      </div>
    </template>
    <div v-else class="empty-state">
      <p>暂无打开的环境</p>
      <p class="empty-desc">请先在「环境管理」中打开至少一个环境。</p>
    </div>

    <!-- 资源操作菜单：资源管理 vs 资源钻取 -->
    <Teleport to="body">
      <div
        v-if="actionMenuVisible"
        class="action-menu-backdrop"
        @click="closeActionMenu"
      >
        <div
          ref="actionMenuRef"
          class="action-menu-overlay"
          :style="{ left: actionMenuPosition.x + 'px', top: actionMenuPosition.y + 'px' }"
          role="menu"
          @click.stop
        >
        <div class="action-menu-section">
          <div class="action-menu-section-title">资源管理</div>
          <button type="button" class="action-menu-item" @click="openResourceDetail">查看详情</button>
          <button
            v-if="selectedResource && selectedResource.kind === 'Pod'"
            type="button"
            class="action-menu-item"
            @click="openPodLogs"
          >
            查看日志
          </button>
          <button
            v-if="selectedResource && SHELL_WORKLOAD_KINDS.has(selectedResource.kind)"
            type="button"
            class="action-menu-item"
            @click="openPodShell"
          >
            打开 Shell
          </button>
          <button
            v-if="selectedResource && (selectedResource.kind === 'ConfigMap' || selectedResource.kind === 'Secret')"
            type="button"
            class="action-menu-item"
            @click="openEditConfig"
          >
            修改配置
          </button>
          <button
            v-if="selectedResource && IMAGE_PATCH_KINDS.has(selectedResource.kind)"
            type="button"
            class="action-menu-item"
            @click="openChangeImageModal"
          >
            修改镜像
          </button>
          <button 
            type="button" 
            class="action-menu-item" 
            @click="openTopology"
          >
            关联资源
          </button>
          <button type="button" class="action-menu-item action-menu-item-danger" @click="openDeleteConfirm">
            删除
          </button>
        </div>
        </div>
      </div>
    </Teleport>

    <!-- 资源详情抽屉（YAML） -->
    <ResourceDetailDrawer
      :visible="detailDrawerVisible"
      :env-id="currentId"
      :resource="selectedResource"
      :initial-tab="detailDrawerInitialTab"
      @close="closeDetailDrawer"
      @navigate="onTopologyNavigate"
    />

    <ChangeImageModal
      :visible="changeImageModalVisible"
      :env-id="currentId"
      :resource="selectedResource"
      @close="changeImageModalVisible = false"
      @success="loadList"
    />

    <DeleteConfirmModal
      :visible="deleteConfirmVisible"
      :resources="deleteConfirmResources"
      :deleting="deleteConfirmDeleting"
      :error="deleteConfirmError"
      @close="deleteConfirmVisible = false"
      @confirm="onDeleteConfirm"
    />

    <!-- 加载失败弹窗（含 SSH 隧道等错误） -->
    <Teleport to="body">
      <div v-if="listError" class="error-modal-overlay" @click.self="dismissError">
        <div class="error-modal" role="alertdialog" aria-labelledby="error-modal-title">
          <h2 id="error-modal-title" class="error-modal-title">加载失败</h2>
          <p class="error-modal-message">{{ listError }}</p>
          <div class="error-modal-actions">
            <button type="button" class="btn-primary" @click="dismissError">确定</button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.main-layout {
  display: flex;
  flex-direction: row;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.content {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
}
.toolbar {
  padding: 0.5rem 1rem;
  border-bottom: 1px solid var(--border-color, #e2e8f0);
  display: flex;
  align-items: center;
  gap: 1rem;
  flex-shrink: 0;
  background: #fff;
}
.env-name {
  font-weight: 500;
  font-size: 0.875rem;
}
.breadcrumb-bar {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  padding: 0.5rem 1rem;
  font-size: 0.8125rem;
  color: #64748b;
  background: #f8fafc;
  border-bottom: 1px solid #e2e8f0;
  flex-shrink: 0;
}
.breadcrumb-seg {
  background: none;
  border: none;
  padding: 0.2rem 0.4rem;
  border-radius: 4px;
  cursor: pointer;
  color: inherit;
  font: inherit;
}
.breadcrumb-seg:hover:not(.breadcrumb-current) {
  background: #e2e8f0;
  color: #334155;
}
.breadcrumb-seg.breadcrumb-current {
  color: #0f172a;
  font-weight: 500;
  cursor: default;
}
.breadcrumb-sep {
  color: #cbd5e1;
  user-select: none;
}
.action-menu-backdrop {
  position: fixed;
  inset: 0;
  z-index: 999;
}
.action-menu-overlay {
  position: fixed;
  z-index: 1000;
  margin: 0.25rem 0 0 0.25rem;
  min-width: 160px;
  padding: 0.5rem 0;
  background: #fff;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}
.action-menu-section {
  padding: 0 0.25rem;
}
.action-menu-section:not(:last-child) {
  margin-bottom: 0.25rem;
  padding-bottom: 0.25rem;
  border-bottom: 1px solid #f1f5f9;
}
.action-menu-section-title {
  padding: 0.25rem 0.5rem;
  font-size: 0.6875rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: #94a3b8;
}
.action-menu-item {
  display: block;
  width: 100%;
  padding: 0.4rem 0.75rem;
  border: none;
  background: none;
  font-size: 0.8125rem;
  text-align: left;
  color: #334155;
  cursor: pointer;
  border-radius: 4px;
}
.action-menu-item:hover {
  background: #f1f5f9;
  color: #2563eb;
}
.action-menu-item-danger:hover {
  background: #fef2f2;
  color: #dc2626;
}
.action-menu-loading {
  padding: 0.4rem 0.75rem;
  font-size: 0.8125rem;
  color: #94a3b8;
}
.combobox-wrap {
  position: relative;
}
.combobox-trigger {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.35rem 0.6rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  background: #fff;
  font-size: 0.8125rem;
  color: #475569;
  cursor: pointer;
  min-width: 0;
}
.combobox-trigger:hover {
  background: #f8fafc;
  border-color: #cbd5e1;
}
.combobox-trigger.open {
  border-color: #2563eb;
  background: #f8fafc;
}
.combobox-label {
  color: #94a3b8;
  font-size: 0.75rem;
}
.combobox-value {
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.combobox-arrow {
  font-size: 0.6rem;
  color: #94a3b8;
}
.combobox-menu {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  min-width: 180px;
  max-height: 280px;
  overflow: auto;
  background: #fff;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
  padding: 0.25rem 0;
  z-index: 100;
  display: flex;
  flex-direction: column;
}
.combobox-input {
  margin: 0.25rem 0.5rem 0.35rem;
  padding: 0.35rem 0.5rem;
  border: 1px solid #e2e8f0;
  border-radius: 4px;
  font-size: 0.8125rem;
}
.combobox-input:focus {
  outline: none;
  border-color: #2563eb;
}
.filter-input {
  padding: 0.35rem 0.6rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  font-size: 0.8125rem;
  min-width: 120px;
  max-width: 160px;
}
.filter-input:focus {
  outline: none;
  border-color: #2563eb;
}
.filter-input-label {
  min-width: 160px;
  max-width: 220px;
}
.combobox-item {
  display: block;
  width: 100%;
  padding: 0.4rem 0.75rem;
  border: none;
  background: none;
  font-size: 0.8125rem;
  text-align: left;
  cursor: pointer;
  color: #334155;
}
.combobox-item:hover {
  background: #f1f5f9;
}
.combobox-item.active {
  background: rgba(37, 99, 235, 0.1);
  color: #2563eb;
  font-weight: 500;
}
.combobox-group-label {
  padding: 0.35rem 0.75rem 0.2rem;
  font-size: 0.6875rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: #94a3b8;
  border-top: 1px solid #f1f5f9;
}
.combobox-group-label:first-of-type {
  border-top: none;
  padding-top: 0.25rem;
}
.resource-table tbody tr.row-clickable {
  cursor: pointer;
}
.resource-table tbody tr.row-clickable:hover {
  background: #f8fafc;
}
.btn-refresh {
  margin-left: auto;
  padding: 0.35rem 0.75rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  background: #fff;
  font-size: 0.8125rem;
  cursor: pointer;
}
.btn-refresh:hover:not(:disabled) {
  background: #f8fafc;
}
.btn-refresh:disabled {
  opacity: 0.7;
  cursor: not-allowed;
}
.btn-secondary-outline {
  padding: 0.35rem 0.75rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  background: #fff;
  color: #475569;
  font-size: 0.8125rem;
  cursor: pointer;
}
.btn-secondary-outline:hover {
  background: #f8fafc;
}
.btn-danger-outline {
  padding: 0.35rem 0.75rem;
  border: 1px solid #dc2626;
  border-radius: 6px;
  background: #fff;
  color: #dc2626;
  font-size: 0.8125rem;
  cursor: pointer;
}
.btn-danger-outline:hover {
  background: #fef2f2;
}
.btn-danger-outline:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.btn-watch {
  padding: 0.35rem 0.75rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  background: #fff;
  font-size: 0.8125rem;
  cursor: pointer;
  color: #64748b;
}
.btn-watch:hover {
  background: #f8fafc;
}
.btn-watch.active {
  border-color: #22c55e;
  background: rgba(34, 197, 94, 0.08);
  color: #16a34a;
}
.disconnect-banner {
  padding: 0.75rem 1rem;
  background: #fef3c7;
  color: #b45309;
  border-bottom: 1px solid #fde68a;
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex-wrap: wrap;
}
.disconnect-text {
  font-weight: 500;
}
.disconnect-detail {
  flex: 1;
  min-width: 0;
  font-size: 0.875rem;
  opacity: 0.9;
  overflow: hidden;
  text-overflow: ellipsis;
}
.btn-reconnect {
  flex-shrink: 0;
  padding: 0.25rem 0.75rem;
  background: #b45309;
  color: #fff;
  border: none;
  border-radius: 4px;
  font-size: 0.875rem;
  cursor: pointer;
}
.btn-reconnect:hover {
  background: #92400e;
}
.connection-stepper {
  padding: 0.75rem 1rem;
  background: #f0f9ff;
  color: #0369a1;
  border-bottom: 1px solid #bae6fd;
  font-size: 0.875rem;
}
.stepper-title {
  font-weight: 500;
}
.stepper-detail {
  margin-top: 0.25rem;
  font-size: 0.8125rem;
  opacity: 0.9;
}
.error-banner {
  padding: 0.75rem 1rem;
  background: #fef2f2;
  color: #dc2626;
  font-size: 0.875rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}
.error-dismiss {
  flex-shrink: 0;
  padding: 0.25rem 0.5rem;
  border: 1px solid #fca5a5;
  border-radius: 4px;
  background: #fff;
  color: #dc2626;
  font-size: 0.8125rem;
  cursor: pointer;
}
.error-dismiss:hover {
  background: #fef2f2;
}
.error-modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}
.error-modal {
  background: #fff;
  border-radius: 12px;
  padding: 1.5rem;
  max-width: 420px;
  width: 90%;
  box-shadow: 0 20px 40px rgba(0, 0, 0, 0.15);
}
.error-modal-title {
  margin: 0 0 1rem 0;
  font-size: 1.125rem;
  font-weight: 600;
  color: #1e293b;
}
.error-modal-message {
  margin: 0 0 1.25rem 0;
  font-size: 0.875rem;
  color: #475569;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}
.error-modal-actions {
  display: flex;
  justify-content: flex-end;
}
.loading-state {
  padding: 1.5rem;
  text-align: center;
  color: #64748b;
  font-size: 0.875rem;
}
.table-wrap {
  flex: 1;
  overflow: auto;
  padding: 1rem;
}
.resource-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.8125rem;
}
.resource-table th.col-checkbox,
.resource-table td.col-checkbox {
  width: 2.5rem;
  padding: 0.5rem 0.5rem;
  vertical-align: middle;
}
.col-checkbox input {
  cursor: pointer;
}
.resource-table th,
.resource-table td {
  padding: 0.5rem 0.75rem;
  text-align: left;
}
.resource-table td {
  border-bottom: 1px solid #e2e8f0;
}
.resource-table th {
  font-weight: 600;
  color: #475569;
  background: #f8fafc;
}
.resource-table th.sortable {
  cursor: pointer;
  user-select: none;
}
.resource-table th.sortable:hover {
  color: #2563eb;
}
.sort-indicator {
  margin-left: 0.25rem;
  font-size: 0.75rem;
  color: #64748b;
}
.resource-table tbody tr:hover {
  background: #f8fafc;
}
.resource-table td.cell-drillable {
  cursor: pointer;
  color: #2563eb;
}
.resource-table td.cell-drillable:hover {
  text-decoration: underline;
}
.resource-table .cell-link {
  cursor: pointer;
  color: #2563eb;
}
.resource-table .cell-link:hover {
  text-decoration: underline;
}
.empty-table {
  margin: 1rem 0 0 0;
  color: #94a3b8;
  font-size: 0.875rem;
}
.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  color: #64748b;
  font-size: 0.875rem;
}
.empty-state p {
  margin: 0;
}
.empty-desc {
  font-size: 0.8125rem;
  color: #94a3b8;
}
</style>
