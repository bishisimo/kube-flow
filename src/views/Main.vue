<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from "vue";
import * as jsYaml from "js-yaml";
import { useEnvStore } from "../stores/env";
import EnvBar from "../components/EnvBar.vue";
import { RESOURCE_GROUPS, RESOURCE_KINDS_FLAT, type ResourceKind } from "../constants/resourceKinds";
import {
  fetchResourceList,
  resourceIsClusterScoped,
  resourceSupportsWatch,
} from "../resources/resourceRegistry";

const RESOURCE_KINDS = RESOURCE_KINDS_FLAT;
import ResourceDetailDrawer from "../components/ResourceDetailDrawer.vue";
import ChangeImageModal from "../components/ChangeImageModal.vue";
import DeleteConfirmModal from "../components/DeleteConfirmModal.vue";
import { listen } from "@tauri-apps/api/event";
import { kubeDeleteResource, kubeRemoveClient } from "../api/kube";
import { useConnectionStore, isConnectionError } from "../stores/connection";
import { useSshAuthStore } from "../stores/sshAuth";
import { useStrongholdAuthStore } from "../stores/strongholdAuth";
import { useShellStore } from "../stores/shell";
import { useLogCenterStore } from "../stores/logCenter";
import { useOrchestratorStore } from "../stores/orchestrator";
import { useAppSettingsStore } from "../stores/appSettings";
import {
  buildNodeTerminalLaunch,
  getNodeTerminalStrategy,
} from "../stores/nodeTerminalStrategy";
import { effectiveContext } from "../api/env";
import {
  kubeGetResource,
  kubeGetPodContainers,
  kubeListNodes,
  kubeListNamespaces,
  kubeListServices,
  kubeStartWatch,
  kubeStopWatch,
  type NamespaceItem,
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
  type WorkloadPodRollup,
} from "../api/kube";
import type { PodDebugNamespace } from "../api/terminal";
import { defaultNamespace } from "../api/env";

const { openedEnvs, currentEnv, currentId, touchEnv, loadEnvironments, getEnvViewState, setEnvViewState } = useEnvStore();
const { pendingOpen, requestSwitchToShell } = useShellStore();
const { pendingLogOpen, requestSwitchToLogCenter } = useLogCenterStore();
const { manifests, upsertFromWorkbenchSync, requestSwitchToOrchestrator } = useOrchestratorStore();
const { nodeResourceUsageEnabled, ensureAppSettingsLoaded } = useAppSettingsStore();
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
const strongholdAuth = useStrongholdAuthStore();

const ENV_BAR_COLLAPSED_KEY = "kube-flow:env-bar-collapsed";
const NS_FAVORITES_KEY = "kube-flow:ns-favorites";
const NS_RECENT_KEY_PREFIX = "kube-flow:ns-recent:";
const RECENT_KINDS_KEY = "kube-flow:recent-kinds";
const MAX_RECENT_KINDS = 6;
const ALL_NAMESPACES_SENTINEL = "__all__";
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
  touchRecentKind(kind);
  kindDropdownOpen.value = false;
  drillFrom.value = null;
  labelSelector.value = "";
  nameFilter.value = "";
  nodeFilter.value = "all";
  podIpFilter.value = "";
  selectedRowKeys.value = new Set();
  batchDeleteMode.value = false;
}

/** 仅清除钻取上下文并刷新（面包屑点击 namespace 时） */
function clearDrillAndReload() {
  drillFrom.value = null;
  labelSelector.value = "";
  nameFilter.value = "";
  nodeFilter.value = "all";
  podIpFilter.value = "";
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
  podIpFilter.value = "";
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
  podIpFilter.value = "";
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
const envSwitching = ref(false);
const envSwitchingName = ref("");
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
const nsMenuRef = ref<HTMLElement | null>(null);
const kindMenuRef = ref<HTMLElement | null>(null);
const nsFilter = ref("");
const kindFilter = ref("");
/** 按名称筛选：前端过滤，支持模糊匹配（包含） */
const nameFilter = ref("");
/** 按 Node 筛选：仅在 Pods 视图生效，默认 all（不过滤） */
const nodeFilter = ref("all");
/** 按 Pod IP 筛选：仅在 Pods 视图生效，支持包含匹配 */
const podIpFilter = ref("");
/** 按 label 筛选：传给 K8s API，格式如 app=nginx 或 env in (prod,staging) */
const labelSelector = ref("");
/** Watch 实时更新：开启后通过 Tauri 事件接收增量，仅部分 kind 支持 */
const watchEnabled = ref(true);
const NODE_ALLOC_REFRESH_MS = 30_000;
const nodeAllocations = ref<Record<string, { cpuRequests: string; memoryRequests: string; gpuRequests: string }>>({});
let nodeAllocRefreshTimer: number | null = null;
/** 排序：默认按创建时间倒序 */

function currentEnvSourceLabel(): string {
  return currentEnv.value?.source === "ssh_tunnel" ? "SSH 环境" : "本地 kubeconfig";
}

function currentEnvContextLabel(): string {
  if (!currentEnv.value) return "未选择 Context";
  return effectiveContext(currentEnv.value) ?? "未选择 Context";
}

function shouldShowCurrentEnvContext(): boolean {
  return currentEnv.value?.source !== "ssh_tunnel";
}

function currentEnvStateLabel(): string {
  if (!currentId.value) return "未连接";
  const state = getState(currentId.value);
  if (state === "disconnected") return "连接断开";
  if (state === "connecting") return "连接中";
  return "已就绪";
}

function supportsIpFilter(): boolean {
  return selectedKind.value === "pods" || selectedKind.value === "services";
}

function ipFilterPlaceholder(): string {
  return selectedKind.value === "services" ? "按 Service IP 筛选…" : "按 Pod IP 筛选…";
}

function ipFilterTitle(): string {
  return selectedKind.value === "services"
    ? "按 Service ClusterIP 包含匹配（前端过滤）"
    : "按 Pod IP 包含匹配（前端过滤）";
}

function ipFilterChipLabel(): string {
  return selectedKind.value === "services" ? "Service IP" : "Pod IP";
}
const sortBy = ref<string>("creationTime");
const sortOrder = ref<"asc" | "desc">("desc");
const viewSessionId = ref(0);
const latestListRequestId = ref(0);
const activeWatchTokens = ref<Record<string, string>>({});
const activeWatchViews = ref<Record<string, { kind: ResourceKind; namespace: string | null; labelSelector: string | null }>>({});

type ResourceCacheEntry = {
  envId: string;
  kind: ResourceKind;
  namespace: string | null;
  labelSelector: string | null;
  items: unknown[];
  updatedAt: number;
};

const namespaceCache = new Map<string, NamespaceItem[]>();
const resourceCache = new Map<string, ResourceCacheEntry>();

function nextToken(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function buildResourceCacheKey(
  envId: string,
  kind: ResourceKind,
  namespace: string | null,
  labelSelector: string | null
): string {
  return `${envId}::${kind}::${namespace ?? ALL_NAMESPACES_SENTINEL}::${labelSelector ?? ""}`;
}

function setWatchToken(envId: string, token: string) {
  activeWatchTokens.value = { ...activeWatchTokens.value, [envId]: token };
}

function setWatchView(envId: string, kind: ResourceKind, namespace: string | null, labelSelector: string | null) {
  activeWatchViews.value = {
    ...activeWatchViews.value,
    [envId]: { kind, namespace, labelSelector },
  };
}

function clearWatchToken(envId: string) {
  const next = { ...activeWatchTokens.value };
  delete next[envId];
  activeWatchTokens.value = next;
  const nextViews = { ...activeWatchViews.value };
  delete nextViews[envId];
  activeWatchViews.value = nextViews;
}

function clearResourceCollections() {
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

function clearNodeAllocations() {
  nodeAllocations.value = {};
}

function applyNodeAllocationSnapshot(items: NodeItem[]) {
  const next: Record<string, { cpuRequests: string; memoryRequests: string; gpuRequests: string }> = {};
  for (const item of items) {
    next[item.name] = {
      cpuRequests: item.cpu_requests ?? "-",
      memoryRequests: item.memory_requests ?? "-",
      gpuRequests: item.gpu_requests ?? "-",
    };
  }
  nodeAllocations.value = next;
}

async function refreshNodeAllocations() {
  const envId = currentId.value;
  if (!envId || selectedKind.value !== "nodes") return;
  try {
    const items = await kubeListNodes(envId, labelSelector.value.trim() || null);
    if (envId !== currentId.value || selectedKind.value !== "nodes") return;
    applyNodeAllocationSnapshot(items);
  } catch {
    // 快照刷新失败时保留上一份结果，避免节点分配列闪空。
  }
}

function stopNodeAllocationPolling() {
  if (nodeAllocRefreshTimer !== null) {
    window.clearInterval(nodeAllocRefreshTimer);
    nodeAllocRefreshTimer = null;
  }
}

function syncNodeAllocationPolling() {
  stopNodeAllocationPolling();
  if (!nodeResourceUsageEnabled.value || !currentId.value || selectedKind.value !== "nodes") {
    clearNodeAllocations();
    return;
  }
  void refreshNodeAllocations();
  nodeAllocRefreshTimer = window.setInterval(() => {
    void refreshNodeAllocations();
  }, NODE_ALLOC_REFRESH_MS);
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

function cacheCurrentView(envId: string, kind: ResourceKind, namespace: string | null, labelSelector: string | null, items: unknown[]) {
  resourceCache.set(buildResourceCacheKey(envId, kind, namespace, labelSelector), {
    envId,
    kind,
    namespace,
    labelSelector,
    items: [...items],
    updatedAt: Date.now(),
  });
}

function applyCachedView(envId: string, kind: ResourceKind, namespace: string | null, labelSelector: string | null): boolean {
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
  envSwitching.value = false;
  listLoading.value = false;
  listError.value = null;
  return true;
}

function resetTransientWorkbenchState() {
  selectedResource.value = null;
  detailDrawerVisible.value = false;
  detailDrawerInitialTab.value = null;
  changeImageModalVisible.value = false;
  closeActionMenu();
  deleteConfirmVisible.value = false;
  deleteConfirmResources.value = [];
  deleteConfirmError.value = null;
  syncOrchestratorDialogVisible.value = false;
  syncOrchestratorRelatedRefs.value = [];
  syncOrchestratorSelectedRefKeys.value = [];
  syncOrchestratorRelatedError.value = null;
  selectedRowKeys.value = new Set();
  batchDeleteMode.value = false;
}

function beginEnvSwitch(nextEnvId: string | null) {
  viewSessionId.value += 1;
  resetTransientWorkbenchState();
  listError.value = null;
  listLoading.value = !!nextEnvId;
  envSwitching.value = !!nextEnvId;
  envSwitchingName.value = nextEnvId
    ? (openedEnvs.value.find((env) => env.id === nextEnvId)?.display_name ?? "新环境")
    : "";
}

function isStaleView(envId: string, sessionId: number, requestId?: number): boolean {
  if (currentId.value !== envId) return true;
  if (viewSessionId.value !== sessionId) return true;
  if (typeof requestId === "number" && latestListRequestId.value !== requestId) return true;
  return false;
}

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

const namespaceFavorites = computed(() =>
  filteredNamespaceOptions.value.filter((n) => favoriteNamespaces.value.has(n.name))
);
const namespaceRecent = computed(() =>
  recentNamespaces.value
    .filter((n) => !favoriteNamespaces.value.has(n))
    .map((name) => filteredNamespaceOptions.value.find((n) => n.name === name))
    .filter((n): n is NamespaceItem => Boolean(n))
);
const namespaceOthers = computed(() => {
  const recent = new Set(namespaceRecent.value.map((n) => n.name));
  return filteredNamespaceOptions.value.filter(
    (n) => !favoriteNamespaces.value.has(n.name) && !recent.has(n.name)
  );
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

const recentKindItems = computed(() =>
  recentKinds.value
    .map((id) => RESOURCE_KINDS.find((k) => k.id === id))
    .filter((k): k is (typeof RESOURCE_KINDS)[number] => Boolean(k))
);

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
const nsSelectionDisabled = computed(() => resourceIsClusterScoped(selectedKind.value));

const favoriteNamespaces = ref<Set<string>>(new Set());
const recentNamespaces = ref<string[]>([]);
const recentKinds = ref<ResourceKind[]>([]);

function loadFavoriteNamespaces(): Set<string> {
  try {
    const s = localStorage.getItem(NS_FAVORITES_KEY);
    if (!s) return new Set();
    const arr = JSON.parse(s) as unknown;
    if (!Array.isArray(arr)) return new Set();
    return new Set(arr.filter((v): v is string => typeof v === "string" && v.trim().length > 0));
  } catch {
    return new Set();
  }
}

function persistFavoriteNamespaces() {
  try {
    localStorage.setItem(NS_FAVORITES_KEY, JSON.stringify(Array.from(favoriteNamespaces.value)));
  } catch {}
}

function loadRecentNamespaces(envId: string | null): string[] {
  if (!envId) return [];
  try {
    const s = localStorage.getItem(`${NS_RECENT_KEY_PREFIX}${envId}`);
    if (!s) return [];
    const arr = JSON.parse(s) as unknown;
    if (!Array.isArray(arr)) return [];
    return arr.filter((v): v is string => typeof v === "string" && v.trim().length > 0);
  } catch {
    return [];
  }
}

function persistRecentNamespaces(envId: string | null) {
  if (!envId) return;
  try {
    localStorage.setItem(`${NS_RECENT_KEY_PREFIX}${envId}`, JSON.stringify(recentNamespaces.value.slice(0, 8)));
  } catch {}
}

function loadRecentKinds(): ResourceKind[] {
  try {
    const s = localStorage.getItem(RECENT_KINDS_KEY);
    if (!s) return [];
    const arr = JSON.parse(s) as unknown;
    if (!Array.isArray(arr)) return [];
    return arr
      .filter((v): v is ResourceKind => typeof v === "string" && VALID_KINDS.has(v))
      .slice(0, MAX_RECENT_KINDS);
  } catch {
    return [];
  }
}

function persistRecentKinds() {
  try {
    localStorage.setItem(RECENT_KINDS_KEY, JSON.stringify(recentKinds.value.slice(0, MAX_RECENT_KINDS)));
  } catch {}
}

function touchRecentNamespace(ns: string | null) {
  if (!ns || !ns.trim()) return;
  recentNamespaces.value = [ns, ...recentNamespaces.value.filter((v) => v !== ns)].slice(0, 8);
  persistRecentNamespaces(currentId.value);
}

function toggleFavoriteNamespace(ns: string) {
  const next = new Set(favoriteNamespaces.value);
  if (next.has(ns)) next.delete(ns);
  else next.add(ns);
  favoriteNamespaces.value = next;
  persistFavoriteNamespaces();
}

function touchRecentKind(kind: ResourceKind) {
  recentKinds.value = [kind, ...recentKinds.value.filter((k) => k !== kind)].slice(0, MAX_RECENT_KINDS);
  persistRecentKinds();
}

function selectNamespace(ns: string | null) {
  selectedNamespace.value = ns;
  nsDropdownOpen.value = false;
  if (ns) touchRecentNamespace(ns);
}

async function refreshNamespaceOptions() {
  const id = currentId.value;
  if (!id) return;
  try {
    namespaceOptions.value = await kubeListNamespaces(id, null);
  } catch {
    // 保持现有列表，避免因瞬时错误清空下拉选项。
  }
}

function toggleNamespaceDropdown() {
  nsDropdownOpen.value = !nsDropdownOpen.value;
  if (nsDropdownOpen.value) {
    nsFilter.value = "";
    kindDropdownOpen.value = false;
    if (!namespaceOptions.value.length) void refreshNamespaceOptions();
  }
}

function openKindSelector() {
  kindDropdownOpen.value = true;
  kindFilter.value = "";
  nsDropdownOpen.value = false;
  nextTick(() => {
    const active = kindMenuRef.value?.querySelector(".combobox-item.active");
    if (active && "scrollIntoView" in active) {
      (active as HTMLElement).scrollIntoView({ block: "nearest" });
    }
  });
}

/** API Kind（如 Service）到 ResourceKind id（如 services）的映射 */
const API_KIND_TO_ID: Record<string, ResourceKind> = Object.fromEntries(
  RESOURCE_KINDS.map((k) => [k.label, k.id])
) as Record<string, ResourceKind>;

/** 集群级资源，无需 namespace */
type SelectedResourceRef = {
  kind: string;
  name: string;
  namespace: string | null;
  nodeName: string | null;
};

const selectedResource = ref<SelectedResourceRef | null>(null);
const detailDrawerVisible = ref(false);
const detailDrawerInitialTab = ref<string | null>(null);
const changeImageModalVisible = ref(false);
const deleteConfirmVisible = ref(false);
const podDebugModalVisible = ref(false);
const podDebugLoading = ref(false);
const podDebugError = ref("");
const podDebugContainerOptions = ref<string[]>([]);
const podDebugSelectedContainer = ref("");
const podDebugNamespaces = ref<PodDebugNamespace[]>(["net"]);
const podDebugProcessMode = ref<"main" | "pid">("main");
const podDebugPidInput = ref("");
const syncOrchestratorDialogVisible = ref(false);
const syncOrchestratorMode = ref<"existing" | "new">("existing");
const syncOrchestratorExistingComponent = ref("");
const syncOrchestratorNewComponent = ref("");
type SyncRelatedKind = "ConfigMap" | "Secret" | "Service";
type SyncRelatedRef = { kind: SyncRelatedKind; name: string; namespace: string | null };
const syncOrchestratorRelatedRefs = ref<SyncRelatedRef[]>([]);
const syncOrchestratorSelectedRefKeys = ref<string[]>([]);
const syncOrchestratorLoadingRelated = ref(false);
const syncOrchestratorRelatedError = ref<string | null>(null);

const POD_DEBUG_NAMESPACE_OPTIONS: Array<{
  value: PodDebugNamespace;
  label: string;
  description: string;
  recommended?: boolean;
}> = [
  { value: "net", label: "网络", description: "保留主机工具，排查连接、路由、DNS、端口与抓包。", recommended: true },
  { value: "pid", label: "进程", description: "观察容器进程视图，配合 ps、lsof、ss 做进程级排障。" },
  { value: "mnt", label: "挂载", description: "查看容器文件系统与挂载点，适合卷与配置文件排查。" },
  { value: "uts", label: "主机名", description: "进入容器 UTS 环境，确认 hostname 与域名行为。" },
  { value: "ipc", label: "IPC", description: "排查共享内存、信号量等 IPC 相关问题。" },
];

const orchestratorComponentsForCurrentEnv = computed(() => {
  const envId = currentId.value;
  if (!envId) return [];
  const names = new Set<string>();
  for (const m of manifests.value) {
    if (m.env_id === envId) names.add(m.component);
  }
  return Array.from(names).sort((a, b) => a.localeCompare(b));
});
const syncSelectedRelatedCount = computed(() => syncOrchestratorSelectedRefKeys.value.length);
const syncRelatedTotalCount = computed(() => syncOrchestratorRelatedRefs.value.length);
const syncAllRelatedSelected = computed(
  () => syncOrchestratorRelatedRefs.value.length > 0 && syncOrchestratorSelectedRefKeys.value.length === syncOrchestratorRelatedRefs.value.length
);
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
const deleteActionArmed = ref(false);
const ACTION_MENU_OFFSET = 6;
const ACTION_MENU_VIEWPORT_GAP = 10;

function selectResourceFromRow(row: Record<string, unknown>) {
  const name = row.name as string | undefined;
  if (!name) return null;
  const kindLabel = RESOURCE_KINDS.find((k) => k.id === selectedKind.value)?.label ?? "";
  if (!kindLabel) return null;
  const isCluster = resourceIsClusterScoped(selectedKind.value);
  const envDefaultNs = currentEnv.value ? defaultNamespace(currentEnv.value) : null;
  const nodeName =
    kindLabel === "Pod"
      ? typeof row.node === "string" && row.node !== "-" ? row.node : null
      : null;
  return {
    kind: kindLabel,
    name,
    namespace: isCluster ? null : ((row.ns as string) ?? envDefaultNs ?? "default"),
    nodeName,
  };
}

/** 行唯一标识：集群级用 name，命名空间级用 ns/name */
function getRowKey(row: Record<string, unknown>): string {
  const name = row.name as string | undefined;
  if (!name) return "";
  const isCluster = resourceIsClusterScoped(selectedKind.value);
  const envDefaultNs = currentEnv.value ? defaultNamespace(currentEnv.value) : null;
  return isCluster ? name : `${(row.ns as string) ?? envDefaultNs ?? "default"}/${name}`;
}

function onRowContextMenu(row: Record<string, unknown>, e: MouseEvent) {
  const resource = selectResourceFromRow(row);
  if (!resource) return;
  selectedResource.value = resource;
  deleteActionArmed.value = false;
  actionMenuVisible.value = true;
  actionMenuPosition.value = {
    x: e.clientX + ACTION_MENU_OFFSET,
    y: e.clientY + ACTION_MENU_OFFSET,
  };
  nextTick(() => {
    adjustActionMenuPosition();
  });
}

function onRowClick(row: Record<string, unknown>) {
  const resource = selectResourceFromRow(row);
  if (resource) openDetailDrawerForResource(resource);
}

function openDetailDrawerForResource(resource: SelectedResourceRef, initialTab: string | null = null) {
  selectedResource.value = resource;
  detailDrawerInitialTab.value = initialTab;
  detailDrawerVisible.value = true;
}

function openNodeTaintsFromRow(row: Record<string, unknown>) {
  const resource = selectResourceFromRow(row);
  if (!resource || resource.kind !== "Node") return;
  openDetailDrawerForResource(resource, "taints");
}

function closeActionMenu() {
  actionMenuVisible.value = false;
  deleteActionArmed.value = false;
}

function adjustActionMenuPosition() {
  if (!actionMenuVisible.value || !actionMenuRef.value) return;
  const rect = actionMenuRef.value.getBoundingClientRect();
  const maxX = Math.max(
    ACTION_MENU_VIEWPORT_GAP,
    window.innerWidth - rect.width - ACTION_MENU_VIEWPORT_GAP
  );
  const maxY = Math.max(
    ACTION_MENU_VIEWPORT_GAP,
    window.innerHeight - rect.height - ACTION_MENU_VIEWPORT_GAP
  );
  actionMenuPosition.value = {
    x: Math.min(Math.max(ACTION_MENU_VIEWPORT_GAP, actionMenuPosition.value.x), maxX),
    y: Math.min(Math.max(ACTION_MENU_VIEWPORT_GAP, actionMenuPosition.value.y), maxY),
  };
}

function openResourceDetail() {
  detailDrawerInitialTab.value = null;
  detailDrawerVisible.value = true;
  closeActionMenu();
}

function openPodLogs() {
  const r = selectedResource.value;
  if (!r || !currentId.value || !currentEnv.value) return;
  const ns = r.namespace ?? "default";
  if (r.kind === "Pod") {
    pendingLogOpen.value = {
      kind: "pod",
      envId: currentId.value,
      envName: currentEnv.value.display_name,
      namespace: ns,
      podName: r.name,
    };
  } else {
    pendingLogOpen.value = {
      kind: "workload",
      envId: currentId.value,
      envName: currentEnv.value.display_name,
      namespace: ns,
      workloadKind: r.kind,
      workloadName: r.name,
    };
  }
  requestSwitchToLogCenter();
  closeActionMenu();
}

const SHELL_WORKLOAD_KINDS = new Set(["Pod", "Deployment", "StatefulSet", "DaemonSet"]);
const NODE_TERMINAL_RESOURCE_KINDS = new Set(["Node", "Pod"]);

const nodeTerminalTargetName = computed(() => {
  const resource = selectedResource.value;
  if (!resource) return "";
  if (resource.kind === "Node") return resource.name;
  if (resource.kind === "Pod") return resource.nodeName?.trim() ?? "";
  return "";
});

const nodeTerminalMenuLabel = computed(() => {
  if (selectedResource.value?.kind === "Pod") return "打开所在节点终端";
  return "打开节点终端";
});

const nodeTerminalDisabledReason = computed(() => {
  const resource = selectedResource.value;
  if (!resource || !NODE_TERMINAL_RESOURCE_KINDS.has(resource.kind)) return "";
  if (!currentId.value || !currentEnv.value) return "当前未定位到环境，无法打开节点终端。";
  const strategy = getNodeTerminalStrategy(currentId.value);
  if (!strategy?.enabled) {
    return "当前环境还没有启用节点终端切换策略，请先到环境管理中的终端策略里配置。";
  }
  if (!nodeTerminalTargetName.value) {
    return resource.kind === "Pod"
      ? "当前 Pod 尚未调度到节点，暂时无法打开所在节点终端。"
      : "未找到节点信息，无法打开节点终端。";
  }
  return "";
});

const podDebugDisabledReason = computed(() => {
  const resource = selectedResource.value;
  if (!resource || resource.kind !== "Pod") return "";
  return nodeTerminalDisabledReason.value;
});

const canOpenPodDebug = computed(
  () => Boolean(selectedResource.value?.kind === "Pod" && !podDebugDisabledReason.value)
);

const canOpenNodeTerminal = computed(
  () =>
    Boolean(
      selectedResource.value &&
        NODE_TERMINAL_RESOURCE_KINDS.has(selectedResource.value.kind) &&
        !nodeTerminalDisabledReason.value
    )
);

function openPodShell() {
  const r = selectedResource.value;
  if (!r || !SHELL_WORKLOAD_KINDS.has(r.kind) || !currentId.value || !currentEnv.value) return;
  const ns = r.namespace ?? "default";
  if (r.kind === "Pod") {
    pendingOpen.value = {
      kind: "pod",
      envId: currentId.value,
      envName: currentEnv.value.display_name,
      namespace: ns,
      podName: r.name,
    };
  } else {
    pendingOpen.value = {
      kind: "pod",
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

async function openNodeTerminalFromMenu() {
  const resource = selectedResource.value;
  const env = currentEnv.value;
  const envId = currentId.value;
  const nodeName = nodeTerminalTargetName.value;
  if (!resource || !env || !envId || !nodeName) return;
  const strategy = getNodeTerminalStrategy(envId);
  const target = buildNodeTerminalLaunch(strategy, nodeName);
  if (!target) return;
  pendingOpen.value = {
    kind: "host",
    envId,
    envName: env.display_name,
    hostLabel: `${env.display_name} / ${nodeName}`,
    nodeTerminalLaunch: target,
  };
  requestSwitchToShell();
  closeActionMenu();
}

async function openPodDebugModal() {
  const resource = selectedResource.value;
  const envId = currentId.value;
  if (!resource || resource.kind !== "Pod" || !envId || podDebugDisabledReason.value) return;
  podDebugLoading.value = true;
  podDebugError.value = "";
  podDebugContainerOptions.value = [];
  podDebugSelectedContainer.value = "";
  podDebugNamespaces.value = ["net"];
  podDebugProcessMode.value = "main";
  podDebugPidInput.value = "";
  try {
    const containers = await kubeGetPodContainers(envId, resource.namespace ?? "default", resource.name);
    podDebugContainerOptions.value = containers;
    podDebugSelectedContainer.value = containers[0] ?? "";
    if (!containers.length) {
      podDebugError.value = "当前 Pod 没有可用容器。";
    }
    podDebugModalVisible.value = true;
    closeActionMenu();
  } catch (e) {
    podDebugError.value = e instanceof Error ? e.message : String(e);
    podDebugModalVisible.value = true;
    closeActionMenu();
  } finally {
    podDebugLoading.value = false;
  }
}

function closePodDebugModal() {
  podDebugModalVisible.value = false;
  podDebugLoading.value = false;
  podDebugError.value = "";
  podDebugContainerOptions.value = [];
  podDebugSelectedContainer.value = "";
  podDebugNamespaces.value = ["net"];
  podDebugProcessMode.value = "main";
  podDebugPidInput.value = "";
}

function togglePodDebugNamespace(value: PodDebugNamespace) {
  const next = new Set(podDebugNamespaces.value);
  if (next.has(value)) {
    if (next.size === 1) return;
    next.delete(value);
  } else {
    next.add(value);
  }
  podDebugNamespaces.value = POD_DEBUG_NAMESPACE_OPTIONS
    .map((item) => item.value)
    .filter((item) => next.has(item));
}

function confirmOpenPodDebug() {
  const resource = selectedResource.value;
  const env = currentEnv.value;
  const envId = currentId.value;
  if (
    !resource ||
    resource.kind !== "Pod" ||
    !env ||
    !envId ||
    !resource.nodeName?.trim() ||
    !podDebugSelectedContainer.value
  ) {
    return;
  }
  const pid =
    podDebugProcessMode.value === "pid"
      ? Number.parseInt(podDebugPidInput.value.trim(), 10)
      : null;
  if (podDebugProcessMode.value === "pid" && (!Number.isInteger(pid) || (pid ?? 0) <= 0)) {
    podDebugError.value = "请输入有效的 PID。";
    return;
  }
  const strategy = getNodeTerminalStrategy(envId);
  const target = buildNodeTerminalLaunch(strategy, resource.nodeName.trim(), {
    namespace: resource.namespace ?? "default",
    podName: resource.name,
    container: podDebugSelectedContainer.value,
    namespaces: podDebugNamespaces.value,
    pid,
  });
  if (!target) return;
  pendingOpen.value = {
    kind: "host",
    envId,
    envName: env.display_name,
    hostLabel: `${env.display_name} / ${resource.name} / ${podDebugSelectedContainer.value}`,
    nodeTerminalLaunch: target,
  };
  requestSwitchToShell();
  closePodDebugModal();
}

function openEnvironmentTerminal(envId?: string | null) {
  const targetEnvId = envId ?? currentId.value;
  if (!targetEnvId) return;
  const env = openedEnvs.value.find((item) => item.id === targetEnvId);
  if (!env) return;
  pendingOpen.value = {
    kind: "host",
    envId: env.id,
    envName: env.display_name,
    hostLabel: `${env.display_name} 主机`,
  };
  requestSwitchToShell();
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

function handleDeleteAction() {
  if (!deleteActionArmed.value) {
    deleteActionArmed.value = true;
    return;
  }
  openDeleteConfirm();
}

function relatedRefKey(ref: SyncRelatedRef): string {
  return `${ref.kind}|${ref.namespace ?? ""}|${ref.name}`;
}

function toggleAllSyncRelatedRefs(checked: boolean) {
  if (!checked) {
    syncOrchestratorSelectedRefKeys.value = [];
    return;
  }
  syncOrchestratorSelectedRefKeys.value = syncOrchestratorRelatedRefs.value.map((r) => relatedRefKey(r));
}

async function loadSyncRelatedRefs(envId: string, resource: { kind: string; name: string; namespace: string | null }) {
  syncOrchestratorLoadingRelated.value = true;
  syncOrchestratorRelatedError.value = null;
  syncOrchestratorRelatedRefs.value = [];
  syncOrchestratorSelectedRefKeys.value = [];
  try {
    const yaml = await kubeGetResource(envId, resource.kind, resource.name, resource.namespace);
    const refs = await collectAssociatedRefsFromWorkloadYaml(envId, yaml, resource.namespace ?? "default");
    syncOrchestratorRelatedRefs.value = refs;
    syncOrchestratorSelectedRefKeys.value = refs.map((r) => relatedRefKey(r));
  } catch (e) {
    syncOrchestratorRelatedError.value = `关联资源解析失败：${extractErrorMessage(e)}`;
  } finally {
    syncOrchestratorLoadingRelated.value = false;
  }
}

async function openSyncToOrchestratorDialog() {
  const envId = currentId.value;
  const r = selectedResource.value;
  if (!envId || !r) return;
  const components = orchestratorComponentsForCurrentEnv.value;
  syncOrchestratorMode.value = "new";
  syncOrchestratorExistingComponent.value = components[0] ?? "";
  syncOrchestratorNewComponent.value = r.name;
  syncOrchestratorDialogVisible.value = true;
  closeActionMenu();
  await loadSyncRelatedRefs(envId, r);
}

function closeSyncToOrchestratorDialog() {
  syncOrchestratorDialogVisible.value = false;
  syncOrchestratorLoadingRelated.value = false;
  syncOrchestratorRelatedError.value = null;
  syncOrchestratorRelatedRefs.value = [];
  syncOrchestratorSelectedRefKeys.value = [];
}

function resolveSyncTargetComponent(): string {
  if (syncOrchestratorMode.value === "existing") {
    const name = syncOrchestratorExistingComponent.value.trim();
    if (!name) throw new Error("请选择已有应用组件。");
    return name;
  }
  const next = syncOrchestratorNewComponent.value.trim();
  if (!next) throw new Error("请输入新应用组件名称。");
  return next;
}

async function syncToOrchestrator() {
  const envId = currentId.value;
  const envName = currentEnv.value?.display_name;
  const r = selectedResource.value;
  if (!envId || !envName || !r) return;
  try {
    const yaml = await kubeGetResource(envId, r.kind, r.name, r.namespace);
    const componentName = resolveSyncTargetComponent();
    const primaryManifest = upsertFromWorkbenchSync(envId, envName, r, yaml, componentName);
    const selectedKeys = new Set(syncOrchestratorSelectedRefKeys.value);
    const relatedRefs = syncOrchestratorRelatedRefs.value.filter((ref) => selectedKeys.has(relatedRefKey(ref)));
    if (relatedRefs.length > 0) {
      const failed: string[] = [];
      for (const ref of relatedRefs) {
        try {
          const relatedYaml = await kubeGetResource(envId, ref.kind, ref.name, ref.namespace);
          upsertFromWorkbenchSync(
            envId,
            envName,
            { kind: ref.kind, name: ref.name, namespace: ref.namespace },
            relatedYaml,
            componentName
          );
        } catch (e) {
          failed.push(`${ref.kind}/${ref.name}: ${extractErrorMessage(e)}`);
        }
      }
      if (failed.length > 0) {
        listError.value = `主资源已同步，部分关联资源同步失败：${failed.join("；")}`;
      }
    }
    requestSwitchToOrchestrator({
      env_id: envId,
      component: primaryManifest.component,
      manifest_id: primaryManifest.id,
      resource_kind: primaryManifest.resource_kind,
      resource_name: primaryManifest.resource_name,
      resource_namespace: primaryManifest.resource_namespace,
    });
    syncOrchestratorDialogVisible.value = false;
  } catch (e) {
    listError.value = extractErrorMessage(e);
  }
}

async function collectAssociatedRefsFromWorkloadYaml(
  envId: string,
  yaml: string,
  defaultNamespace: string
): Promise<SyncRelatedRef[]> {
  let parsed: unknown;
  try {
    parsed = jsYaml.load(yaml);
  } catch {
    return [];
  }
  if (!parsed || typeof parsed !== "object") return [];
  const obj = parsed as Record<string, unknown>;
  const kind = typeof obj.kind === "string" ? obj.kind : "";
  if (!["Deployment", "StatefulSet", "DaemonSet", "Pod"].includes(kind)) return [];

  const metadata = obj.metadata && typeof obj.metadata === "object" ? (obj.metadata as Record<string, unknown>) : null;
  const spec = obj.spec && typeof obj.spec === "object" ? (obj.spec as Record<string, unknown>) : null;
  const template =
    spec?.template && typeof spec.template === "object" ? (spec.template as Record<string, unknown>) : null;
  const podSpec =
    kind === "Pod"
      ? spec
      : template?.spec && typeof template.spec === "object"
        ? (template.spec as Record<string, unknown>)
        : null;
  if (!podSpec) return [];

  const refs: SyncRelatedRef[] = [];
  const pushRef = (refKind: SyncRelatedKind, refName: string) => {
    const n = refName.trim();
    if (!n) return;
    refs.push({ kind: refKind, name: n, namespace: defaultNamespace || "default" });
  };

  const volumes = Array.isArray(podSpec.volumes) ? (podSpec.volumes as Array<Record<string, unknown>>) : [];
  for (const v of volumes) {
    const cm = v.configMap && typeof v.configMap === "object" ? (v.configMap as Record<string, unknown>) : null;
    if (cm && typeof cm.name === "string") pushRef("ConfigMap", cm.name);
    const sec = v.secret && typeof v.secret === "object" ? (v.secret as Record<string, unknown>) : null;
    if (sec && typeof sec.secretName === "string") pushRef("Secret", sec.secretName);
  }

  const imagePullSecrets = Array.isArray(podSpec.imagePullSecrets)
    ? (podSpec.imagePullSecrets as Array<Record<string, unknown>>)
    : [];
  for (const s of imagePullSecrets) {
    if (typeof s.name === "string") pushRef("Secret", s.name);
  }

  const containers = [
    ...(Array.isArray(podSpec.containers) ? (podSpec.containers as Array<Record<string, unknown>>) : []),
    ...(Array.isArray(podSpec.initContainers) ? (podSpec.initContainers as Array<Record<string, unknown>>) : []),
  ];
  for (const c of containers) {
    const envFrom = Array.isArray(c.envFrom) ? (c.envFrom as Array<Record<string, unknown>>) : [];
    for (const ef of envFrom) {
      const cm =
        ef.configMapRef && typeof ef.configMapRef === "object"
          ? (ef.configMapRef as Record<string, unknown>)
          : null;
      if (cm && typeof cm.name === "string") pushRef("ConfigMap", cm.name);
      const sec =
        ef.secretRef && typeof ef.secretRef === "object" ? (ef.secretRef as Record<string, unknown>) : null;
      if (sec && typeof sec.name === "string") pushRef("Secret", sec.name);
    }

    const envList = Array.isArray(c.env) ? (c.env as Array<Record<string, unknown>>) : [];
    for (const env of envList) {
      const valueFrom =
        env.valueFrom && typeof env.valueFrom === "object"
          ? (env.valueFrom as Record<string, unknown>)
          : null;
      const cmRef =
        valueFrom?.configMapKeyRef && typeof valueFrom.configMapKeyRef === "object"
          ? (valueFrom.configMapKeyRef as Record<string, unknown>)
          : null;
      if (cmRef && typeof cmRef.name === "string") pushRef("ConfigMap", cmRef.name);
      const secRef =
        valueFrom?.secretKeyRef && typeof valueFrom.secretKeyRef === "object"
          ? (valueFrom.secretKeyRef as Record<string, unknown>)
          : null;
      if (secRef && typeof secRef.name === "string") pushRef("Secret", secRef.name);
    }
  }

  const podLabels =
    kind === "Pod"
      ? (metadata?.labels as Record<string, unknown> | undefined)
      : template?.metadata && typeof template.metadata === "object"
        ? ((template.metadata as Record<string, unknown>).labels as Record<string, unknown> | undefined)
        : undefined;
  const podLabelMap: Record<string, string> = {};
  if (podLabels && typeof podLabels === "object") {
    for (const [k, v] of Object.entries(podLabels)) {
      if (typeof v === "string") podLabelMap[k] = v;
    }
  }
  const labelKeys = Object.keys(podLabelMap);
  if (labelKeys.length > 0) {
    try {
      const services = await kubeListServices(envId, defaultNamespace || "default", null);
      for (const svc of services) {
        try {
          const serviceYaml = await kubeGetResource(envId, "Service", svc.name, svc.namespace || defaultNamespace || "default");
          const serviceParsed = jsYaml.load(serviceYaml);
          if (!serviceParsed || typeof serviceParsed !== "object") continue;
          const serviceObj = serviceParsed as Record<string, unknown>;
          const serviceSpec =
            serviceObj.spec && typeof serviceObj.spec === "object"
              ? (serviceObj.spec as Record<string, unknown>)
              : null;
          const selector =
            serviceSpec?.selector && typeof serviceSpec.selector === "object"
              ? (serviceSpec.selector as Record<string, unknown>)
              : null;
          if (!selector) continue;
          const selectorEntries = Object.entries(selector).filter(([, value]) => typeof value === "string");
          if (!selectorEntries.length) continue;
          const matched = selectorEntries.every(([key, value]) => podLabelMap[key] === value);
          if (matched) {
            pushRef("Service", svc.name);
          }
        } catch {
          continue;
        }
      }
    } catch {
      // 忽略 Service 扩展解析失败，不影响主流程。
    }
  }

  const dedup = new Map<string, SyncRelatedRef>();
  for (const ref of refs) {
    dedup.set(`${ref.kind}|${ref.namespace ?? ""}|${ref.name}`, ref);
  }
  return Array.from(dedup.values());
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
    .filter((r): r is SelectedResourceRef => r !== null);
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
    selectedNamespace.value = (row.ns as string) ?? selectedNamespace.value;
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
  selectedNamespace.value = subject.namespace ?? (row.ns as string) ?? selectedNamespace.value;
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
  selectedNamespace.value = (row.ns as string) ?? selectedNamespace.value;
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
  return "全部";
});

const rawTableRows = computed(() => {
  switch (selectedKind.value) {
    case "namespaces":
      return namespaceOptions.value.map((n) => ({ name: n.name, creationTime: n.creation_time ?? "-" }));
    case "nodes":
      return nodes.value.map((n) => ({
        name: n.name,
        status: n.status ?? "-",
        taints: typeof n.taint_count === "number" ? (n.taint_count > 0 ? `${n.taint_count}` : "无") : "-",
        internalIp: n.internal_ip ?? "-",
        cpuTotal: n.cpu_total ?? "-",
        memoryTotal: n.memory_total ?? "-",
        gpuTotal: n.gpu_total ?? "-",
        cpuRequests: nodeResourceUsageEnabled.value
          ? (nodeAllocations.value[n.name]?.cpuRequests ?? n.cpu_requests ?? "-")
          : "-",
        memoryRequests: nodeResourceUsageEnabled.value
          ? (nodeAllocations.value[n.name]?.memoryRequests ?? n.memory_requests ?? "-")
          : "-",
        gpuRequests: nodeResourceUsageEnabled.value
          ? (nodeAllocations.value[n.name]?.gpuRequests ?? n.gpu_requests ?? "-")
          : "-",
        creationTime: n.creation_time ?? "-",
      }));
    case "pods":
      return pods.value.map((p) => ({
        name: p.name,
        ns: p.namespace,
        phase: p.phase ?? "-",
        containerStatus: p.container_status ?? "-",
        podIp: p.pod_ip ?? "-",
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
        podRollup: d.pod_rollup ?? null,
        recentRestart: d.pod_rollup?.last_container_restart ?? "-",
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
        podRollup: st.pod_rollup ?? null,
        recentRestart: st.pod_rollup?.last_container_restart ?? "-",
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
        podRollup: d.pod_rollup ?? null,
        recentRestart: d.pod_rollup?.last_container_restart ?? "-",
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
        allowVolumeExpansion:
          s.allow_volume_expansion == null ? "-" : s.allow_volume_expansion ? "是" : "否",
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

function compareCreationTime(a: unknown, b: unknown, order: "asc" | "desc"): number {
  const da = a === "-" || !a ? "" : String(a);
  const db = b === "-" || !b ? "" : String(b);
  const cmp = da.localeCompare(db);
  return order === "asc" ? cmp : -cmp;
}

function sortRows<T extends Record<string, unknown>>(rows: T[], by: string, order: "asc" | "desc"): T[] {
  if (!by || !rows.length) return rows;
  return [...rows].sort((a, b) => {
    const primary =
      by === "creationTime"
        ? compareCreationTime(a[by], b[by], order)
        : compareForSort(a[by], b[by], order);
    if (primary !== 0) return primary;

    return compareForSort(a.name, b.name, "desc");
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

/** 应用名称/Node/Pod IP 筛选与排序 */
const tableRows = computed(() => {
  let raw = rawTableRows.value as Record<string, unknown>[];
  const q = nameFilter.value.trim().toLowerCase();
  if (q) raw = raw.filter((r) => String(r.name ?? "").toLowerCase().includes(q));
  if (selectedKind.value === "pods" && nodeFilter.value !== "all") {
    raw = raw.filter((r) => String(r.node ?? "") === nodeFilter.value);
  }
  if (supportsIpFilter()) {
    const ip = podIpFilter.value.trim().toLowerCase();
    if (ip) {
      raw = raw.filter((r) => {
        const candidate = selectedKind.value === "services" ? r.clusterIp : r.podIp;
        return String(candidate ?? "").toLowerCase().includes(ip);
      });
    }
  }
  const by = sortBy.value;
  const order = sortOrder.value;
  if (by && tableColumns.value.some((c) => c.key === by)) {
    return sortRows(raw, by, order);
  }
  return sortRows(raw, "creationTime", "desc");
});

type ActiveFilterChip = {
  id: "name" | "node" | "podIp" | "label";
  label: string;
  value: string;
};

const activeFilterChips = computed<ActiveFilterChip[]>(() => {
  const chips: ActiveFilterChip[] = [];
  const name = nameFilter.value.trim();
  const label = labelSelector.value.trim();
  if (name) chips.push({ id: "name", label: "名称", value: name });
  if (selectedKind.value === "pods" && nodeFilter.value !== "all") {
    chips.push({ id: "node", label: "Node", value: nodeFilter.value });
  }
  if (supportsIpFilter()) {
    const podIp = podIpFilter.value.trim();
    if (podIp) chips.push({ id: "podIp", label: ipFilterChipLabel(), value: podIp });
  }
  if (label) chips.push({ id: "label", label: "Label", value: label });
  return chips;
});

function clearFilterChip(id: ActiveFilterChip["id"]) {
  if (id === "name") nameFilter.value = "";
  else if (id === "node") nodeFilter.value = "all";
  else if (id === "podIp") podIpFilter.value = "";
  else if (id === "label") labelSelector.value = "";
  if (watchEnabled.value && resourceSupportsWatch(selectedKind.value)) applyWatch();
  else loadList();
}

function clearAllFilters() {
  nameFilter.value = "";
  nodeFilter.value = "all";
  podIpFilter.value = "";
  labelSelector.value = "";
  if (watchEnabled.value && resourceSupportsWatch(selectedKind.value)) applyWatch();
  else loadList();
}

function normalizeStatus(raw: unknown): string {
  const v = String(raw ?? "-").trim();
  return v || "-";
}

function statusTone(statusValue: unknown): "ok" | "warn" | "error" | "neutral" {
  const v = normalizeStatus(statusValue).toLowerCase();
  if (!v || v === "-") return "neutral";
  if (v.includes("notready") || v.includes("not ready") || v.includes("unready")) return "error";
  if (v.includes("running") || v.includes("ready") || v.includes("bound") || v.includes("active")) return "ok";
  if (
    v.includes("pending") ||
    v.includes("containercreating") ||
    v.includes("terminating") ||
    v.includes("unknown")
  ) {
    return "warn";
  }
  if (
    v.includes("failed") ||
    v.includes("error") ||
    v.includes("crashloopbackoff") ||
    v.includes("imagepullbackoff")
  ) {
    return "error";
  }
  return "neutral";
}

function isStatusColumn(colKey: string): boolean {
  return colKey === "phase" || colKey === "status" || colKey === "containerStatus";
}
type PodRollupBadgeTone = "running" | "pending" | "succeeded" | "failed" | "abnormal";
type PodRollupBadge = { key: string; value: number; tone: PodRollupBadgeTone };

function isPodRollupColumn(colKey: string): boolean {
  return colKey === "podRollup";
}

function buildPodRollupBadges(v: unknown): PodRollupBadge[] {
  const r = (v ?? {}) as WorkloadPodRollup;
  const out: PodRollupBadge[] = [];
  const push = (key: string, tone: PodRollupBadgeTone, n: unknown) => {
    const m = Number(n ?? 0);
    if (Number.isFinite(m) && m > 0) out.push({ key, value: m, tone });
  };
  push("running", "running", r.running_ready);
  push("pending", "pending", r.pending);
  push("succeeded", "succeeded", r.succeeded);
  push("failed", "failed", r.failed);
  push("abnormal", "abnormal", r.abnormal);
  return out;
}

function formatRecentRestart(v: unknown): string {
  const r = (v ?? {}) as WorkloadPodRollup;
  return r.last_container_restart ?? "-";
}

function isRecentRestartHot(v: unknown): boolean {
  const text = formatRecentRestart(v);
  if (text === "-") return false;
  const m = text.match(/\(([^)]+)\)/);
  const age = (m?.[1] ?? text).trim();
  if (age.endsWith("秒前")) return true;
  if (age.endsWith("分钟前")) {
    const n = Number.parseInt(age.replace("分钟前", ""), 10);
    return Number.isFinite(n) && n <= 15;
  }
  return false;
}

function isNodeAllocColumn(key: string): boolean {
  return key === "cpuRequests" || key === "memoryRequests" || key === "gpuRequests";
}

function parseAllocPercent(value: unknown): number | null {
  if (typeof value !== "string") return null;
  const m = value.match(/\((\d+)%\)/);
  if (!m) return null;
  const percent = Number.parseInt(m[1], 10);
  return Number.isFinite(percent) ? percent : null;
}

function nodeAllocTone(value: unknown): "" | "warn" | "danger" {
  const percent = parseAllocPercent(value);
  if (percent == null) return "";
  if (percent >= 90) return "danger";
  if (percent >= 80) return "warn";
  return "";
}


function isSelectedRow(row: Record<string, unknown>): boolean {
  if (!selectedResource.value) return false;
  const resource = selectResourceFromRow(row);
  if (!resource) return false;
  return (
    resource.kind === selectedResource.value.kind &&
    resource.name === selectedResource.value.name &&
    resource.namespace === selectedResource.value.namespace
  );
}

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
        { key: "taints", label: "污点" },
        { key: "internalIp", label: "Internal IP" },
        ...(nodeResourceUsageEnabled.value
          ? [
              { key: "cpuRequests", label: "CPU 分配/总量" },
              { key: "memoryRequests", label: "内存分配/总量" },
              { key: "gpuRequests", label: "GPU 分配/总量" },
            ]
          : [
              { key: "cpuTotal", label: "CPU 总量" },
              { key: "memoryTotal", label: "内存总量" },
              { key: "gpuTotal", label: "GPU 总量" },
            ]),
        { key: "creationTime", label: "创建时间" },
      ];
    case "pods":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "phase", label: "状态" },
        { key: "containerStatus", label: "容器启动" },
        { key: "podIp", label: "Pod IP" },
        { key: "node", label: "Node" },
        { key: "creationTime", label: "创建时间" },
      ];
    case "deployments":
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "replicas", label: "副本" },
        { key: "podRollup", label: "Pod 态势" },
        { key: "recentRestart", label: "最近重启" },
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
        { key: "podRollup", label: "Pod 态势" },
        { key: "recentRestart", label: "最近重启" },
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
        { key: "podRollup", label: "Pod 态势" },
        { key: "recentRestart", label: "最近重启" },
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
        { key: "allowVolumeExpansion", label: "允许扩容" },
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
    clearResourceCollections();
    return;
  }
  const requestId = ++latestListRequestId.value;
  const sessionId = viewSessionId.value;
  const ns = selectedNamespace.value ?? ALL_NAMESPACES_SENTINEL;
  const labelSel = labelSelector.value.trim() || null;
  const namespaceKey = selectedKind.value === "namespaces" || selectedKind.value === "nodes" ? null : ns;
  const hasCache = applyCachedView(id, selectedKind.value, namespaceKey, labelSel);
  listLoading.value = !hasCache;
  listError.value = null;
  if (getState(id) !== "connected") setConnecting(id);
  try {
    await touchEnv(id);
    if (isStaleView(id, sessionId, requestId)) return;
    await loadEnvironments();
    if (isStaleView(id, sessionId, requestId)) return;
    const nextNamespaces = await kubeListNamespaces(id, labelSel);
    if (isStaleView(id, sessionId, requestId)) return;
    namespaceCache.set(id, [...nextNamespaces]);
    const applyResult = () => {
      clearResourceCollections();
      namespaceOptions.value = nextNamespaces;
      envSwitching.value = false;
      listLoading.value = false;
    };
    const targetNamespace =
      selectedKind.value === "namespaces" || resourceIsClusterScoped(selectedKind.value) ? null : ns;
    const items =
      selectedKind.value === "namespaces"
        ? nextNamespaces
        : await fetchResourceList(selectedKind.value, id, targetNamespace, labelSel);
    if (isStaleView(id, sessionId, requestId)) return;
    applyResult();
    setResourceItems(selectedKind.value, items);
    cacheCurrentView(id, selectedKind.value, targetNamespace, labelSel, items);
    if (isStaleView(id, sessionId, requestId)) return;
    setConnected(id);
  } catch (e: unknown) {
    if (isStaleView(id, sessionId, requestId)) return;
    const msg = extractErrorMessage(e);
    const isStrongholdRequired = await strongholdAuth.checkAndHandle(
      msg,
      () => {
        loadList();
      },
      {
        title: "解锁环境凭证",
        description: "当前环境连接需要访问已保存凭证，请先输入 Stronghold 主密码解锁。",
      }
    );
    if (isStrongholdRequired) {
      setConnecting(id);
      return;
    }
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
    clearResourceCollections();
    envSwitching.value = false;
  } finally {
    if (isStaleView(id, sessionId, requestId)) return;
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
  favoriteNamespaces.value = loadFavoriteNamespaces();
  recentKinds.value = loadRecentKinds();
  recentNamespaces.value = loadRecentNamespaces(currentId.value);
  void ensureAppSettingsLoaded();
  const id = currentId.value;
  if (id) restoreEnvViewState(id);
  document.addEventListener("click", onDocClick);
  window.addEventListener("resize", adjustActionMenuPosition);
  window.addEventListener("scroll", adjustActionMenuPosition, true);
});
onUnmounted(() => {
  document.removeEventListener("click", onDocClick);
  window.removeEventListener("resize", adjustActionMenuPosition);
  window.removeEventListener("scroll", adjustActionMenuPosition, true);
  stopNodeAllocationPolling();
});
watch(selectedKind, () => {
  sortBy.value = "creationTime";
  sortOrder.value = "desc";
  nodeFilter.value = "all";
  podIpFilter.value = "";
});
watch(currentId, (id) => {
  beginEnvSwitch(id);
  recentNamespaces.value = loadRecentNamespaces(id);
  if (id) restoreEnvViewState(id);
});
watch(nsDropdownOpen, (open) => {
  if (!open) return;
  nextTick(() => {
    const active = nsMenuRef.value?.querySelector(".combobox-item.active");
    if (active && "scrollIntoView" in active) {
      (active as HTMLElement).scrollIntoView({ block: "nearest" });
    }
  });
});
watch(kindDropdownOpen, (open) => {
  if (!open) return;
  nextTick(() => {
    const active = kindMenuRef.value?.querySelector(".combobox-item.active");
    if (active && "scrollIntoView" in active) {
      (active as HTMLElement).scrollIntoView({ block: "nearest" });
    }
  });
});
watch([selectedNamespace, selectedKind], () => {
  const id = currentId.value;
  if (id) saveEnvViewState(id);
});
watch(
  [currentId, selectedKind, labelSelector, nodeResourceUsageEnabled],
  () => {
    syncNodeAllocationPolling();
  },
  { immediate: true }
);
watch(
  [currentId, selectedNamespace, selectedKind],
  () => {
    const id = currentId.value;
    if (watchEnabled.value && resourceSupportsWatch(selectedKind.value)) {
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
  const sessionId = viewSessionId.value;
  const watchToken = nextToken("watch");
  const ns =
    selectedKind.value === "namespaces" || selectedKind.value === "nodes" || resourceIsClusterScoped(selectedKind.value)
      ? null
      : (selectedNamespace.value ?? ALL_NAMESPACES_SENTINEL);
  const labelSel = labelSelector.value.trim() || null;
  const hasCache = applyCachedView(id, selectedKind.value, ns, labelSel);
  setWatchToken(id, watchToken);
  setWatchView(id, selectedKind.value, ns, labelSel);
  listLoading.value = !hasCache;
  listError.value = null;
  envSwitching.value = false;
  if (!namespaceCache.has(id)) void refreshNamespaceOptions();
  kubeStopWatch(id).catch(() => {});
  if (watchEnabled.value && resourceSupportsWatch(selectedKind.value)) {
    kubeStartWatch(id, selectedKind.value, ns, labelSel, watchToken).catch(async (e) => {
      if (isStaleView(id, sessionId)) return;
      const msg = extractErrorMessage(e);
      const isStrongholdRequired = await strongholdAuth.checkAndHandle(
        msg,
        () => applyWatch(),
        {
          title: "解锁环境凭证",
          description: "当前环境连接需要访问已保存凭证，请先输入 Stronghold 主密码解锁。",
        }
      );
      if (isStrongholdRequired) {
        setConnecting(id);
        return;
      }
      const isAuthRequired = await sshAuth.checkAndHandle(msg, () => applyWatch());
      if (isAuthRequired) {
        setConnecting(id);
        return;
      }
      clearWatchToken(id);
      listError.value = msg;
      if (isConnectionError(msg)) setDisconnected(id, msg);
    });
  }
}

watch(watchEnabled, () => {
  const id = currentId.value;
  if (!id) return;
  if (watchEnabled.value) {
    if (resourceSupportsWatch(selectedKind.value)) {
      applyWatch();
    } else {
      loadList();
    }
  } else {
    for (const env of openedEnvs.value) {
      kubeStopWatch(env.id).catch(() => {});
      clearWatchToken(env.id);
    }
    loadList();
  }
});

let unlistenWatch: (() => void) | null = null;
let unlistenConnection: (() => void) | null = null;
onMounted(async () => {
  unlistenConnection = await setupConnectionProgressListener();
  unlistenWatch = await listen<{ envId?: string; watchToken?: string; kind?: string; items?: unknown[]; error?: string }>(
    "resource-watch-update",
    (ev) => {
      const payload = ev.payload;
      if (!payload) return;
      const envId = payload.envId;
      if (!envId) return;
      if (payload.watchToken !== activeWatchTokens.value[envId]) return;
      if (payload?.error) {
        if (envId === currentId.value) {
          listError.value = payload.error;
          if (isConnectionError(payload.error)) setDisconnected(envId, payload.error);
        }
        return;
      }
      const kind = payload?.kind;
      const items = payload?.items ?? [];
      if (!kind) return;
      const watchView = activeWatchViews.value[envId];
      if (!watchView) return;
      const resourceKind = watchView.kind;
      const namespace = watchView.namespace;
      const labelSel = watchView.labelSelector;
      if (resourceKind === "namespaces") {
        namespaceCache.set(envId, [...(items as NamespaceItem[])]);
      }
      cacheCurrentView(envId, resourceKind, namespace, labelSel, items);
      if (
        envId !== currentId.value ||
        resourceKind !== selectedKind.value ||
        namespace !== (selectedKind.value === "namespaces" || selectedKind.value === "nodes" || resourceIsClusterScoped(selectedKind.value)
          ? null
          : (selectedNamespace.value ?? ALL_NAMESPACES_SENTINEL)) ||
        labelSel !== (labelSelector.value.trim() || null)
      ) return;
      listError.value = null;
      listLoading.value = false;
      envSwitching.value = false;
      clearResourceCollections();
      const cachedNamespaces = namespaceCache.get(envId);
      if (cachedNamespaces) {
        namespaceOptions.value = [...cachedNamespaces];
      }
      setResourceItems(resourceKind, items);
    }
  );
});
onUnmounted(() => {
  document.removeEventListener("click", onDocClick);
  unlistenConnection?.();
  unlistenWatch?.();
  stopNodeAllocationPolling();
  for (const env of openedEnvs.value) {
    kubeStopWatch(env.id).catch(() => {});
    clearWatchToken(env.id);
  }
});
</script>

<template>
  <div class="main-layout">
    <template v-if="openedEnvs.length">
      <EnvBar
        :collapsed="envBarCollapsed"
        :on-reconnect="handleReconnect"
        :on-open-terminal="openEnvironmentTerminal"
        @toggle="setEnvBarCollapsed(!envBarCollapsed)"
      />
      <div class="content">
        <nav v-if="currentId" class="breadcrumb-bar" aria-label="导航">
          <span class="breadcrumb-kicker">当前位置</span>
          <div class="breadcrumb-trail">
            <template v-if="drillFrom">
              <button
                type="button"
                class="breadcrumb-seg"
                :title="drillFrom!.namespace || '全部'"
                @click="selectedNamespace = drillFrom!.namespace; clearDrillAndReload()"
              >
                {{ drillFrom!.namespace || "全部" }}
              </button>
              <span class="breadcrumb-sep">›</span>
              <button type="button" class="breadcrumb-seg" :title="drillFrom!.kind" @click="onBreadcrumbKindClick()">
                {{ drillFrom!.kind }}
              </button>
              <span class="breadcrumb-sep">›</span>
              <button
                v-if="(API_KIND_TO_ID[drillFrom!.kind] ?? 'services') !== selectedKind"
                type="button"
                class="breadcrumb-seg"
                :title="drillFrom!.name"
                @click="onBreadcrumbResourceNameClick()"
              >
                {{ drillFrom!.name }}
              </button>
              <template v-if="(API_KIND_TO_ID[drillFrom!.kind] ?? 'services') !== selectedKind">
                <span class="breadcrumb-sep">›</span>
                <span class="breadcrumb-seg breadcrumb-current" :title="selectedKindLabel">{{ selectedKindLabel }}</span>
              </template>
              <span v-else class="breadcrumb-seg breadcrumb-current" :title="drillFrom!.name">{{ drillFrom!.name }}</span>
            </template>
            <template v-else>
              <span class="breadcrumb-seg breadcrumb-base" :title="effectiveNamespace">{{ effectiveNamespace }}</span>
              <span class="breadcrumb-sep">›</span>
              <span class="breadcrumb-seg breadcrumb-current" :title="selectedKindLabel">{{ selectedKindLabel }}</span>
            </template>
          </div>
        </nav>
        <header class="toolbar">
          <div class="toolbar-card">
            <div class="toolbar-main">
              <div v-if="currentEnv" class="active-env-banner">
                <div class="active-env-copy">
                  <div class="active-env-name-row">
                    <span class="active-env-kicker">当前环境</span>
                    <span class="env-name">{{ currentEnv.display_name }}</span>
                    <span class="active-env-state" :class="`active-env-state-${getState(currentId!)}`">
                      {{ currentEnvStateLabel() }}
                    </span>
                  </div>
                  <div class="active-env-meta-row">
                    <span class="active-env-chip">{{ currentEnvSourceLabel() }}</span>
                    <span v-if="shouldShowCurrentEnvContext()" class="active-env-chip subtle">
                      Context: {{ currentEnvContextLabel() }}
                    </span>
                  </div>
                </div>
              </div>
              <div v-if="currentId" ref="nsDropdownRef" class="combobox-wrap">
                <button
                  type="button"
                  class="combobox-trigger"
                  :class="{ open: nsDropdownOpen, disabled: nsSelectionDisabled, 'combobox-trigger-strong': true }"
                  :disabled="nsSelectionDisabled"
                  :title="nsSelectionDisabled ? '当前资源为集群级，命名空间不生效' : '命名空间：输入筛选后选择'"
                  @click="toggleNamespaceDropdown"
                >
                  <span class="combobox-trigger-main">
                    <span class="combobox-label">命名空间</span>
                    <span class="combobox-value">{{ nsSelectionDisabled ? '集群级资源' : effectiveNamespace }}</span>
                  </span>
                  <span class="combobox-arrow">▼</span>
                </button>
                <div v-show="nsDropdownOpen" ref="nsMenuRef" class="combobox-menu">
                  <div class="combobox-panel-head">
                    <div class="combobox-panel-title">选择命名空间</div>
                    <div class="combobox-panel-subtitle">当前资源范围会基于这里切换</div>
                  </div>
                  <div class="combobox-search">
                    <input
                      v-model="nsFilter"
                      type="text"
                      class="combobox-input"
                      placeholder="搜索命名空间…"
                      autocomplete="off"
                    />
                  </div>
                  <button type="button" class="combobox-item" :class="{ active: selectedNamespace === null }" @click="selectNamespace(null)">
                    <span class="combobox-item-main">
                      <span class="combobox-item-title">全部命名空间</span>
                      <span class="combobox-item-subtitle">浏览当前环境下的全部命名空间</span>
                    </span>
                    <span class="combobox-item-check">{{ selectedNamespace === null ? "✓" : "" }}</span>
                  </button>
                  <template v-if="namespaceFavorites.length > 0">
                    <div class="combobox-group-label">收藏</div>
                    <button
                      v-for="n in namespaceFavorites"
                      :key="`fav-${n.name}`"
                      type="button"
                      class="combobox-item combobox-item-with-action"
                      :class="{ active: selectedNamespace === n.name }"
                      @click="selectNamespace(n.name)"
                    >
                      <span class="combobox-item-main">
                        <span class="combobox-item-title">{{ n.name }}</span>
                        <span class="combobox-item-subtitle">收藏的命名空间</span>
                      </span>
                      <span class="combobox-item-trailing">
                        <span class="combobox-item-check">{{ selectedNamespace === n.name ? "✓" : "" }}</span>
                        <span
                          class="ns-star active"
                          title="取消收藏"
                          @click.stop="toggleFavoriteNamespace(n.name)"
                        >
                          ★
                        </span>
                      </span>
                    </button>
                  </template>
                  <template v-if="namespaceRecent.length > 0">
                    <div class="combobox-group-label">最近</div>
                    <button
                      v-for="n in namespaceRecent"
                      :key="`recent-${n.name}`"
                      type="button"
                      class="combobox-item combobox-item-with-action"
                      :class="{ active: selectedNamespace === n.name }"
                      @click="selectNamespace(n.name)"
                    >
                      <span class="combobox-item-main">
                        <span class="combobox-item-title">{{ n.name }}</span>
                        <span class="combobox-item-subtitle">最近访问</span>
                      </span>
                      <span class="combobox-item-trailing">
                        <span class="combobox-item-check">{{ selectedNamespace === n.name ? "✓" : "" }}</span>
                        <span
                          class="ns-star"
                          :class="{ active: favoriteNamespaces.has(n.name) }"
                          :title="favoriteNamespaces.has(n.name) ? '取消收藏' : '收藏'"
                          @click.stop="toggleFavoriteNamespace(n.name)"
                        >
                          ★
                        </span>
                      </span>
                    </button>
                  </template>
                  <div class="combobox-group-label">全部</div>
                  <button
                    v-for="n in namespaceOthers"
                    :key="n.name"
                    type="button"
                    class="combobox-item combobox-item-with-action"
                    :class="{ active: selectedNamespace === n.name }"
                    @click="selectNamespace(n.name)"
                  >
                    <span class="combobox-item-main">
                      <span class="combobox-item-title">{{ n.name }}</span>
                    </span>
                    <span class="combobox-item-trailing">
                      <span class="combobox-item-check">{{ selectedNamespace === n.name ? "✓" : "" }}</span>
                      <span
                        class="ns-star"
                        :class="{ active: favoriteNamespaces.has(n.name) }"
                        :title="favoriteNamespaces.has(n.name) ? '取消收藏' : '收藏'"
                        @click.stop="toggleFavoriteNamespace(n.name)"
                      >
                        ★
                      </span>
                    </span>
                  </button>
                </div>
              </div>
              <div ref="kindDropdownRef" class="combobox-wrap">
                <button
                  type="button"
                  class="combobox-trigger"
                  :class="{ open: kindDropdownOpen, 'combobox-trigger-strong': true }"
                  title="资源类型：输入筛选后选择"
                  @click="kindDropdownOpen = !kindDropdownOpen; if (kindDropdownOpen) { kindFilter = ''; nsDropdownOpen = false }"
                >
                  <span class="combobox-trigger-main">
                    <span class="combobox-label">资源类型</span>
                    <span class="combobox-value">{{ selectedKindLabel }}</span>
                  </span>
                  <span class="combobox-arrow">▼</span>
                </button>
                <div v-show="kindDropdownOpen" ref="kindMenuRef" class="combobox-menu combobox-menu-grouped">
                  <div class="combobox-panel-head">
                    <div class="combobox-panel-title">选择资源类型</div>
                    <div class="combobox-panel-subtitle">快速切换当前工作台的资源视图</div>
                  </div>
                  <div class="combobox-search">
                    <input
                      v-model="kindFilter"
                      type="text"
                      class="combobox-input"
                      placeholder="搜索资源类型…"
                      autocomplete="off"
                    />
                  </div>
                  <div v-if="recentKindItems.length > 0 && !kindFilter.trim()" class="recent-kind-panel">
                    <div class="recent-kind-title">最近使用</div>
                    <div class="recent-kind-list">
                      <button
                        v-for="k in recentKindItems"
                        :key="`recent-kind-${k.id}`"
                        type="button"
                        class="recent-kind-pill"
                        :class="{ active: selectedKind === k.id }"
                        @click="selectKindAndClearDrill(k.id)"
                      >
                        {{ k.label }}
                      </button>
                    </div>
                  </div>
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
                      <span class="combobox-item-main">
                        <span class="combobox-item-title">{{ k.label }}</span>
                      </span>
                      <span class="combobox-item-check">{{ selectedKind === k.id ? "✓" : "" }}</span>
                    </button>
                  </template>
                </div>
              </div>
              <div class="toolbar-actions">
                <button
                  v-if="currentId && resourceSupportsWatch(selectedKind)"
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
              </div>
            </div>
            <div v-if="currentId" class="toolbar-filters">
              <div class="toolbar-filters-primary">
                <input
                  v-model="nameFilter"
                  type="text"
                  class="filter-input"
                  placeholder="按名称筛选…"
                  autocomplete="off"
                  title="按名称包含匹配（前端过滤）"
                />
                <input
                  v-model="labelSelector"
                  type="text"
                  class="filter-input filter-input-label"
                  placeholder="Label 筛选，如 app=nginx"
                  autocomplete="off"
                  title="K8s label selector，如 app=nginx 或 env in (prod,staging)"
                  @keyup.enter="watchEnabled && resourceSupportsWatch(selectedKind) ? applyWatch() : loadList()"
                />
              </div>
              <div v-if="selectedKind === 'pods' || selectedKind === 'services'" class="toolbar-filters-secondary">
                <select
                  v-if="selectedKind === 'pods'"
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
                  v-if="supportsIpFilter()"
                  v-model="podIpFilter"
                  type="text"
                  class="filter-input"
                  :placeholder="ipFilterPlaceholder()"
                  autocomplete="off"
                  :title="ipFilterTitle()"
                />
              </div>
            </div>
            <div v-if="activeFilterChips.length" class="filter-chip-bar">
              <span class="filter-chip-label">已启用筛选</span>
              <button
                v-for="chip in activeFilterChips"
                :key="chip.id"
                type="button"
                class="filter-chip"
                @click="clearFilterChip(chip.id)"
                :title="`点击移除 ${chip.label} 筛选`"
              >
                {{ chip.label }}: {{ chip.value }}
                <span class="filter-chip-close" aria-hidden="true">×</span>
              </button>
              <button type="button" class="filter-chip-clear-all" @click="clearAllFilters">清除全部</button>
            </div>
          </div>
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
        <div v-else-if="listLoading" class="loading-state">
          <div class="loading-state-title">
            {{ envSwitching ? `正在切换到 ${envSwitchingName || "目标环境"}` : "加载中…" }}
          </div>
          <div class="loading-state-detail">
            {{ envSwitching ? "旧环境数据已清空，正在拉取新环境资源。" : "正在同步当前环境下的资源列表。" }}
          </div>
        </div>
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
                :class="{ 'row-selected': isSelectedRow(row) }"
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
                    <span
                      v-if="isStatusColumn(col.key)"
                      class="status-pill"
                      :class="`status-${statusTone(row[col.key as keyof typeof row])}`"
                    >
                      {{ normalizeStatus(row[col.key as keyof typeof row]) }}
                    </span>
                    <template v-else-if="isPodRollupColumn(col.key)">
                      <div class="pod-rollup-cell">
                        <template v-for="badge in buildPodRollupBadges(row[col.key as keyof typeof row])" :key="badge.key">
                          <span class="pod-rollup-badge" :class="`pod-rollup-badge-${badge.tone}`"><span class="pod-rollup-dot" />{{ badge.value }}</span>
                        </template>
                        <span v-if="buildPodRollupBadges(row[col.key as keyof typeof row]).length === 0" class="pod-rollup-empty">-</span>
                      </div>
                    </template>
                    <template v-else-if="col.key === 'recentRestart'">
                      <span :class="{ 'recent-restart-hot': isRecentRestartHot(row.podRollup) }">{{ formatRecentRestart(row.podRollup) }}</span>
                    </template>
                    <template v-else-if="isNodeAllocColumn(col.key)">
                      <span class="node-alloc-pill" :class="`node-alloc-pill-${nodeAllocTone(row[col.key as keyof typeof row])}`">
                        {{ row[col.key as keyof typeof row] }}
                      </span>
                    </template>
                    <template v-else-if="selectedKind === 'nodes' && col.key === 'taints'">
                      <button
                        type="button"
                        class="taint-entry-btn"
                        :class="{ 'taint-entry-btn-empty': row[col.key as keyof typeof row] === '无' }"
                        @click.stop="openNodeTaintsFromRow(row)"
                      >
                        <span class="taint-entry-label">污点</span>
                        <span class="taint-entry-value">{{ row[col.key as keyof typeof row] }}</span>
                      </button>
                    </template>
                    <template v-else>
                      {{ row[col.key as keyof typeof row] }}
                    </template>
                  </template>
                </td>
              </tr>
            </tbody>
          </table>
          <div v-if="!tableRows.length" class="empty-table">
            <div class="empty-emoji" aria-hidden="true">📭</div>
            <p class="empty-title">暂无资源</p>
            <p class="empty-desc">可尝试调整命名空间、资源类型或筛选条件。</p>
            <div class="empty-actions">
              <button type="button" class="btn-secondary-outline" @click="clearAllFilters">清空筛选</button>
              <button
                v-if="!nsSelectionDisabled && selectedNamespace !== null"
                type="button"
                class="btn-secondary-outline"
                @click="selectNamespace(null)"
              >
                切回默认命名空间
              </button>
              <button type="button" class="btn-secondary-outline" @click="openKindSelector">切换资源类型</button>
            </div>
          </div>
        </div>
      </div>
    </template>
    <div v-else class="empty-state">
      <div class="empty-emoji" aria-hidden="true">🌐</div>
      <p class="empty-title">暂无打开的环境</p>
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
            <div class="action-menu-section-title">查看与导航</div>
            <button type="button" class="action-menu-item" @click="openResourceDetail">
              <span class="action-menu-icon" aria-hidden="true">📄</span>
              <span class="action-menu-text">
                <span class="action-menu-main">查看详情</span>
                <span class="action-menu-sub">打开 YAML、Describe 与编辑面板</span>
              </span>
            </button>
            <button type="button" class="action-menu-item" @click="openTopology">
              <span class="action-menu-icon" aria-hidden="true">🧭</span>
              <span class="action-menu-text">
                <span class="action-menu-main">关联资源</span>
                <span class="action-menu-sub">查看上下游资源拓扑并快速跳转</span>
              </span>
            </button>
            <button
              v-if="selectedResource && ['Pod', 'Deployment', 'StatefulSet', 'DaemonSet'].includes(selectedResource.kind)"
              type="button"
              class="action-menu-item"
              @click="openPodLogs"
            >
              <span class="action-menu-icon" aria-hidden="true">📜</span>
              <span class="action-menu-text">
                <span class="action-menu-main">打开日志中心</span>
                <span class="action-menu-sub">集中查看 Pod 或工作负载日志</span>
              </span>
            </button>
            <button
              v-if="selectedResource && SHELL_WORKLOAD_KINDS.has(selectedResource.kind)"
              type="button"
              class="action-menu-item"
              @click="openPodShell"
            >
              <span class="action-menu-icon" aria-hidden="true">⌨</span>
              <span class="action-menu-text">
                <span class="action-menu-main">打开 Shell</span>
                <span class="action-menu-sub">进入容器执行命令与排障</span>
              </span>
            </button>
            <button
              v-if="selectedResource && NODE_TERMINAL_RESOURCE_KINDS.has(selectedResource.kind)"
              type="button"
              class="action-menu-item"
              :class="{ 'action-menu-item-disabled': !canOpenNodeTerminal }"
              :disabled="!canOpenNodeTerminal"
              :title="nodeTerminalDisabledReason || nodeTerminalMenuLabel"
              @click="openNodeTerminalFromMenu"
            >
              <span class="action-menu-icon" aria-hidden="true">🖥</span>
              <span class="action-menu-text">
                <span class="action-menu-main">{{ nodeTerminalMenuLabel }}</span>
                <span class="action-menu-sub">通过环境入口快速切换到目标节点主机</span>
              </span>
            </button>
            <button
              v-if="selectedResource?.kind === 'Pod'"
              type="button"
              class="action-menu-item"
              :class="{ 'action-menu-item-disabled': !canOpenPodDebug }"
              :disabled="!canOpenPodDebug"
              :title="podDebugDisabledReason || '进入容器调试环境'"
              @click="openPodDebugModal"
            >
              <span class="action-menu-icon" aria-hidden="true">🧪</span>
              <span class="action-menu-text">
                <span class="action-menu-main">进入容器调试环境</span>
                <span class="action-menu-sub">通过 nsenter 进入目标容器的网络或完整隔离空间</span>
              </span>
            </button>
          </div>
          <div class="action-menu-section">
            <div class="action-menu-section-title">编辑与变更</div>
            <button
              v-if="selectedResource && (selectedResource.kind === 'ConfigMap' || selectedResource.kind === 'Secret')"
              type="button"
              class="action-menu-item"
              @click="openEditConfig"
            >
              <span class="action-menu-icon" aria-hidden="true">⚙</span>
              <span class="action-menu-text">
                <span class="action-menu-main">修改配置</span>
                <span class="action-menu-sub">编辑 ConfigMap / Secret 内容</span>
              </span>
            </button>
            <button
              v-if="selectedResource && IMAGE_PATCH_KINDS.has(selectedResource.kind)"
              type="button"
              class="action-menu-item"
              @click="openChangeImageModal"
            >
              <span class="action-menu-icon" aria-hidden="true">🧩</span>
              <span class="action-menu-text">
                <span class="action-menu-main">修改镜像</span>
                <span class="action-menu-sub">更新工作负载容器镜像版本</span>
              </span>
            </button>
            <button type="button" class="action-menu-item" @click="openSyncToOrchestratorDialog">
              <span class="action-menu-icon" aria-hidden="true">🧱</span>
              <span class="action-menu-text">
                <span class="action-menu-main">编排中心</span>
                <span class="action-menu-sub">同步到编排中心并统一维护 YAML</span>
              </span>
            </button>
          </div>
          <div class="action-menu-section action-menu-section-danger">
            <div class="action-menu-section-title">危险操作</div>
            <button
              type="button"
              class="action-menu-item action-menu-item-danger"
              :class="{ 'action-menu-item-danger-armed': deleteActionArmed }"
              @click="handleDeleteAction"
            >
              <span class="action-menu-icon" aria-hidden="true">🗑</span>
              <span class="action-menu-text">
                <span class="action-menu-main">
                  {{ deleteActionArmed ? "再次点击确认删除" : "删除" }}
                </span>
                <span class="action-menu-sub">
                  {{
                    deleteActionArmed
                      ? "将打开删除确认弹窗，避免误操作"
                      : "高风险操作，资源删除后通常不可恢复"
                  }}
                </span>
              </span>
            </button>
          </div>
        </div>
      </div>
    </Teleport>

    <Teleport to="body">
        <div v-if="syncOrchestratorDialogVisible" class="error-modal-overlay" @click.self="closeSyncToOrchestratorDialog">
        <div class="sync-orchestrator-modal" role="dialog" aria-label="同步到编排中心">
          <h3 class="sync-orchestrator-title">同步到编排中心</h3>
          <p class="sync-orchestrator-desc">
            选择将当前资源同步到哪个应用组件，可用于继续维护已有组件或新建组件。
          </p>
          <div class="sync-orchestrator-mode-row">
            <label class="sync-radio">
              <input v-model="syncOrchestratorMode" type="radio" value="new" />
              新增应用组件
            </label>
            <label class="sync-radio">
              <input v-model="syncOrchestratorMode" type="radio" value="existing" :disabled="orchestratorComponentsForCurrentEnv.length === 0" />
              加入已有应用组件
            </label>
          </div>
          <label v-if="syncOrchestratorMode === 'existing'" class="sync-field">
            <span>已有组件</span>
            <select v-model="syncOrchestratorExistingComponent" class="filter-input" :disabled="orchestratorComponentsForCurrentEnv.length === 0">
              <option value="" disabled>选择组件</option>
              <option v-for="name in orchestratorComponentsForCurrentEnv" :key="name" :value="name">
                {{ name }}
              </option>
            </select>
          </label>
          <label v-else class="sync-field">
            <span>新组件名称</span>
            <input v-model="syncOrchestratorNewComponent" type="text" class="filter-input" placeholder="输入应用组件名称" />
          </label>
          <div class="sync-related-panel">
            <div class="sync-related-head">
              <span>关联资源</span>
              <small v-if="syncRelatedTotalCount > 0">已选 {{ syncSelectedRelatedCount }} / {{ syncRelatedTotalCount }}</small>
            </div>
            <div v-if="syncOrchestratorLoadingRelated" class="sync-related-empty">正在解析关联资源…</div>
            <div v-else-if="syncOrchestratorRelatedError" class="sync-related-error">{{ syncOrchestratorRelatedError }}</div>
            <template v-else>
              <label v-if="syncRelatedTotalCount > 0" class="sync-select-all">
                <input
                  type="checkbox"
                  :checked="syncAllRelatedSelected"
                  @change="toggleAllSyncRelatedRefs(($event.target as HTMLInputElement).checked)"
                />
                全选关联资源
              </label>
              <div v-if="syncRelatedTotalCount > 0" class="sync-related-list">
                <label v-for="ref in syncOrchestratorRelatedRefs" :key="relatedRefKey(ref)" class="sync-related-item">
                  <input v-model="syncOrchestratorSelectedRefKeys" type="checkbox" :value="relatedRefKey(ref)" />
                  <span>{{ ref.kind }}/{{ ref.name }}</span>
                  <small>{{ ref.namespace || "default" }}</small>
                </label>
              </div>
              <div v-else class="sync-related-empty">当前资源未检测到可关联同步的 ConfigMap/Secret/Service。</div>
            </template>
          </div>
          <div class="sync-orchestrator-actions">
            <button type="button" class="btn-secondary-outline" @click="closeSyncToOrchestratorDialog">取消</button>
            <button
              type="button"
              class="btn-primary"
              :disabled="(syncOrchestratorMode === 'existing' && !syncOrchestratorExistingComponent) || syncOrchestratorLoadingRelated"
              @click="syncToOrchestrator"
            >
              确认同步
            </button>
          </div>
        </div>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-if="podDebugModalVisible" class="error-modal-overlay" @click.self="closePodDebugModal">
        <div class="pod-debug-modal" role="dialog" aria-label="进入容器调试环境">
          <h3 class="sync-orchestrator-title">进入容器调试环境</h3>
          <p class="sync-orchestrator-desc">
            先通过节点终端策略进入 Pod 所在主机，再按你勾选的 namespace 组合执行 `nsenter`。这样可以保留主机工具，同时进入目标容器的关键隔离空间。
          </p>
          <div class="pod-debug-grid">
            <label class="sync-field">
              <span>容器</span>
              <select v-model="podDebugSelectedContainer" class="filter-input pod-debug-input" :disabled="podDebugLoading || !podDebugContainerOptions.length">
                <option value="" disabled>选择容器</option>
                <option v-for="name in podDebugContainerOptions" :key="name" :value="name">
                  {{ name }}
                </option>
              </select>
            </label>
            <div class="sync-field">
              <span>进程目标</span>
              <div class="pod-debug-radio-row">
                <label class="pod-debug-radio">
                  <input v-model="podDebugProcessMode" type="radio" value="main" />
                  <span>容器主进程</span>
                </label>
                <label class="pod-debug-radio">
                  <input v-model="podDebugProcessMode" type="radio" value="pid" />
                  <span>指定 PID</span>
                </label>
              </div>
            </div>
            <label v-if="podDebugProcessMode === 'pid'" class="sync-field">
              <span>PID</span>
              <input
                v-model="podDebugPidInput"
                type="text"
                class="filter-input pod-debug-input"
                inputmode="numeric"
                placeholder="输入目标进程 PID"
              />
            </label>
          </div>
          <div class="pod-debug-section">
            <div class="pod-debug-section-title">
              <span>Namespace 组合</span>
              <small>至少保留一个，推荐先勾选 `网络`</small>
            </div>
            <div class="pod-debug-option-grid">
              <button
                v-for="item in POD_DEBUG_NAMESPACE_OPTIONS"
                :key="item.value"
                type="button"
                class="pod-debug-option"
                :class="{ active: podDebugNamespaces.includes(item.value) }"
                @click="togglePodDebugNamespace(item.value)"
              >
                <div class="pod-debug-option-head">
                  <span class="pod-debug-option-title">
                    {{ item.label }}
                    <span v-if="item.recommended" class="pod-debug-badge">推荐</span>
                  </span>
                  <span class="pod-debug-option-check">{{ podDebugNamespaces.includes(item.value) ? "✓" : "" }}</span>
                </div>
                <div class="pod-debug-option-desc">{{ item.description }}</div>
              </button>
            </div>
          </div>
          <p class="pod-debug-summary">
            当前组合：{{ podDebugNamespaces.join(" + ") }}，{{ podDebugProcessMode === "main" ? "默认进入容器主进程" : `按指定 PID ${podDebugPidInput || "..." } 进入` }}。
          </p>
          <p v-if="podDebugError" class="form-error">{{ podDebugError }}</p>
          <div class="pod-debug-actions">
            <button type="button" class="btn-secondary-outline pod-debug-cancel-btn" @click="closePodDebugModal">取消</button>
            <button
              type="button"
              class="btn-primary pod-debug-primary-btn"
              :disabled="podDebugLoading || !podDebugSelectedContainer || !!podDebugError || (podDebugProcessMode === 'pid' && !podDebugPidInput.trim())"
              @click="confirmOpenPodDebug"
            >
              进入调试终端
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
  padding: 0.75rem 1rem;
  border-bottom: 1px solid #e2e8f0;
  background: #f8fafc;
  display: block;
  flex-shrink: 0;
}
.toolbar-card {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.65rem 0.75rem;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  background: #fff;
}
.toolbar-main {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem;
}
.toolbar-filters {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem;
  padding-top: 0.1rem;
}
.toolbar-filters-primary,
.toolbar-filters-secondary {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem;
}
.toolbar-filters-secondary {
  margin-left: auto;
}
.toolbar-actions {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
}
.env-name {
  font-weight: 800;
  font-size: 0.88rem;
  color: #0f172a;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.active-env-banner {
  display: inline-flex;
  align-items: center;
  flex: 0 1 auto;
  width: fit-content;
  max-width: min(100%, 300px);
  min-width: 0;
  padding: 0.4rem 0.55rem;
  border-radius: 12px;
  border: 1px solid rgba(37, 99, 235, 0.14);
  background:
    radial-gradient(circle at top right, rgba(14, 165, 233, 0.14), transparent 26%),
    linear-gradient(135deg, #eff6ff, #f8fafc 72%);
}
.active-env-copy {
  min-width: 0;
  width: auto;
}
.active-env-kicker {
  display: inline-flex;
  align-items: center;
  padding: 0.12rem 0.34rem;
  border-radius: 999px;
  background: rgba(37, 99, 235, 0.1);
  font-size: 0.6rem;
  font-weight: 700;
  color: #2563eb;
  flex-shrink: 0;
}
.active-env-name-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.38rem;
}
.active-env-meta-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.24rem;
  margin-top: 0.28rem;
}
.active-env-chip {
  display: inline-flex;
  align-items: center;
  padding: 0.12rem 0.36rem;
  border-radius: 999px;
  background: rgba(37, 99, 235, 0.1);
  color: #1d4ed8;
  font-size: 0.64rem;
  font-weight: 700;
  max-width: 100%;
}
.active-env-chip.subtle {
  background: rgba(255, 255, 255, 0.78);
  color: #475569;
  border: 1px solid rgba(148, 163, 184, 0.22);
}
.active-env-state {
  display: inline-flex;
  align-items: center;
  padding: 0.12rem 0.34rem;
  border-radius: 999px;
  font-size: 0.62rem;
  font-weight: 700;
  flex-shrink: 0;
}
.active-env-state-connected,
.active-env-state-error {
  background: #ecfdf5;
  color: #15803d;
}
.active-env-state-connecting {
  background: #e0f2fe;
  color: #0369a1;
}
.active-env-state-disconnected {
  background: #fef2f2;
  color: #dc2626;
}
.breadcrumb-bar {
  display: flex;
  align-items: center;
  gap: 0.55rem;
  padding: 0.55rem 1rem;
  font-size: 0.8rem;
  color: #64748b;
  background:
    linear-gradient(180deg, #f8fafc, #f1f5f9);
  border-bottom: 1px solid #e2e8f0;
  flex-shrink: 0;
  min-width: 0;
}
.breadcrumb-kicker {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  padding: 0.14rem 0.45rem;
  border-radius: 999px;
  background: #e2e8f0;
  color: #475569;
  font-size: 0.68rem;
  font-weight: 700;
  letter-spacing: 0.03em;
}
.breadcrumb-trail {
  display: flex;
  align-items: center;
  gap: 0.28rem;
  min-width: 0;
  overflow: auto hidden;
  padding-bottom: 0.05rem;
  scrollbar-width: none;
}
.breadcrumb-trail::-webkit-scrollbar {
  display: none;
}
.breadcrumb-seg {
  display: inline-flex;
  align-items: center;
  max-width: 260px;
  background: rgba(255, 255, 255, 0.9);
  border: 1px solid rgba(203, 213, 225, 0.9);
  padding: 0.28rem 0.55rem;
  border-radius: 999px;
  cursor: pointer;
  color: inherit;
  font: inherit;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  box-shadow: 0 1px 1px rgba(15, 23, 42, 0.02);
}
.breadcrumb-seg:hover:not(.breadcrumb-current):not(.breadcrumb-base) {
  background: #fff;
  border-color: #93c5fd;
  color: #1d4ed8;
}
.breadcrumb-seg.breadcrumb-base {
  background: transparent;
  border-color: transparent;
  color: #64748b;
  cursor: default;
  box-shadow: none;
}
.breadcrumb-seg.breadcrumb-current {
  background: #dbeafe;
  border-color: #bfdbfe;
  color: #0f172a;
  font-weight: 700;
  cursor: default;
}
.breadcrumb-sep {
  color: #94a3b8;
  user-select: none;
  flex-shrink: 0;
}
.action-menu-backdrop {
  position: fixed;
  inset: 0;
  z-index: 999;
}
.action-menu-overlay {
  position: fixed;
  z-index: 1000;
  width: min(320px, calc(100vw - 20px));
  max-width: calc(100vw - 20px);
  max-height: calc(100vh - 20px);
  padding: 0.5rem 0;
  background: #fff;
  border: 1px solid #e2e8f0;
  border-radius: 12px;
  box-shadow: 0 14px 32px rgba(15, 23, 42, 0.18);
  overflow: auto;
  overscroll-behavior: contain;
}
.action-menu-section {
  padding: 0 0.25rem;
}
.action-menu-section:not(:last-child) {
  margin-bottom: 0.35rem;
  padding-bottom: 0.35rem;
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
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
  width: 100%;
  padding: 0.45rem 0.75rem;
  border: none;
  background: none;
  font-size: 0.8125rem;
  text-align: left;
  color: #334155;
  cursor: pointer;
  border-radius: 4px;
}
.action-menu-icon {
  width: 1rem;
  text-align: center;
  opacity: 0.9;
  margin-top: 0.1rem;
}
.action-menu-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.action-menu-main {
  color: inherit;
  line-height: 1.2;
}
.action-menu-sub {
  margin-top: 0.12rem;
  font-size: 0.72rem;
  line-height: 1.25;
  color: #94a3b8;
  word-break: break-word;
}
.action-menu-item:hover .action-menu-sub {
  color: #64748b;
}
.action-menu-item:hover {
  background: #f1f5f9;
  color: #2563eb;
}
.action-menu-item-disabled {
  cursor: not-allowed;
  opacity: 0.72;
}
.action-menu-item-disabled:hover {
  background: none;
  color: #334155;
}
.action-menu-item-disabled:hover .action-menu-sub {
  color: #94a3b8;
}
.action-menu-section-danger {
  border-left: 2px solid #fecaca;
  margin-left: 0.25rem;
  padding-left: 0.25rem;
}
.action-menu-item-danger:hover {
  background: #fef2f2;
  color: #dc2626;
}
@media (max-width: 960px) {
  .active-env-banner {
    display: flex;
    width: 100%;
    max-width: none;
  }
  .toolbar-filters-secondary {
    margin-left: 0;
  }
}
.action-menu-item-danger:hover .action-menu-sub {
  color: #b91c1c;
}
.action-menu-item-danger-armed {
  background: #fee2e2;
  color: #b91c1c;
  box-shadow: inset 2px 0 0 #dc2626;
}
.action-menu-item-danger-armed .action-menu-sub {
  color: #b91c1c;
}
.action-menu-loading {
  padding: 0.4rem 0.75rem;
  font-size: 0.8125rem;
  color: #94a3b8;
}
@media (max-width: 640px) {
  .action-menu-overlay {
    width: min(300px, calc(100vw - 16px));
    max-width: calc(100vw - 16px);
    max-height: calc(100vh - 16px);
    padding: 0.35rem 0;
    border-radius: 10px;
  }
  .action-menu-section {
    padding: 0 0.18rem;
  }
  .action-menu-item {
    gap: 0.42rem;
    padding: 0.42rem 0.62rem;
  }
  .action-menu-main {
    font-size: 0.78rem;
  }
  .action-menu-sub {
    font-size: 0.68rem;
  }
}
.combobox-wrap {
  position: relative;
}
.combobox-trigger {
  display: inline-flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.55rem;
  padding: 0.48rem 0.7rem;
  border: 1px solid #e2e8f0;
  border-radius: 12px;
  background: #fff;
  font-size: 0.8125rem;
  color: #475569;
  cursor: pointer;
  min-width: 0;
  min-height: 48px;
  box-shadow: 0 1px 2px rgba(15, 23, 42, 0.03);
}
.combobox-trigger:hover {
  background: #f8fafc;
  border-color: #cbd5e1;
}
.combobox-trigger-strong {
  min-width: 190px;
}
.combobox-trigger.disabled {
  opacity: 0.6;
  cursor: not-allowed;
  background: #f8fafc;
}
.combobox-trigger.open {
  border-color: #2563eb;
  background: #eff6ff;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
}
.combobox-trigger-main {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  min-width: 0;
}
.combobox-label {
  color: #64748b;
  font-size: 0.68rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
}
.combobox-value {
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: #0f172a;
  font-weight: 700;
  line-height: 1.25;
}
.combobox-arrow {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.4rem;
  height: 1.4rem;
  border-radius: 999px;
  background: #f1f5f9;
  font-size: 0.62rem;
  color: #64748b;
  flex-shrink: 0;
}
.combobox-menu {
  position: absolute;
  top: calc(100% + 8px);
  left: 0;
  min-width: max(280px, 100%);
  width: fit-content;
  max-width: min(560px, calc(100vw - 32px));
  max-height: 420px;
  overflow-y: auto;
  overflow-x: hidden;
  background: #fff;
  border: 1px solid #e2e8f0;
  border-radius: 16px;
  box-shadow: 0 18px 40px rgba(15, 23, 42, 0.16);
  padding: 0.3rem 0;
  z-index: 100;
  display: flex;
  flex-direction: column;
}
.combobox-panel-head {
  padding: 0.55rem 0.8rem 0.25rem;
}
.combobox-panel-title {
  font-size: 0.86rem;
  font-weight: 700;
  color: #0f172a;
}
.combobox-panel-subtitle {
  margin-top: 0.16rem;
  font-size: 0.72rem;
  color: #64748b;
}
.combobox-search {
  padding: 0.2rem 0.6rem 0.35rem;
}
.combobox-input {
  width: 100%;
  box-sizing: border-box;
  padding: 0.55rem 0.7rem;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  font-size: 0.8125rem;
  background: #f8fafc;
}
.combobox-input:focus {
  outline: none;
  border-color: #2563eb;
  background: #fff;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
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
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.12);
}
.filter-input-label {
  min-width: 160px;
  max-width: 220px;
}
.combobox-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: calc(100% - 0.7rem);
  box-sizing: border-box;
  gap: 0.5rem;
  padding: 0.52rem 0.8rem;
  border: none;
  background: none;
  font-size: 0.8125rem;
  text-align: left;
  cursor: pointer;
  color: #334155;
  border-radius: 10px;
  margin: 0 0.35rem;
}
.combobox-item-with-action {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.combobox-item-main {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
}
.combobox-item-title {
  white-space: nowrap;
}
.combobox-item-subtitle {
  margin-top: 0.1rem;
  font-size: 0.71rem;
  color: #94a3b8;
  white-space: nowrap;
}
.combobox-item-trailing {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  flex-shrink: 0;
}
.combobox-item-check {
  width: 1rem;
  text-align: center;
  color: #2563eb;
  font-weight: 800;
  flex-shrink: 0;
}
.ns-star {
  font-size: 0.875rem;
  line-height: 1;
  color: #cbd5e1;
  padding: 0.1rem 0.2rem;
  border-radius: 4px;
}
.ns-star.active {
  color: #f59e0b;
}
.ns-star:hover {
  background: #f1f5f9;
}
.combobox-item:hover {
  background: #f8fafc;
}
.combobox-item.active {
  background: rgba(37, 99, 235, 0.09);
  color: #1d4ed8;
  font-weight: 600;
}
.combobox-group-label {
  padding: 0.45rem 0.85rem 0.22rem;
  font-size: 0.6875rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: #94a3b8;
  border-top: 1px solid #f1f5f9;
}
.recent-kind-panel {
  margin: 0.15rem 0.6rem 0.45rem;
  padding: 0.45rem 0.5rem 0.5rem;
  border: 1px solid #dbeafe;
  border-radius: 12px;
  background: #f0f7ff;
}
.recent-kind-title {
  margin: 0.05rem 0.15rem 0.32rem;
  font-size: 0.71rem;
  font-weight: 600;
  color: #3b82f6;
  letter-spacing: 0.03em;
}
.recent-kind-list {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.3rem;
}
.recent-kind-pill {
  border: 1px solid #bfdbfe;
  background: #eff6ff;
  color: #1d4ed8;
  border-radius: 999px;
  padding: 0.2rem 0.6rem;
  font-size: 0.75rem;
  line-height: 1.2;
  cursor: pointer;
}
.recent-kind-pill:hover {
  background: #dbeafe;
}
.recent-kind-pill.active {
  border-color: #3b82f6;
  background: #bfdbfe;
  color: #1e40af;
  font-weight: 600;
}
.combobox-group-label:first-of-type {
  border-top: none;
  padding-top: 0.25rem;
}
.resource-table tbody tr.row-clickable {
  cursor: pointer;
  background: #fff;
}
.resource-table tbody tr.row-clickable:hover {
  background: #f7faff;
}
.resource-table tbody tr.row-clickable.row-selected {
  background: #ecf4ff;
  box-shadow: inset 3px 0 0 #2563eb;
}
.btn-refresh {
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
  border-bottom: 1px solid #fecaca;
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
.sync-orchestrator-modal {
  width: min(92vw, 520px);
  background: #fff;
  border-radius: 12px;
  border: 1px solid #cbd5e1;
  padding: 1rem;
  box-shadow: 0 20px 40px rgba(0, 0, 0, 0.15);
}
.sync-orchestrator-title {
  margin: 0 0 0.35rem;
  font-size: 1rem;
  color: #1e293b;
}
.sync-orchestrator-desc {
  margin: 0 0 0.75rem;
  font-size: 0.82rem;
  color: #64748b;
}
.sync-orchestrator-mode-row {
  display: flex;
  align-items: center;
  gap: 0.8rem;
  margin-bottom: 0.7rem;
}
.sync-radio {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.82rem;
  color: #334155;
}
.sync-field {
  display: grid;
  gap: 0.35rem;
  margin-bottom: 0.85rem;
  font-size: 0.8rem;
  color: #334155;
}
.sync-related-panel {
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  background: #f8fafc;
  padding: 0.6rem;
  margin-bottom: 0.85rem;
}
.sync-related-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.45rem;
  font-size: 0.8rem;
  color: #334155;
}
.sync-related-head small {
  color: #64748b;
}
.sync-select-all {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.76rem;
  color: #334155;
  margin-bottom: 0.45rem;
}
.sync-related-list {
  display: grid;
  gap: 0.3rem;
  max-height: 180px;
  overflow: auto;
}
.sync-related-item {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  background: #fff;
  padding: 0.3rem 0.4rem;
  font-size: 0.76rem;
  color: #0f172a;
}
.sync-related-item small {
  margin-left: auto;
  color: #64748b;
}
.sync-related-empty {
  font-size: 0.76rem;
  color: #64748b;
}
.sync-related-error {
  font-size: 0.76rem;
  color: #b91c1c;
  background: #fef2f2;
  border: 1px solid #fecaca;
  border-radius: 6px;
  padding: 0.35rem 0.45rem;
}
.sync-orchestrator-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
}
.pod-debug-modal {
  width: min(92vw, 680px);
  background: linear-gradient(180deg, #ffffff 0%, #f8fafc 100%);
  border-radius: 18px;
  border: 1px solid #dbe4ee;
  padding: 1.1rem;
  box-shadow: 0 24px 64px rgba(15, 23, 42, 0.22);
}
.pod-debug-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.9rem 1rem;
}
.pod-debug-input {
  min-width: 0;
  max-width: none;
  width: 100%;
  box-sizing: border-box;
}
.pod-debug-radio-row {
  display: flex;
  gap: 0.7rem;
  flex-wrap: wrap;
}
.pod-debug-radio {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  padding: 0.55rem 0.7rem;
  border: 1px solid #dbe4ee;
  border-radius: 12px;
  background: #fff;
  font-size: 0.82rem;
  color: #334155;
}
.pod-debug-section {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  padding: 0.9rem;
  border: 1px solid #e2e8f0;
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.92);
  margin-bottom: 0.85rem;
}
.pod-debug-section-title {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 0.75rem;
}
.pod-debug-section-title span {
  font-size: 0.92rem;
  font-weight: 700;
  color: #0f172a;
}
.pod-debug-section-title small {
  color: #64748b;
  font-size: 0.76rem;
}
.pod-debug-option-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.75rem;
}
.pod-debug-option {
  text-align: left;
  border: 1px solid #dbe4ee;
  border-radius: 14px;
  background: #fff;
  padding: 0.85rem 0.9rem;
  cursor: pointer;
  transition: border-color 120ms ease, box-shadow 120ms ease;
}
.pod-debug-option:hover {
  border-color: #93c5fd;
  box-shadow: 0 10px 22px rgba(59, 130, 246, 0.08);
}
.pod-debug-option.active {
  border-color: #2563eb;
  background: #eff6ff;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.1);
}
.pod-debug-option-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}
.pod-debug-option-title {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font-weight: 700;
  color: #0f172a;
}
.pod-debug-badge {
  display: inline-flex;
  align-items: center;
  padding: 0.08rem 0.38rem;
  border-radius: 999px;
  background: #dcfce7;
  color: #166534;
  font-size: 0.68rem;
  font-weight: 700;
}
.pod-debug-option-check {
  width: 1.1rem;
  text-align: center;
  font-weight: 800;
  color: #2563eb;
}
.pod-debug-option-desc {
  margin-top: 0.45rem;
  font-size: 0.78rem;
  line-height: 1.45;
  color: #64748b;
}
.pod-debug-summary {
  margin: -0.15rem 0 0.6rem;
  font-size: 0.8rem;
  color: #475569;
}
.pod-debug-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.75rem;
  margin-top: 0.9rem;
}
.pod-debug-cancel-btn {
  padding: 0.6rem 0.95rem;
  border-radius: 12px;
}
.pod-debug-primary-btn {
  min-width: 12.5rem;
  min-height: 3rem;
  padding: 0.8rem 1.4rem;
  border: none;
  border-radius: 14px;
  background: linear-gradient(135deg, #2563eb 0%, #1d4ed8 100%);
  color: #fff;
  font-size: 0.92rem;
  font-weight: 700;
  letter-spacing: 0.01em;
  box-shadow: 0 14px 28px rgba(37, 99, 235, 0.22);
}
.pod-debug-primary-btn:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 16px 30px rgba(37, 99, 235, 0.26);
}
.pod-debug-primary-btn:disabled {
  background: linear-gradient(135deg, #93c5fd 0%, #60a5fa 100%);
  color: rgba(255, 255, 255, 0.96);
  box-shadow: none;
  opacity: 0.72;
}
@media (max-width: 640px) {
  .pod-debug-grid,
  .pod-debug-option-grid {
    grid-template-columns: 1fr;
  }
  .pod-debug-actions {
    flex-direction: column-reverse;
    align-items: stretch;
  }
  .pod-debug-cancel-btn,
  .pod-debug-primary-btn {
    width: 100%;
  }
}
.loading-state {
  padding: 2rem 1.5rem;
  text-align: center;
  color: #64748b;
  font-size: 0.875rem;
  background: #fff;
}
.loading-state-title {
  font-size: 0.95rem;
  font-weight: 600;
  color: #334155;
}
.loading-state-detail {
  margin-top: 0.45rem;
  font-size: 0.8125rem;
  color: #64748b;
}
.table-wrap {
  flex: 1;
  overflow: auto;
  padding: 0.9rem 1rem 1rem;
  background: #f8fafc;
}
.resource-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.8125rem;
  background: #fff;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  overflow: hidden;
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
  position: sticky;
  top: 0;
  z-index: 1;
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
.resource-table tbody tr:nth-child(odd),
.resource-table tbody tr:nth-child(even) {
  background: #fff;
}
.resource-table td.cell-drillable {
  cursor: pointer;
  color: #2563eb;
}
.resource-table td.cell-drillable:hover {
  text-decoration: underline;
}
.status-pill {
  display: inline-flex;
  align-items: center;
  padding: 0.12rem 0.48rem;
  border-radius: 999px;
  font-size: 0.75rem;
  line-height: 1.2;
  font-weight: 600;
  border: 1px solid transparent;
}
.status-pill.status-ok {
  color: #15803d;
  background: #f0fdf4;
  border-color: #bbf7d0;
}
.status-pill.status-warn {
  color: #b45309;
  background: #fffbeb;
  border-color: #fde68a;
}
.status-pill.status-error {
  color: #b91c1c;
  background: #fef2f2;
  border-color: #fecaca;
}
.status-pill.status-neutral {
  color: #475569;
  background: #f8fafc;
  border-color: #e2e8f0;
}

.pod-rollup-cell {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
}

.pod-rollup-badge {
  display: inline-flex;
  align-items: center;
  border-radius: 999px;
  padding: 2px 8px;
  font-size: 12px;
  font-weight: 600;
  line-height: 1.3;
  border: 1px solid transparent;
}
.pod-rollup-dot {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: currentColor;
  margin-right: 6px;
  opacity: 0.9;
}

.pod-rollup-badge-running {
  background: rgba(34, 197, 94, 0.16);
  color: #166534;
  border-color: rgba(34, 197, 94, 0.35);
}

.pod-rollup-badge-pending {
  background: rgba(245, 158, 11, 0.16);
  color: #92400e;
  border-color: rgba(245, 158, 11, 0.35);
}

.pod-rollup-badge-succeeded {
  background: rgba(100, 116, 139, 0.16);
  color: #334155;
  border-color: rgba(100, 116, 139, 0.35);
}

.pod-rollup-badge-failed {
  background: rgba(239, 68, 68, 0.16);
  color: #991b1b;
  border-color: rgba(239, 68, 68, 0.35);
}

.pod-rollup-badge-abnormal {
  background: rgba(190, 24, 93, 0.18);
  color: #9f1239;
  border-color: rgba(190, 24, 93, 0.35);
}

.pod-rollup-empty {
  color: rgba(15, 23, 42, 0.45);
}

.recent-restart-hot {
  color: #b91c1c;
  font-weight: 600;
}
.resource-table .cell-link {
  cursor: pointer;
  color: #2563eb;
}
.resource-table .cell-link:hover {
  text-decoration: underline;
}
.taint-entry-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  min-height: 1.85rem;
  padding: 0 0.55rem;
  border: 1px solid #bfdbfe;
  border-radius: 999px;
  background: #eff6ff;
  color: #1d4ed8;
  cursor: pointer;
  font: inherit;
}
.taint-entry-btn:hover {
  background: #dbeafe;
  border-color: #93c5fd;
}
.taint-entry-btn-empty {
  border-color: #cbd5e1;
  background: #f8fafc;
  color: #475569;
}
.taint-entry-btn-empty:hover {
  background: #f1f5f9;
  border-color: #94a3b8;
}
.taint-entry-label {
  font-size: 0.68rem;
  font-weight: 700;
  letter-spacing: 0.04em;
}
.taint-entry-value {
  font-size: 0.78rem;
  font-weight: 700;
}
.node-alloc-pill {
  display: inline-flex;
  align-items: center;
  padding: 0.2rem 0.55rem;
  border-radius: 999px;
  background: #f8fafc;
  color: #334155;
  font-size: 0.75rem;
  line-height: 1.3;
  white-space: nowrap;
}
.node-alloc-pill-warn {
  background: #fef3c7;
  color: #b45309;
}
.node-alloc-pill-danger {
  background: #fee2e2;
  color: #b91c1c;
}
.empty-table {
  margin: 1rem 0 0;
  border: 1px dashed #cbd5e1;
  border-radius: 10px;
  background: #fff;
  padding: 1.25rem 1rem;
  text-align: center;
}
.empty-actions {
  margin-top: 0.8rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.45rem;
  flex-wrap: wrap;
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
  background: #f8fafc;
}
.empty-state p {
  margin: 0;
}
.empty-emoji {
  font-size: 1.5rem;
  line-height: 1;
}
.empty-title {
  margin: 0;
  font-size: 0.95rem;
  color: #334155;
  font-weight: 600;
}
.empty-desc {
  font-size: 0.8125rem;
  color: #94a3b8;
}
.filter-chip-bar {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  flex-wrap: wrap;
}
.filter-chip-label {
  font-size: 0.75rem;
  color: #64748b;
}
.filter-chip {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  border: 1px solid #bfdbfe;
  border-radius: 999px;
  padding: 0.2rem 0.55rem;
  font-size: 0.75rem;
  color: #1d4ed8;
  background: #eff6ff;
  cursor: pointer;
}
.filter-chip:hover {
  background: #dbeafe;
}
.filter-chip-close {
  opacity: 0.75;
}
.filter-chip-clear-all {
  border: none;
  background: transparent;
  color: #64748b;
  font-size: 0.75rem;
  cursor: pointer;
  text-decoration: underline;
  padding: 0.1rem 0.2rem;
}
</style>
