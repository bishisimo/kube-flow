<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick, type Ref } from "vue";

defineOptions({ name: "Main" });
import * as jsYaml from "js-yaml";
import { extractErrorMessage } from "../utils/errorMessage";
import { createStorage } from "../utils/storage";
import { useEnvStore } from "../stores/env";
import EnvBar from "../components/EnvBar.vue";
import WorkbenchBreadcrumb from "../components/workbench/WorkbenchBreadcrumb.vue";
import WorkbenchToolbar from "../components/workbench/WorkbenchToolbar.vue";
import WorkbenchResourceFrame from "../components/workbench/WorkbenchResourceFrame.vue";
import WorkbenchResourceTable from "../components/workbench/WorkbenchResourceTable.vue";
import { RESOURCE_GROUPS, RESOURCE_KINDS_FLAT, type ResourceKind } from "../constants/resourceKinds";
import { resourceKindMatchesSearch } from "../constants/resourceAliases";
import {
  WORKBENCH_ACTION_MENU_OFFSET,
  WORKBENCH_ACTION_MENU_VIEWPORT_GAP,
  WORKBENCH_ENV_BAR_COLLAPSED_KEY,
  WORKBENCH_IMAGE_PATCH_KINDS,
  WORKBENCH_NODE_ALLOC_REFRESH_MS,
  WORKBENCH_NODE_TERMINAL_RESOURCE_KINDS,
  WORKBENCH_POD_DEBUG_NAMESPACE_OPTIONS,
  WORKBENCH_SHELL_WORKLOAD_KINDS,
  buildApiKindToIdMap,
  buildValidResourceKindSet,
  useWorkbenchCustomResource,
  useWorkbenchResourceCollections,
  useWorkbenchLoadList,
  useWorkbenchTableModel,
  useWorkbenchTableDecorators,
  useWorkbenchResourceWatch,
  useWorkbenchFilterUi,
  getWorkbenchResourceDescriptor,
  useWorkbenchRecents,
  useWorkbenchWatch,
  useWorkbenchListUi,
  useWorkbenchNavigation,
} from "../features/workbench";
import ResourceDetailDrawer from "../components/ResourceDetailDrawer.vue";
import ChangeImageModal from "../components/ChangeImageModal.vue";
import DeleteConfirmModal from "../components/DeleteConfirmModal.vue";
import {
  kubeDeleteDynamicResource,
  kubeDeleteResource,
  kubeRemoveClient,
} from "../api/kube";
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
  type ResolvedAliasTarget,
  type NamespaceItem,
  type NodeItem,
} from "../api/kube";
import type { PodDebugNamespace } from "../api/terminal";
import { defaultNamespace } from "../api/env";
import {
  collectAssociatedRefsFromWorkloadYaml,
  type SyncRelatedKind,
  type SyncRelatedRef,
} from "../features/workbench/utils/parseWorkloadRefs";

const RESOURCE_KINDS = RESOURCE_KINDS_FLAT;
const VALID_KINDS = buildValidResourceKindSet(RESOURCE_KINDS);
const API_KIND_TO_ID = buildApiKindToIdMap(RESOURCE_KINDS);

const { openedEnvs, currentEnv, currentId, touchEnv, loadEnvironments, getEnvViewState, setEnvViewState } = useEnvStore();
const {
  favoriteNamespaces,
  recentNamespaces,
  recentKinds,
  touchRecentNamespace,
  toggleFavoriteNamespace,
  touchRecentKind,
  hydrateFromStorage,
  loadRecentNamespacesForEnv,
} = useWorkbenchRecents({ currentId, validKindIds: VALID_KINDS });
const {
  watchEnabled,
  activeWatchTokens,
  activeWatchViews,
  createWatchToken,
  setWatchToken,
  setWatchView,
  clearWatchToken,
} = useWorkbenchWatch();
const {
  listLoading,
  listError,
  envSwitching,
  envSwitchingName,
  dismissError,
  beginListSwitch,
} = useWorkbenchListUi();
const {
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
  clearResourceCollections,
  setResourceItems,
  cacheCurrentView,
  applyCachedView,
} = useWorkbenchResourceCollections({
  listLoading,
  listError,
  envSwitching,
});
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

const envBarCollapsedStorage = createStorage<boolean>({
  key: WORKBENCH_ENV_BAR_COLLAPSED_KEY,
  version: 1,
  fallback: true,
  migrate: (old) => old === "1" || old === 1 || old === true,
});
const envBarCollapsed = ref(envBarCollapsedStorage.read());
function setEnvBarCollapsed(v: boolean) {
  envBarCollapsed.value = v;
  envBarCollapsedStorage.write(v);
}

const selectedNamespace = ref<string | null>(null);
const selectedKind = ref<ResourceKind>("namespaces");
const selectedCustomTarget = ref<ResolvedAliasTarget | null>(null);

/** 钻取来源：从某资源跳转到关联列表时保留，用于面包屑与侧栏点击时清除 */
const drillFrom = ref<{ kind: string; name: string; namespace: string | null } | null>(null);

function restoreEnvViewState(envId: string) {
  selectedCustomTarget.value = null;
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
  touchRecentKind(kind);
  navigateTo({ kind, customTarget: null, nameFilter: "", drillFrom: null, reload: false, closeDrawer: false });
}

function selectCustomKindOption(target: ResolvedAliasTarget) {
  customResourceResolveMessage.value = `→ ${target.api_version} ${target.kind} · ${target.plural} · ${
    target.namespaced ? "命名空间资源" : "集群资源"
  }`;
  navigateTo({ customTarget: target, nameFilter: "", drillFrom: null, closeDrawer: false });
}

/** 仅清除钻取上下文并刷新（面包屑点击 namespace 时） */
function clearDrillAndReload() {
  navigateTo({ drillFrom: null, nameFilter: "", closeDrawer: false });
}

/** 面包屑点击「资源名」：跳转到该类型资源列表并预填名称筛选，保留 drillFrom 以维持面包屑 */
function onBreadcrumbResourceNameClick() {
  if (!drillFrom.value) return;
  const kindId = API_KIND_TO_ID[drillFrom.value.kind] ?? "services";
  navigateTo({ kind: kindId, namespace: drillFrom.value.namespace, nameFilter: drillFrom.value.name, closeDrawer: false });
}

/** 面包屑点击「资源类型」：跳转到该类型资源列表（全量） */
function onBreadcrumbKindClick() {
  if (!drillFrom.value) return;
  const kindId = API_KIND_TO_ID[drillFrom.value.kind] ?? "services";
  navigateTo({ kind: kindId, namespace: drillFrom.value.namespace, drillFrom: null, nameFilter: "", closeDrawer: false });
}

/** 面包屑钻取路径下点击命名空间段：恢复该命名空间上下文并刷新列表 */
function onDrillBreadcrumbNamespace() {
  if (!drillFrom.value) return;
  selectedNamespace.value = drillFrom.value.namespace;
  clearDrillAndReload();
}

function saveEnvViewState(envId: string) {
  setEnvViewState(envId, {
    namespace: selectedNamespace.value,
    kind: selectedKind.value,
  });
}

const nsDropdownOpen = ref(false);
const kindDropdownOpen = ref(false);
const kindFilter = ref("");
const {
  aliasDiscoveryForEnvId,
  aliasDiscoveryLoading,
  aliasDiscoveryError,
  customResourceQuery,
  customResourceHits,
  customResourceResolveMessage,
  customResourceHintLine,
  customResourceStatusClass,
  resetCustomResourceInline,
  scheduleCustomResourceResolve,
  primeAliasDiscoveryForCurrentEnv,
} = useWorkbenchCustomResource({
  currentId,
  kindFilter,
  getState,
});
const workbenchToolbarRef = ref<InstanceType<typeof WorkbenchToolbar> | null>(null);

/** 子组件 expose 的模板 ref 在运行时为 Ref，类型侧可能已解包，这里统一取 DOM。 */
function toolbarHTMLElement(
  tb: NonNullable<InstanceType<typeof WorkbenchToolbar>>,
  key: "nsDropdownRef" | "kindDropdownRef" | "nsMenuRef" | "kindMenuRef"
): HTMLElement | null {
  const v = (tb as unknown as Record<string, unknown>)[key];
  if (v == null) return null;
  if (typeof v === "object" && v !== null && "value" in v) {
    return (v as Ref<HTMLElement | null>).value;
  }
  return v as HTMLElement;
}
const nsFilter = ref("");
/** 按名称筛选：前端过滤，支持模糊匹配（包含） */
const nameFilter = ref("");
/** 按 Node 筛选：仅在 Pods 视图生效，默认 all（不过滤） */
const nodeFilter = ref("all");
/** 按 Pod IP 筛选：仅在 Pods 视图生效，支持包含匹配 */
const podIpFilter = ref("");
/** 按 label 筛选：传给 K8s API，格式如 app=nginx 或 env in (prod,staging) */
const labelSelector = ref("");
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

const sortBy = ref<string>("creationTime");
const sortOrder = ref<"asc" | "desc">("desc");
const viewSessionId = ref(0);
const latestListRequestId = ref(0);

const { tableColumns, tableRows, onSortColumn } = useWorkbenchTableModel({
  selectedKind,
  selectedCustomTarget,
  nodeResourceUsageEnabled,
  nodeAllocations,
  nameFilter,
  nodeFilter,
  podIpFilter,
  sortBy,
  sortOrder,
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
});

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
  }, WORKBENCH_NODE_ALLOC_REFRESH_MS);
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
  beginListSwitch(
    nextEnvId,
    nextEnvId ? (openedEnvs.value.find((env) => env.id === nextEnvId)?.display_name ?? "新环境") : ""
  );
}

function isStaleView(envId: string, sessionId: number, requestId?: number): boolean {
  if (currentId.value !== envId) return true;
  if (viewSessionId.value !== sessionId) return true;
  if (typeof requestId === "number" && latestListRequestId.value !== requestId) return true;
  return false;
}

const { loadList } = useWorkbenchLoadList({
  currentId,
  currentEnv,
  selectedNamespace,
  selectedKind,
  selectedCustomTarget,
  labelSelector,
  listError,
  listLoading,
  envSwitching,
  latestListRequestId,
  viewSessionId,
  isStaleView,
  getState,
  setConnecting,
  setConnected,
  setDisconnected,
  touchEnv,
  loadEnvironments,
  namespaceCache,
  clearResourceCollections,
  namespaceOptions,
  dynamicCrdItems,
  setResourceItems,
  cacheCurrentView,
  applyCachedView,
  strongholdAuth,
  sshAuth,
});

function onDocClick(e: MouseEvent) {
  const target = e.target as Node;
  const tb = workbenchToolbarRef.value;
  const nsRoot = tb ? toolbarHTMLElement(tb, "nsDropdownRef") : null;
  const kindRoot = tb ? toolbarHTMLElement(tb, "kindDropdownRef") : null;
  if (nsDropdownOpen.value && nsRoot && !nsRoot.contains(target)) nsDropdownOpen.value = false;
  if (kindDropdownOpen.value && kindRoot && !kindRoot.contains(target)) kindDropdownOpen.value = false;
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
  if (!kindFilter.value.trim()) return RESOURCE_GROUPS;
  return RESOURCE_GROUPS.map((g) => ({
    ...g,
    kinds: g.kinds.filter((k) => resourceKindMatchesSearch(k.id, k.label, kindFilter.value)),
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
const workbenchKindLabel = computed(() => {
  const target = selectedCustomTarget.value;
  if (!target) return selectedKindLabel.value;
  return `${target.kind}（CRD）`;
});
const customKindCandidates = computed(() => {
  const q = kindFilter.value.trim();
  if (!q) return [] as ResolvedAliasTarget[];
  const dedup = new Map<string, ResolvedAliasTarget>();
  for (const hit of customResourceHits.value) {
    const key = `${hit.api_version}/${hit.kind}/${hit.plural}`;
    if (!dedup.has(key)) dedup.set(key, hit);
  }
  return Array.from(dedup.values()).slice(0, 8);
});
const nsSelectionDisabled = computed(() => {
  const target = selectedCustomTarget.value;
  if (target) return !target.namespaced;
  return getWorkbenchResourceDescriptor(selectedKind.value).capabilities.clusterScoped;
});

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
  selectedCustomTarget.value = null;
  kindDropdownOpen.value = true;
  kindFilter.value = "";
  nsDropdownOpen.value = false;
  nextTick(() => {
    const tb = workbenchToolbarRef.value;
    const menu = tb ? toolbarHTMLElement(tb, "kindMenuRef") : null;
    const active = menu?.querySelector(".combobox-item.active");
    if (active && "scrollIntoView" in active) {
      (active as HTMLElement).scrollIntoView({ block: "nearest" });
    }
  });
}

/** 集群级资源，无需 namespace；CRD 等动态资源带 dynamic 走专用 API */
type SelectedResourceRef = {
  kind: string;
  name: string;
  namespace: string | null;
  nodeName: string | null;
  dynamic?: { api_version: string; namespaced: boolean };
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
const syncOrchestratorRelatedRefs = ref<SyncRelatedRef[]>([]);
const syncOrchestratorSelectedRefKeys = ref<string[]>([]);
const syncOrchestratorLoadingRelated = ref(false);
const syncOrchestratorRelatedError = ref<string | null>(null);

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
const deleteConfirmResources = ref<SelectedResourceRef[]>([]);
const deleteConfirmDeleting = ref(false);
const deleteConfirmError = ref<string | null>(null);
const selectedRowKeys = ref<Set<string>>(new Set());
/** 批量删除模式：为 true 时显示复选框列 */
const batchDeleteMode = ref(false);

const { navigateTo } = useWorkbenchNavigation({
  selectedKind,
  selectedCustomTarget,
  selectedNamespace,
  labelSelector,
  nameFilter,
  nodeFilter,
  podIpFilter,
  kindDropdownOpen,
  detailDrawerVisible,
  selectedRowKeys,
  batchDeleteMode,
  drillFrom,
  loadList,
});

/** 操作菜单：点击行时显示，区分资源管理与资源钻取 */
const actionMenuVisible = ref(false);
const actionMenuPosition = ref({ x: 0, y: 0 });
const actionMenuRef = ref<HTMLElement | null>(null);
const deleteActionArmed = ref(false);

function selectResourceFromRow(row: Record<string, unknown>): SelectedResourceRef | null {
  const name = row.name as string | undefined;
  if (!name) return null;
  const crd = selectedCustomTarget.value;
  if (crd) {
    const envDefaultNs = currentEnv.value ? defaultNamespace(currentEnv.value) : null;
    const rowNs = typeof row.ns === "string" && row.ns !== "—" ? row.ns : null;
    return {
      kind: crd.kind,
      name,
      namespace: crd.namespaced ? (rowNs ?? envDefaultNs ?? "default") : null,
      nodeName: null,
      dynamic: { api_version: crd.api_version, namespaced: crd.namespaced },
    };
  }
  const kindLabel = RESOURCE_KINDS.find((k) => k.id === selectedKind.value)?.label ?? "";
  if (!kindLabel) return null;
  const isCluster = getWorkbenchResourceDescriptor(selectedKind.value).capabilities.clusterScoped;
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
  const crd = selectedCustomTarget.value;
  if (crd) {
    if (!crd.namespaced) return name;
    const envDefaultNs = currentEnv.value ? defaultNamespace(currentEnv.value) : null;
    const rowNs = typeof row.ns === "string" && row.ns !== "—" ? row.ns : null;
    return `${rowNs ?? envDefaultNs ?? "default"}/${name}`;
  }
  const isCluster = getWorkbenchResourceDescriptor(selectedKind.value).capabilities.clusterScoped;
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
    x: e.clientX + WORKBENCH_ACTION_MENU_OFFSET,
    y: e.clientY + WORKBENCH_ACTION_MENU_OFFSET,
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
    WORKBENCH_ACTION_MENU_VIEWPORT_GAP,
    window.innerWidth - rect.width - WORKBENCH_ACTION_MENU_VIEWPORT_GAP
  );
  const maxY = Math.max(
    WORKBENCH_ACTION_MENU_VIEWPORT_GAP,
    window.innerHeight - rect.height - WORKBENCH_ACTION_MENU_VIEWPORT_GAP
  );
  actionMenuPosition.value = {
    x: Math.min(Math.max(WORKBENCH_ACTION_MENU_VIEWPORT_GAP, actionMenuPosition.value.x), maxX),
    y: Math.min(Math.max(WORKBENCH_ACTION_MENU_VIEWPORT_GAP, actionMenuPosition.value.y), maxY),
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
  if (!resource || !WORKBENCH_NODE_TERMINAL_RESOURCE_KINDS.has(resource.kind)) return "";
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
        WORKBENCH_NODE_TERMINAL_RESOURCE_KINDS.has(selectedResource.value.kind) &&
        !nodeTerminalDisabledReason.value
    )
);

function openPodShell() {
  const r = selectedResource.value;
  if (!r || !WORKBENCH_SHELL_WORKLOAD_KINDS.has(r.kind) || !currentId.value || !currentEnv.value) return;
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
    podDebugError.value = extractErrorMessage(e);
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
  podDebugNamespaces.value = WORKBENCH_POD_DEBUG_NAMESPACE_OPTIONS
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

function enterBatchDeleteMode() {
  batchDeleteMode.value = true;
  selectedRowKeys.value = new Set();
}

function exitBatchDeleteMode() {
  batchDeleteMode.value = false;
  selectedRowKeys.value = new Set();
}

function openBatchDeleteConfirm() {
  if (selectedCustomTarget.value) return;
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
      if (r.dynamic) {
        await kubeDeleteDynamicResource(envId, r.dynamic.api_version, r.kind, r.name, r.namespace);
      } else {
        await kubeDeleteResource(envId, r.kind, r.name, r.namespace);
      }
    } catch (e) {
      failed.push(`${r.namespace ? `${r.namespace}/` : ""}${r.name}: ${extractErrorMessage(e)}`);
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
  navigateTo({
    kind: payload.targetKind as ResourceKind,
    namespace: payload.namespace,
    labelSelector: payload.labelSelector ?? "",
    nameFilter: payload.resourceName ?? "",
    drillFrom: { kind: source.kind, name: source.name, namespace: source.namespace },
  });
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
  const isRole = kind === "Role";
  navigateTo({
    kind: isRole ? "roles" : "clusterroles",
    namespace: isRole ? ((row.ns as string) ?? selectedNamespace.value) : null,
    nameFilter: name,
    drillFrom: {
      kind: isRole ? "RoleBinding" : "ClusterRoleBinding",
      name: String(row.name),
      namespace: (row.ns as string) ?? null,
    },
  });
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
  navigateTo({
    kind: "serviceaccounts",
    namespace: subject.namespace ?? (row.ns as string) ?? selectedNamespace.value,
    nameFilter: subject.name,
    drillFrom: { kind: sourceKind, name: String(row.name), namespace: (row.ns as string) ?? null },
  });
}

function getSubjectLabelForTable(s: unknown, row: Record<string, unknown>): string {
  return getSubjectLabel(s as { kind: string; name: string; namespace?: string | null }, row);
}

function onSubjectClickForTable(row: Record<string, unknown>, s: unknown) {
  onSubjectClick(row, s as { kind: string; name: string; namespace?: string | null });
}

/** PVC 单元格点击：volume 跳转 PV，storageClass 跳转 StorageClass */
function onPvcCellClick(row: Record<string, unknown>, colKey: string) {
  if (colKey === "volume") {
    const name = row.volume as string;
    if (!name || name === "-") return;
    navigateTo({
      kind: "persistentvolumes",
      namespace: null,
      nameFilter: name,
      drillFrom: { kind: "PersistentVolumeClaim", name: String(row.name), namespace: (row.ns as string) ?? null },
    });
  } else if (colKey === "storageClass") {
    const name = row.storageClass as string;
    if (!name || name === "-") return;
    navigateTo({
      kind: "storageclasses",
      namespace: null,
      nameFilter: name,
      drillFrom: { kind: "PersistentVolumeClaim", name: String(row.name), namespace: (row.ns as string) ?? null },
    });
  }
}

/** 副本数点击：跳转到 Pods 并筛选该 workload 管理的 Pod */
function onReplicasClick(row: Record<string, unknown>) {
  const ls = row.labelSelector as string | null | undefined;
  if (!ls || !row.name) return;
  const kindLabel = RESOURCE_KINDS.find((k) => k.id === selectedKind.value)?.label ?? "";
  if (!kindLabel) return;
  navigateTo({
    kind: "pods",
    namespace: (row.ns as string) ?? selectedNamespace.value,
    labelSelector: ls,
    nameFilter: "",
    drillFrom: { kind: kindLabel, name: String(row.name), namespace: (row.ns as string) ?? null },
  });
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


function isSelectedRow(row: Record<string, unknown>): boolean {
  if (!selectedResource.value) return false;
  const resource = selectResourceFromRow(row);
  if (!resource) return false;
  const cur = selectedResource.value;
  const dynMatch =
    (!!resource.dynamic && !!cur.dynamic && resource.dynamic.api_version === cur.dynamic.api_version) ||
    (!resource.dynamic && !cur.dynamic);
  return (
    dynMatch &&
    resource.kind === cur.kind &&
    resource.name === cur.name &&
    resource.namespace === cur.namespace
  );
}


async function handleReconnect(envId: string) {
  await kubeRemoveClient(envId);
  setConnecting(envId);
  if (currentId.value === envId) {
    loadList();
  }
}

let unlistenConnection: (() => void) | null = null;

onMounted(async () => {
  hydrateFromStorage();
  void ensureAppSettingsLoaded();
  const id = currentId.value;
  if (id) restoreEnvViewState(id);
  document.addEventListener("click", onDocClick);
  window.addEventListener("resize", adjustActionMenuPosition);
  window.addEventListener("scroll", adjustActionMenuPosition, true);
  unlistenConnection = await setupConnectionProgressListener();
});

onUnmounted(() => {
  document.removeEventListener("click", onDocClick);
  window.removeEventListener("resize", adjustActionMenuPosition);
  window.removeEventListener("scroll", adjustActionMenuPosition, true);
  unlistenConnection?.();
  stopNodeAllocationPolling();
});

watch(selectedKind, () => {
  selectedCustomTarget.value = null;
  sortBy.value = "creationTime";
  sortOrder.value = "desc";
  nodeFilter.value = "all";
  podIpFilter.value = "";
});
watch(currentId, (id) => {
  selectedCustomTarget.value = null;
  beginEnvSwitch(id);
  loadRecentNamespacesForEnv(id);
  if (id) restoreEnvViewState(id);
});
watch(nsDropdownOpen, (open) => {
  if (!open) return;
  nextTick(() => {
    const tb = workbenchToolbarRef.value;
    const menu = tb ? toolbarHTMLElement(tb, "nsMenuRef") : null;
    const active = menu?.querySelector(".combobox-item.active");
    if (active && "scrollIntoView" in active) {
      (active as HTMLElement).scrollIntoView({ block: "nearest" });
    }
  });
});
watch(kindDropdownOpen, (open) => {
  if (!open) {
    customResourceHits.value = [];
    customResourceResolveMessage.value = "";
    return;
  }
  nextTick(() => {
    const tb = workbenchToolbarRef.value;
    const menu = tb ? toolbarHTMLElement(tb, "kindMenuRef") : null;
    const active = menu?.querySelector(".combobox-item.active");
    if (active && "scrollIntoView" in active) {
      (active as HTMLElement).scrollIntoView({ block: "nearest" });
    }
  });
});
watch(kindFilter, (q) => {
  if (!kindDropdownOpen.value) return;
  const query = q.trim();
  if (!query) {
    customResourceQuery.value = "";
    customResourceHits.value = [];
    customResourceResolveMessage.value = "";
    return;
  }
  scheduleCustomResourceResolve(false);
});
watch([selectedNamespace, selectedKind], () => {
  const id = currentId.value;
  if (id) saveEnvViewState(id);
});

watch(
  () => ({
    id: currentId.value,
    state: currentId.value ? getState(currentId.value) : null,
  }),
  async ({ id, state }) => {
    if (!id) {
      aliasDiscoveryForEnvId.value = null;
      aliasDiscoveryError.value = null;
      aliasDiscoveryLoading.value = false;
      resetCustomResourceInline();
      return;
    }
    if (state !== "connected") {
      aliasDiscoveryForEnvId.value = null;
      if (kindFilter.value.trim()) {
        customResourceResolveMessage.value =
          state === "connecting" ? "连接中…" : "未连接或已断开，重连后将自动同步发现";
        customResourceHits.value = [];
      } else {
        customResourceResolveMessage.value = "";
      }
      return;
    }
    await primeAliasDiscoveryForCurrentEnv();
  },
  { immediate: true }
);
watch(
  [currentId, selectedKind, labelSelector, nodeResourceUsageEnabled],
  () => {
    syncNodeAllocationPolling();
  },
  { immediate: true }
);

const { applyWatch } = useWorkbenchResourceWatch({
  currentId,
  selectedNamespace,
  selectedKind,
  selectedCustomTarget,
  labelSelector,
  watchEnabled,
  listLoading,
  listError,
  envSwitching,
  activeWatchTokens,
  activeWatchViews,
  createWatchToken,
  setWatchToken,
  setWatchView,
  clearWatchToken,
  namespaceCache,
  namespaceOptions,
  clearResourceCollections,
  setResourceItems,
  cacheCurrentView,
  applyCachedView,
  openedEnvs,
  loadList,
  refreshNamespaceOptions,
  viewSessionId,
  isStaleView,
  strongholdAuth,
  sshAuth,
  setConnecting,
  setDisconnected,
});

const {
  supportsIpFilter,
  ipFilterPlaceholder,
  ipFilterTitle,
  activeFilterChips,
  clearAllFilters,
  applyLabelFilterFromToolbar,
  onToolbarClearFilterChip,
} = useWorkbenchFilterUi({
  selectedKind,
  selectedCustomTarget,
  watchEnabled,
  nameFilter,
  nodeFilter,
  podIpFilter,
  labelSelector,
  applyWatch,
  loadList,
});

const {
  normalizeStatus,
  statusTone,
  isStatusColumn,
  isPodRollupColumn,
  buildPodRollupBadges,
  formatRecentRestart,
  isRecentRestartHot,
  isNodeAllocColumn,
  nodeAllocTone,
} = useWorkbenchTableDecorators();
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
        <WorkbenchBreadcrumb
          v-if="currentId"
          :drill-from="drillFrom"
          :api-kind-to-id="API_KIND_TO_ID"
          :workbench-kind-label="workbenchKindLabel"
          :effective-namespace="effectiveNamespace"
          :selected-kind="selectedKind"
          @drill-namespace="onDrillBreadcrumbNamespace"
          @breadcrumb-kind="onBreadcrumbKindClick"
          @breadcrumb-resource="onBreadcrumbResourceNameClick"
        />
        <WorkbenchToolbar
          ref="workbenchToolbarRef"
          v-model:ns-filter="nsFilter"
          v-model:kind-filter="kindFilter"
          v-model:name-filter="nameFilter"
          v-model:label-selector="labelSelector"
          v-model:node-filter="nodeFilter"
          v-model:pod-ip-filter="podIpFilter"
          v-model:watch-enabled="watchEnabled"
          v-model:ns-dropdown-open="nsDropdownOpen"
          v-model:kind-dropdown-open="kindDropdownOpen"
          :current-env="currentEnv"
          :current-id="currentId"
          :env-connection-state="currentId ? getState(currentId) : 'disconnected'"
          :current-env-state-label="currentEnvStateLabel()"
          :current-env-source-label="currentEnvSourceLabel()"
          :current-env-context-label="currentEnvContextLabel()"
          :should-show-current-env-context="shouldShowCurrentEnvContext()"
          :ns-selection-disabled="nsSelectionDisabled"
          :effective-namespace="effectiveNamespace"
          :selected-namespace="selectedNamespace"
          :namespace-favorites="namespaceFavorites"
          :namespace-recent="namespaceRecent"
          :namespace-others="namespaceOthers"
          :favorite-namespaces="favoriteNamespaces"
          :workbench-kind-label="workbenchKindLabel"
          :custom-resource-hint-line="customResourceHintLine"
          :custom-resource-status-class="customResourceStatusClass"
          :recent-kind-items="recentKindItems"
          :filtered-kind-groups="filteredKindGroups"
          :selected-kind="selectedKind"
          :selected-custom-target="selectedCustomTarget"
          :custom-kind-candidates="customKindCandidates"
          :list-loading="listLoading"
          :show-batch-toolbar="!!(currentId && !selectedCustomTarget)"
          :batch-delete-mode="batchDeleteMode"
          :selected-row-count="selectedRowKeys.size"
          :pod-node-options="podNodeOptions"
          :supports-ip-filter="supportsIpFilter()"
          :selected-kind-for-ip="selectedKind"
          :ip-filter-placeholder="ipFilterPlaceholder()"
          :ip-filter-title="ipFilterTitle()"
          :active-filter-chips="activeFilterChips"
          @toggle-namespace="toggleNamespaceDropdown"
          @select-namespace="selectNamespace"
          @toggle-favorite-namespace="toggleFavoriteNamespace"
          @select-kind="selectKindAndClearDrill"
          @select-custom-kind="selectCustomKindOption"
          @refresh="loadList"
          @enter-batch-delete="enterBatchDeleteMode"
          @exit-batch-delete="exitBatchDeleteMode"
          @open-batch-delete-confirm="openBatchDeleteConfirm"
          @apply-label-filter="applyLabelFilterFromToolbar"
          @clear-filter-chip="onToolbarClearFilterChip"
          @clear-all-filters="clearAllFilters"
        />
        <WorkbenchResourceFrame
          :current-id="currentId"
          :env-state="currentId ? getState(currentId) : null"
          :env-error="currentId ? getError(currentId) : undefined"
          :progress="currentId ? getProgress(currentId) : undefined"
          :list-error="listError"
          :list-loading="listLoading"
          :env-switching="envSwitching"
          :env-switching-name="envSwitchingName"
          :reconnect-on-list-error="!!(listError && currentId && isConnectionError(listError))"
          @reconnect="() => currentId && handleReconnect(currentId)"
          @dismiss-error="dismissError"
        >
          <WorkbenchResourceTable
            :table-rows="tableRows"
            :table-columns="tableColumns"
            :batch-delete-mode="batchDeleteMode"
            :selected-row-keys="selectedRowKeys"
            :sort-by="sortBy"
            :sort-order="sortOrder"
            :selected-kind="selectedKind"
            :ns-selection-disabled="nsSelectionDisabled"
            :selected-namespace="selectedNamespace"
            :get-row-key="getRowKey"
            :toggle-select-all="toggleSelectAll"
            :toggle-row-selection="toggleRowSelection"
            :on-row-click="onRowClick"
            :on-row-context-menu="onRowContextMenu"
            :on-namespace-row-dbl-click="onNamespaceRowClick"
            :is-selected-row="isSelectedRow"
            :on-sort-column="onSortColumn"
            :is-cell-drillable="isCellDrillable"
            :on-replicas-click="onReplicasClick"
            :on-role-ref-click="onRoleRefClick"
            :on-pvc-cell-click="onPvcCellClick"
            :get-subjects-list="getSubjectsList"
            :get-subject-label="getSubjectLabelForTable"
            :on-subject-click="onSubjectClickForTable"
            :is-status-column="isStatusColumn"
            :status-tone="statusTone"
            :normalize-status="normalizeStatus"
            :is-pod-rollup-column="isPodRollupColumn"
            :build-pod-rollup-badges="buildPodRollupBadges"
            :format-recent-restart="formatRecentRestart"
            :is-recent-restart-hot="isRecentRestartHot"
            :is-node-alloc-column="isNodeAllocColumn"
            :node-alloc-tone="nodeAllocTone"
            :open-node-taints-from-row="openNodeTaintsFromRow"
            :clear-all-filters="clearAllFilters"
            :select-all-namespaces="() => selectNamespace(null)"
            :open-kind-selector="openKindSelector"
          />
        </WorkbenchResourceFrame>
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
              v-if="selectedResource && WORKBENCH_SHELL_WORKLOAD_KINDS.has(selectedResource.kind)"
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
              v-if="selectedResource && WORKBENCH_NODE_TERMINAL_RESOURCE_KINDS.has(selectedResource.kind)"
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
              v-if="selectedResource && WORKBENCH_IMAGE_PATCH_KINDS.has(selectedResource.kind)"
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
                v-for="item in WORKBENCH_POD_DEBUG_NAMESPACE_OPTIONS"
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
/* 弹层内表单控件（Teleport 仍挂在本组件下，需保留样式） */
.filter-input {
  padding: 0.35rem 0.6rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  font-size: 0.8125rem;
  min-width: 120px;
  max-width: 160px;
  box-sizing: border-box;
}
.filter-input:focus {
  outline: none;
  border-color: #2563eb;
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.12);
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
.btn-primary {
  padding: 0.45rem 0.9rem;
  border: none;
  border-radius: 6px;
  background: #2563eb;
  color: #fff;
  font-size: 0.8125rem;
  font-weight: 600;
  cursor: pointer;
}
.btn-primary:hover:not(:disabled) {
  background: #1d4ed8;
}
.btn-primary:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
.form-error {
  margin: 0.35rem 0 0;
  font-size: 0.8rem;
  color: #b91c1c;
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
</style>
