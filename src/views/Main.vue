<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick, type Ref } from "vue";
import { NLayout, NLayoutSider } from "naive-ui";

defineOptions({ name: "Main" });
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
  WORKBENCH_ENV_BAR_COLLAPSED_KEY,
  WORKBENCH_NODE_ALLOC_REFRESH_MS,
  WORKBENCH_NODE_TERMINAL_RESOURCE_KINDS,
  WORKBENCH_SHELL_WORKLOAD_KINDS,
  WORKBENCH_SORTABLE_KEYS,
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
  useWorkbenchDrillNavigation,
} from "../features/workbench";
import ResourceDetailDrawer from "../components/ResourceDetailDrawer.vue";
import ChangeImageModal from "../components/ChangeImageModal.vue";
import DeleteConfirmModal from "../components/DeleteConfirmModal.vue";
import WorkbenchActionMenu from "../components/workbench/WorkbenchActionMenu.vue";
import SyncOrchestratorDialog from "../components/workbench/SyncOrchestratorDialog.vue";
import PodDebugDialog from "../components/workbench/PodDebugDialog.vue";
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
  collectAssociatedRefs,
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

const { tableColumns, tableRows } = useWorkbenchTableModel({
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
const syncOrchestratorDialogVisible = ref(false);
const syncOrchestratorRelatedRefs = ref<SyncRelatedRef[]>([]);
const syncOrchestratorLoadingRelated = ref(false);
const syncOrchestratorRelatedError = ref<string | null>(null);
const syncOrchestratorInitialResourceName = ref("");

const orchestratorComponentsForCurrentEnv = computed(() => {
  const envId = currentId.value;
  if (!envId) return [];
  const names = new Set<string>();
  for (const m of manifests.value) {
    if (m.env_id === envId) names.add(m.component);
  }
  return Array.from(names).sort((a, b) => a.localeCompare(b));
});
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

const {
  isCellDrillable,
  onRoleRefClick,
  getSubjectsList,
  getSubjectLabelForTable,
  onSubjectClickForTable,
  onPvcCellClick,
  onReplicasClick,
} = useWorkbenchDrillNavigation({
  navigateTo,
  selectedKind,
  selectedNamespace,
  resourceKinds: RESOURCE_KINDS,
});

/** 操作菜单：点击行时显示，区分资源管理与资源钻取 */
const actionMenuVisible = ref(false);
const actionMenuPosition = ref({ x: 0, y: 0 });
const deleteActionArmed = ref(false);

/** 当前被 ActionMenu 锁定的行 key，用于表格行高亮（与 getRowKey 规则保持一致）。 */
const activeRowKey = computed<string | null>(() => {
  if (!actionMenuVisible.value) return null;
  const r = selectedResource.value;
  if (!r) return null;
  return r.namespace ? `${r.namespace}/${r.name}` : r.name;
});

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
  actionMenuVisible.value = false;
  actionMenuPosition.value = {
    x: e.clientX + WORKBENCH_ACTION_MENU_OFFSET,
    y: e.clientY + WORKBENCH_ACTION_MENU_OFFSET,
  };
  nextTick(() => {
    actionMenuVisible.value = true;
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
  try {
    const containers = await kubeGetPodContainers(envId, resource.namespace ?? "default", resource.name);
    podDebugContainerOptions.value = containers;
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
}

function onPodDebugConfirm(
  container: string,
  _processMode: "main" | "pid",
  pid: number | null,
  namespaces: PodDebugNamespace[],
) {
  const resource = selectedResource.value;
  const env = currentEnv.value;
  const envId = currentId.value;
  if (!resource || resource.kind !== "Pod" || !env || !envId || !resource.nodeName?.trim()) return;
  const strategy = getNodeTerminalStrategy(envId);
  const target = buildNodeTerminalLaunch(strategy, resource.nodeName.trim(), {
    namespace: resource.namespace ?? "default",
    podName: resource.name,
    container,
    namespaces,
    pid,
  });
  if (!target) return;
  pendingOpen.value = {
    kind: "host",
    envId,
    envName: env.display_name,
    hostLabel: `${env.display_name} / ${resource.name} / ${container}`,
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

function relatedRefKey(r: SyncRelatedRef): string {
  return `${r.kind}|${r.namespace ?? ""}|${r.name}`;
}

async function loadSyncRelatedRefs(envId: string, resource: { kind: string; name: string; namespace: string | null }) {
  syncOrchestratorLoadingRelated.value = true;
  syncOrchestratorRelatedError.value = null;
  syncOrchestratorRelatedRefs.value = [];
  try {
    const refs = await collectAssociatedRefs(envId, resource.kind, resource.name, resource.namespace);
    syncOrchestratorRelatedRefs.value = refs;
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
  syncOrchestratorInitialResourceName.value = r.name;
  syncOrchestratorDialogVisible.value = true;
  closeActionMenu();
  await loadSyncRelatedRefs(envId, r);
}

function closeSyncToOrchestratorDialog() {
  syncOrchestratorDialogVisible.value = false;
  syncOrchestratorLoadingRelated.value = false;
  syncOrchestratorRelatedError.value = null;
  syncOrchestratorRelatedRefs.value = [];
}

async function onSyncDialogConfirm(mode: "existing" | "new", componentName: string, selectedRefKeys: string[]) {
  const envId = currentId.value;
  const envName = currentEnv.value?.display_name;
  const r = selectedResource.value;
  if (!envId || !envName || !r) return;
  if (!componentName) {
    listError.value = mode === "existing" ? "请选择已有应用组件。" : "请输入新应用组件名称。";
    return;
  }
  try {
    const yaml = await kubeGetResource(envId, r.kind, r.name, r.namespace);
    const primaryManifest = upsertFromWorkbenchSync(envId, envName, r, yaml, componentName);
    const selectedKeySet = new Set(selectedRefKeys);
    const relatedRefs = syncOrchestratorRelatedRefs.value.filter((ref) => selectedKeySet.has(relatedRefKey(ref)));
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

/** 与 WorkbenchResourceTable 中 Naive DataTable 受控勾选同步。 */
function replaceSelectedRowKeys(keys: string[]) {
  selectedRowKeys.value = new Set(keys);
}

/** 远程排序：DataTable 表头触发，由 useWorkbenchTableModel 对行重排。 */
function setWorkbenchSort(key: string, order: "asc" | "desc") {
  if (!WORKBENCH_SORTABLE_KEYS.has(key)) return;
  sortBy.value = key;
  sortOrder.value = order;
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
  unlistenConnection = await setupConnectionProgressListener();
});

onUnmounted(() => {
  document.removeEventListener("click", onDocClick);
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
  <NLayout :has-sider="openedEnvs.length > 0" class="main-layout">
    <NLayoutSider
      v-if="openedEnvs.length"
      bordered
      :width="236"
      :collapsed-width="44"
      show-trigger="arrow-circle"
      collapse-mode="transform"
      :collapsed="envBarCollapsed"
      content-style="height: 100%; overflow: hidden;"
      @update:collapsed="setEnvBarCollapsed"
    >
      <EnvBar
        :collapsed="envBarCollapsed"
        :on-reconnect="handleReconnect"
        :on-open-terminal="openEnvironmentTerminal"
      />
    </NLayoutSider>
    <div v-if="openedEnvs.length" class="content">
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
            :workbench-kind-label="workbenchKindLabel"
            :effective-namespace="effectiveNamespace"
            :table-rows="tableRows"
            :table-columns="tableColumns"
            :batch-delete-mode="batchDeleteMode"
            :selected-row-keys="selectedRowKeys"
            :sort-by="sortBy"
            :sort-order="sortOrder"
            :selected-kind="selectedKind"
            :list-loading="listLoading"
            :active-row-key="activeRowKey"
            :delete-action-armed="deleteActionArmed"
            :ns-selection-disabled="nsSelectionDisabled"
            :selected-namespace="selectedNamespace"
            :get-row-key="getRowKey"
            :replace-selected-row-keys="replaceSelectedRowKeys"
            :set-workbench-sort="setWorkbenchSort"
            :on-row-click="onRowClick"
            :on-row-context-menu="onRowContextMenu"
            :on-namespace-row-dbl-click="onNamespaceRowClick"
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
    <div v-else class="empty-state">
      <div class="empty-emoji" aria-hidden="true">🌐</div>
      <p class="empty-title">暂无打开的环境</p>
      <p class="empty-desc">请先在「环境管理」中打开至少一个环境。</p>
    </div>

    <!-- overlay 组件全部基于 Teleport 渲染到 body，不会影响 NLayout 的 flex 布局 -->
    <WorkbenchActionMenu
      :visible="actionMenuVisible"
      :position="actionMenuPosition"
      :selected-resource="selectedResource"
      :can-open-node-terminal="canOpenNodeTerminal"
      :can-open-pod-debug="canOpenPodDebug"
      :node-terminal-menu-label="nodeTerminalMenuLabel"
      :node-terminal-disabled-reason="nodeTerminalDisabledReason"
      :pod-debug-disabled-reason="podDebugDisabledReason"
      :delete-action-armed="deleteActionArmed"
      @close="closeActionMenu"
      @open-detail="openResourceDetail"
      @open-topology="openTopology"
      @open-pod-logs="openPodLogs"
      @open-pod-shell="openPodShell"
      @open-node-terminal="openNodeTerminalFromMenu"
      @open-pod-debug="openPodDebugModal"
      @open-edit-config="openEditConfig"
      @open-change-image="openChangeImageModal"
      @open-sync-orchestrator="openSyncToOrchestratorDialog"
      @handle-delete="handleDeleteAction"
    />

    <SyncOrchestratorDialog
      :visible="syncOrchestratorDialogVisible"
      :components="orchestratorComponentsForCurrentEnv"
      :related-refs="syncOrchestratorRelatedRefs"
      :loading-related="syncOrchestratorLoadingRelated"
      :related-error="syncOrchestratorRelatedError"
      :initial-resource-name="syncOrchestratorInitialResourceName"
      @close="closeSyncToOrchestratorDialog"
      @sync="onSyncDialogConfirm"
    />

    <PodDebugDialog
      :visible="podDebugModalVisible"
      :loading="podDebugLoading"
      :error="podDebugError"
      :container-options="podDebugContainerOptions"
      @close="closePodDebugModal"
      @confirm="onPodDebugConfirm"
    />

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
  </NLayout>
</template>


<style src="./main-view-modals.css"></style>
<style src="./main-view.css" scoped></style>
