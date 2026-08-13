<script setup lang="ts">
import { ref, computed, nextTick, onMounted, onUnmounted, watch } from "vue";
import type { MenuOption } from "naive-ui";
import {
  NAlert,
  NButton,
  NCard,
  NDynamicTags,
  NInput,
  NInputNumber,
  NMenu,
  NPopconfirm,
  NSelect,
  NSpace,
  NTabPane,
  NTabs,
  NTag,
  useDialog,
} from "naive-ui";
import { kfSpace } from "../kf";
import { useSaveable } from "../features/settings/useSaveable";
import { CodeEditor } from "monaco-editor-vue3";
import {
  logGetLevel,
  logSetLevel,
  logGetDisplaySettings,
  logSetDisplaySettings,
  LOG_LEVELS,
  LOG_DISPLAY_ORDERS,
  LOG_DISPLAY_FORMATS,
  type LogLevel,
  type LogDisplayOrder,
  type LogDisplayFormat,
} from "../api/log";
import { useLogStore } from "../stores/log";
import {
  EDITOR_DARK_THEME_OPTIONS,
  EDITOR_LIGHT_THEME_OPTIONS,
  useYamlMonacoTheme,
  useYamlTheme,
} from "../stores/yamlTheme";
import {
  appSettingsGetAutoSnapshotEnabled,
  appSettingsGetAutoSnapshotLimitPerResource,
  appSettingsGetBuiltinGpuResourceNames,
  appSettingsGetCustomGpuResourceRules,
  appSettingsGetLogActiveStreamLimit,
  appSettingsGetNodeResourceUsageEnabled,
  appSettingsGetResourceDeployStrategy,
  appSettingsGetTerminalInstanceCacheLimit,
  appSettingsGetSshTunnelMode,
  appSettingsGetWhitespaceRenderEnabled,
  appSettingsSetAutoSnapshotEnabled,
  appSettingsSetAutoSnapshotLimitPerResource,
  appSettingsSetCustomGpuResourceRules,
  appSettingsSetLogActiveStreamLimit,
  appSettingsSetNodeResourceUsageEnabled,
  appSettingsSetResourceDeployStrategy,
  appSettingsSetTerminalInstanceCacheLimit,
  appSettingsSetSshTunnelMode,
  appSettingsSetWhitespaceRenderEnabled,
  sshConfigDefaultPath,
  sshConfigDeleteEntry,
  sshConfigListEntries,
  sshConfigUpsertEntry,
  type GpuResourceRule,
  type ResourceDeployStrategy,
  type SshConfigEntry,
  type TunnelMappingMode,
} from "../api/config";
import { useAppSettingsStore } from "../stores/appSettings";
import { useEnvStore } from "../stores/env";
import { useSnapshotCenterStore } from "../stores/snapshotCenter";
import { appChromeScheme, setAppChromeScheme, APP_CHROME_OPTIONS } from "../stores/appChromeTheme";
import SettingsSecurityPanel from "../components/settings/SettingsSecurityPanel.vue";
import SettingsDataPanel from "../components/settings/SettingsDataPanel.vue";
import SettingsMcpPanel from "../components/settings/SettingsMcpPanel.vue";

type CategoryId = "appearance" | "workspace" | "debug" | "ssh" | "security" | "data" | "mcp";

const CATEGORIES: { id: CategoryId; label: string; icon: string }[] = [
  { id: "appearance", label: "外观", icon: "🎨" },
  { id: "workspace", label: "工作流", icon: "🧭" },
  { id: "data", label: "数据", icon: "💾" },
  { id: "debug", label: "调试", icon: "🔧" },
  { id: "ssh", label: "SSH 隧道", icon: "📡" },
  { id: "security", label: "安全与凭证", icon: "🔒" },
  { id: "mcp", label: "MCP", icon: "🤖" },
];

const { themeIdLight, themeIdDark, activeYamlThemeId } = useYamlTheme();
const { monacoTheme } = useYamlMonacoTheme();
const { triggerLogRefresh } = useLogStore();
const { autoSnapshotEnabled, autoSnapshotLimitPerResource, terminalInstanceCacheLimit, logActiveStreamLimit, nodeResourceUsageEnabled, whitespaceRenderEnabled } = useAppSettingsStore();
const { loadEnvironments } = useEnvStore();
const { requestSwitchToSnapshotCenter } = useSnapshotCenterStore();
function initialCategory(): CategoryId {
  const value = window.sessionStorage.getItem("kube-flow:settings-category");
  return CATEGORIES.some((item) => item.id === value) ? (value as CategoryId) : "appearance";
}

const activeCategory = ref<CategoryId>(initialCategory());
const currentLevel = ref<string>("off");
const currentOrder = ref<LogDisplayOrder>("asc");
const currentFormat = ref<LogDisplayFormat>("json");
const currentLogTailLines = ref(100);
const currentSshTunnelMode = ref<TunnelMappingMode>("ssh");
const currentAutoSnapshotEnabled = ref(true);
const currentAutoSnapshotLimitPerResource = ref(10);
const currentResourceDeployStrategy = ref<ResourceDeployStrategy>("create_replace");
const currentTerminalInstanceCacheLimit = ref(6);
const currentLogActiveStreamLimit = ref(3);
const currentNodeResourceUsageEnabled = ref(false);
const currentWhitespaceRenderEnabled = ref(true);
const builtinGpuResourceNames = ref<string[]>([]);
const customGpuResourceRules = ref<GpuResourceRule[]>([]);
const dialog = useDialog();
type SshSettingsTab = "tunnel" | "hosts";
function initialSshTab(): SshSettingsTab {
  const value = window.sessionStorage.getItem("kube-flow:settings-ssh-tab");
  return value === "hosts" || value === "tunnel" ? value : "tunnel";
}
const sshActiveTab = ref<SshSettingsTab>(initialSshTab());
const sshConfigPath = ref("");
const sshConfigEntries = ref<SshConfigEntry[]>([]);
const selectedSshHost = ref("");
const sshHostFilter = ref("");
const sshConfigLoading = ref(false);
const sshConfigSaving = ref(false);
const sshConfigError = ref("");
const sshConfigMessage = ref("");
const sshForm = ref<SshConfigEntry>(emptySshConfigEntry());
/** 规范化表单快照，用于未保存变更检测。 */
const sshFormBaseline = ref("");
/** 新建前记住的 Host，取消新建时用于回退。 */
const sshCreateReturnHost = ref("");
const sshAdvancedOpen = ref(false);
const sshHostListBodyRef = ref<HTMLElement | null>(null);
const sshHostPanelRef = ref<HTMLElement | null>(null);
/** 左侧 Host 列表高度：按窗口可用空间折算为可完整展示的条目数。 */
const sshHostListHeight = ref(360);
const SSH_HOST_LIST_CHROME_PX = 96;
const SSH_HOST_ITEM_ESTIMATE_PX = 58;
const SSH_HOST_LIST_MIN_VISIBLE = 4;
let sshHostListResizeObserver: ResizeObserver | null = null;
let sshFeedbackClearTimer: ReturnType<typeof setTimeout> | null = null;
/** 合并 Host 配置的校验提示、后端错误与保存结果，仅占位一条避免出现叠放与布局抖动。 */
const sshHostFeedback = computed(() => {
  const err = sshConfigError.value.trim();
  if (err) return { type: "error" as const, text: err };
  const issues = sshForm.value.issues ?? [];
  if (issues.length) return { type: "warning" as const, text: issues.join("；") };
  const msg = sshConfigMessage.value.trim();
  if (msg) return { type: "success" as const, text: msg };
  return null;
});
/** 按 Host 别名、别名列表、HostName、User 做不区分大小写的子串筛选。 */
const filteredSshConfigEntries = computed(() => {
  const q = sshHostFilter.value.trim().toLowerCase();
  if (!q) return sshConfigEntries.value;
  return sshConfigEntries.value.filter((entry) => {
    const haystacks = [
      entry.host,
      ...(entry.aliases ?? []),
      entry.hostname ?? "",
      entry.user ?? "",
      formatSshEntryLabel(entry),
    ];
    return haystacks.some((text) => text.toLowerCase().includes(q));
  });
});
const sshHostListCountLabel = computed(() => {
  const total = sshConfigEntries.value.length;
  const q = sshHostFilter.value.trim();
  if (!q) return `Host · ${total}`;
  return `Host · ${filteredSshConfigEntries.value.length}/${total}`;
});
/** 右侧表单模式：新建 / 编辑 / 只读。 */
const sshFormMode = computed(() => {
  if (!selectedSshHost.value) return "create" as const;
  if (!sshForm.value.editable) return "readonly" as const;
  return "edit" as const;
});
const sshFormModeTitle = computed(() => {
  if (sshFormMode.value === "create") return "新建 Host";
  if (sshFormMode.value === "readonly") return `只读 · ${selectedSshHost.value}`;
  return `编辑 · ${selectedSshHost.value}`;
});
const sshReadonlyReason = computed(() => {
  if (sshFormMode.value !== "readonly") return "";
  const issue = sshForm.value.issues?.[0]?.trim();
  return issue || "含通配符，暂不支持可视化编辑";
});
/** 编辑态锁定主 Host，避免 upsert 按新名追加导致重复块。 */
const sshHostAliasLocked = computed(
  () => sshFormMode.value === "edit" || sshFormMode.value === "readonly"
);
const sshFormFieldsDisabled = computed(
  () => sshConfigSaving.value || !sshForm.value.editable
);
const sshFormDirty = computed(() => {
  if (sshFormMode.value === "readonly") return false;
  return snapshotSshForm(sshForm.value) !== sshFormBaseline.value;
});
const sshSaveButtonLabel = computed(() =>
  sshFormDirty.value && sshFormMode.value === "edit" ? "保存修改" : "保存 Host"
);
const { saving, message, runSave } = useSaveable();
const yamlThemePreview = `apiVersion: apps/v1
kind: Deployment
metadata:
  name: web-app
  namespace: default
  labels:
    app: web-app
spec:
  replicas: 2
  selector:
    matchLabels:
      app: web-app
  template:
    metadata:
      labels:
        app: web-app
    spec:
      containers:
        - name: nginx
          image: nginx:1.27
          ports:
            - containerPort: 80
          env:
            - name: LOG_LEVEL
              value: info`;
const previewOptions = {
  readOnly: true,
  minimap: { enabled: false },
  automaticLayout: true,
  wordWrap: "on",
  lineNumbers: "on",
  scrollBeyondLastLine: false,
  fontSize: 13,
};

function emptySshConfigEntry(): SshConfigEntry {
  return {
    host: "",
    aliases: [],
    hostname: "",
    user: "",
    port: 22,
    identity_file: "",
    proxy_jump: "",
    proxy_command: "",
    options: [],
    source_file: sshConfigPath.value,
    editable: true,
    issues: [],
  };
}

function cloneSshEntry(entry: SshConfigEntry): SshConfigEntry {
  return {
    ...entry,
    hostname: entry.hostname ?? "",
    user: entry.user ?? "",
    identity_file: entry.identity_file ?? "",
    proxy_jump: entry.proxy_jump ?? "",
    proxy_command: entry.proxy_command ?? "",
    aliases: [...(entry.aliases ?? [])],
    options: (entry.options ?? []).map((item) => ({ ...item })),
  };
}

function formatSshEntryLabel(entry: SshConfigEntry): string {
  const target = entry.hostname || entry.host;
  const user = entry.user ? `${entry.user}@` : "";
  const port = entry.port ? `:${entry.port}` : "";
  return `${user}${target}${port}`;
}

/** 列表副标题：仅展示相对 Host 别名的差异信息，避免与主行重复。 */
function formatSshEntrySubtitle(entry: SshConfigEntry): string {
  const hostname = `${entry.hostname ?? ""}`.trim();
  const user = `${entry.user ?? ""}`.trim();
  const port = entry.port && entry.port !== 22 ? entry.port : null;
  const host = entry.host;
  let target = "";
  if (hostname && hostname !== host) {
    target = user ? `${user}@${hostname}` : hostname;
  } else if (user) {
    target = user;
  }
  if (port) {
    target = target ? `${target}:${port}` : `port ${port}`;
  }
  return target;
}

function sshEntryHasProxy(entry: SshConfigEntry): boolean {
  return !!(
    `${entry.proxy_jump ?? ""}`.trim() || `${entry.proxy_command ?? ""}`.trim()
  );
}

function normalizeSshForm(entry: SshConfigEntry): SshConfigEntry {
  const cleanText = (value?: string | null) => {
    const trimmed = `${value ?? ""}`.trim();
    return trimmed ? trimmed : null;
  };
  const options = (entry.options ?? [])
    .map((item) => ({ key: item.key.trim(), value: item.value.trim() }))
    .filter((item) => item.key && item.value);
  return {
    ...entry,
    host: entry.host.trim(),
    aliases: (entry.aliases ?? []).map((item) => item.trim()).filter(Boolean),
    hostname: cleanText(entry.hostname),
    user: cleanText(entry.user),
    port: entry.port || null,
    identity_file: cleanText(entry.identity_file),
    proxy_jump: cleanText(entry.proxy_jump),
    proxy_command: cleanText(entry.proxy_command),
    options,
    source_file: sshConfigPath.value,
    editable: entry.editable,
    issues: [...(entry.issues ?? [])],
  };
}

function snapshotSshForm(entry: SshConfigEntry): string {
  const normalized = normalizeSshForm(entry);
  return JSON.stringify({
    host: normalized.host,
    aliases: normalized.aliases,
    hostname: normalized.hostname,
    user: normalized.user,
    port: normalized.port,
    identity_file: normalized.identity_file,
    proxy_jump: normalized.proxy_jump,
    proxy_command: normalized.proxy_command,
    options: normalized.options,
  });
}

function captureSshFormBaseline() {
  sshFormBaseline.value = snapshotSshForm(sshForm.value);
}

function clearSshFeedbackTimer() {
  if (sshFeedbackClearTimer) {
    clearTimeout(sshFeedbackClearTimer);
    sshFeedbackClearTimer = null;
  }
}

function showSshSuccess(text: string, autoClearMs = 3000) {
  clearSshFeedbackTimer();
  sshConfigError.value = "";
  sshConfigMessage.value = text;
  if (autoClearMs > 0) {
    sshFeedbackClearTimer = setTimeout(() => {
      sshConfigMessage.value = "";
      sshFeedbackClearTimer = null;
    }, autoClearMs);
  }
}

function showSshError(text: string) {
  clearSshFeedbackTimer();
  sshConfigMessage.value = "";
  sshConfigError.value = text;
}

function clearSshFeedback() {
  clearSshFeedbackTimer();
  sshConfigError.value = "";
  sshConfigMessage.value = "";
}

function confirmDiscardSshChanges(): Promise<boolean> {
  if (!sshFormDirty.value) return Promise.resolve(true);
  return new Promise((resolve) => {
    dialog.warning({
      title: "未保存的修改",
      content: "当前 Host 配置尚未保存，离开后将丢失。确定继续吗？",
      positiveText: "放弃修改",
      negativeText: "继续编辑",
      onPositiveClick: () => {
        resolve(true);
        return true;
      },
      onNegativeClick: () => {
        resolve(false);
        return true;
      },
      onClose: () => resolve(false),
    });
  });
}

function selectSshEntry(entry: SshConfigEntry, opts?: { preserveFeedback?: boolean }) {
  selectedSshHost.value = entry.host;
  sshForm.value = cloneSshEntry(entry);
  sshAdvancedOpen.value = (entry.options?.length ?? 0) > 0;
  captureSshFormBaseline();
  if (!opts?.preserveFeedback) {
    clearSshFeedback();
  }
}

function startNewSshEntry(opts?: { preserveFeedback?: boolean }) {
  selectedSshHost.value = "";
  sshForm.value = emptySshConfigEntry();
  sshAdvancedOpen.value = false;
  captureSshFormBaseline();
  if (!opts?.preserveFeedback) {
    clearSshFeedback();
  }
}

async function requestSelectSshEntry(entry: SshConfigEntry) {
  if (sshFormMode.value !== "create" && entry.host === selectedSshHost.value) return;
  if (!(await confirmDiscardSshChanges())) return;
  selectSshEntry(entry);
}

async function requestStartNewSshEntry() {
  if (sshFormMode.value === "create" && !sshFormDirty.value) return;
  if (!(await confirmDiscardSshChanges())) return;
  sshCreateReturnHost.value = selectedSshHost.value;
  startNewSshEntry();
}

async function cancelNewSshEntry() {
  if (!(await confirmDiscardSshChanges())) return;
  const returnHost = sshCreateReturnHost.value;
  const entry = returnHost
    ? sshConfigEntries.value.find((item) => item.host === returnHost)
    : sshConfigEntries.value[0];
  if (entry) selectSshEntry(entry);
  else startNewSshEntry();
}

/** 保存/删除后静默刷新列表，避免整表重载导致表单与提示条闪烁。 */
function syncSshEntryMetadata(entry: SshConfigEntry) {
  sshForm.value.issues = [...(entry.issues ?? [])];
  sshForm.value.editable = entry.editable;
  sshForm.value.source_file = entry.source_file;
  sshForm.value.aliases = [...(entry.aliases ?? [])];
  captureSshFormBaseline();
}

async function scrollSelectedSshHostIntoView() {
  await nextTick();
  const host = selectedSshHost.value;
  if (!host || !sshHostListBodyRef.value) return;
  const el = sshHostListBodyRef.value.querySelector(
    `[data-ssh-host="${CSS.escape(host)}"]`
  );
  el?.scrollIntoView({ block: "nearest" });
}

/** 按设置内容区可用高度折算左侧列表，底部预留边距，避免整页滚动。 */
function updateSshHostListHeight() {
  if (activeCategory.value !== "ssh" || sshActiveTab.value !== "hosts") return;
  const panel = sshHostPanelRef.value;
  if (!panel) return;
  const rect = panel.getBoundingClientRect();
  const contentEl = panel.closest(".settings-content") as HTMLElement | null;
  const bottomMargin = 20;
  const bottomEdge = contentEl
    ? contentEl.getBoundingClientRect().bottom - bottomMargin
    : window.innerHeight - bottomMargin;
  const descEl = panel.querySelector(".ssh-host-panel-desc") as HTMLElement | null;
  const descHeight = descEl ? descEl.getBoundingClientRect().height + 12 : 48;
  const available = Math.floor(bottomEdge - rect.top - descHeight);
  if (available <= 0) return;
  const bodyAvail = Math.max(
    SSH_HOST_LIST_MIN_VISIBLE * SSH_HOST_ITEM_ESTIMATE_PX,
    available - SSH_HOST_LIST_CHROME_PX
  );
  const visibleCount = Math.max(
    SSH_HOST_LIST_MIN_VISIBLE,
    Math.floor(bodyAvail / SSH_HOST_ITEM_ESTIMATE_PX)
  );
  let nextHeight = SSH_HOST_LIST_CHROME_PX + visibleCount * SSH_HOST_ITEM_ESTIMATE_PX;
  // 窄屏列表与表单纵向堆叠，限制列表约占视口四成，避免挤掉表单。
  if (window.innerWidth <= 900) {
    nextHeight = Math.min(nextHeight, Math.floor(window.innerHeight * 0.4));
  }
  // 不超过可用高度，短窗口也至少能看到几条。
  sshHostListHeight.value = Math.max(160, Math.min(available, nextHeight));
}

function bindSshHostListResize() {
  unbindSshHostListResize();
  updateSshHostListHeight();
  window.addEventListener("resize", updateSshHostListHeight);
  if (typeof ResizeObserver !== "undefined" && sshHostPanelRef.value) {
    sshHostListResizeObserver = new ResizeObserver(() => updateSshHostListHeight());
    sshHostListResizeObserver.observe(sshHostPanelRef.value);
  }
}

function unbindSshHostListResize() {
  window.removeEventListener("resize", updateSshHostListHeight);
  sshHostListResizeObserver?.disconnect();
  sshHostListResizeObserver = null;
}

async function onSshTabUpdate(tab: string) {
  const next = tab === "hosts" ? "hosts" : "tunnel";
  if (next === sshActiveTab.value) return;
  if (sshActiveTab.value === "hosts" && next !== "hosts") {
    if (!(await confirmDiscardSshChanges())) return;
  }
  sshActiveTab.value = next;
  window.sessionStorage.setItem("kube-flow:settings-ssh-tab", next);
  if (next === "hosts") {
    await nextTick();
    bindSshHostListResize();
    await scrollSelectedSshHostIntoView();
  } else {
    unbindSshHostListResize();
  }
}

async function fetchSshConfigEntries() {
  const [path, entries] = await Promise.all([sshConfigDefaultPath(), sshConfigListEntries()]);
  sshConfigPath.value = path ?? "~/.ssh/config";
  sshConfigEntries.value = entries;
  return entries;
}

async function refreshSshEntriesAfterMutation(host: string, opts?: { preserveFeedback?: boolean }) {
  const entries = await fetchSshConfigEntries();
  if (!host) {
    if (entries.length) selectSshEntry(entries[0], opts);
    else startNewSshEntry(opts);
    await scrollSelectedSshHostIntoView();
    return;
  }
  const saved = entries.find((entry) => entry.host === host);
  if (saved && selectedSshHost.value === host) {
    syncSshEntryMetadata(saved);
    await scrollSelectedSshHostIntoView();
    return;
  }
  if (saved) {
    selectSshEntry(saved, opts);
    await scrollSelectedSshHostIntoView();
    return;
  }
  if (entries.length) selectSshEntry(entries[0], opts);
  else startNewSshEntry(opts);
  await scrollSelectedSshHostIntoView();
}

function addSshOption() {
  sshForm.value.options = [...(sshForm.value.options ?? []), { key: "", value: "" }];
  sshAdvancedOpen.value = true;
}

function removeSshOption(index: number) {
  sshForm.value.options = (sshForm.value.options ?? []).filter((_, idx) => idx !== index);
}

async function loadSshConfigEntries(opts?: { preserveFeedback?: boolean }) {
  sshConfigLoading.value = true;
  if (!opts?.preserveFeedback) {
    clearSshFeedback();
  }
  try {
    const entries = await fetchSshConfigEntries();
    const current = selectedSshHost.value
      ? entries.find((entry) => entry.host === selectedSshHost.value)
      : entries[0];
    const preserveFeedback = !!opts?.preserveFeedback;
    if (current) selectSshEntry(current, { preserveFeedback });
    else startNewSshEntry({ preserveFeedback });
    await scrollSelectedSshHostIntoView();
  } catch (e) {
    showSshError(e instanceof Error ? e.message : String(e));
    sshConfigEntries.value = [];
  } finally {
    sshConfigLoading.value = false;
  }
}

async function requestReloadSshConfig() {
  if (!(await confirmDiscardSshChanges())) return;
  await loadSshConfigEntries();
}

async function saveSshConfigEntry() {
  const payload = normalizeSshForm(sshForm.value);
  if (!payload.host) {
    showSshError("请输入 Host 别名");
    return;
  }
  if (payload.proxy_jump && payload.proxy_command) {
    showSshError("ProxyJump 与 ProxyCommand 只能配置一个");
    return;
  }
  sshConfigSaving.value = true;
  try {
    await sshConfigUpsertEntry(payload);
    selectedSshHost.value = payload.host;
    showSshSuccess("SSH 配置已保存");
    await refreshSshEntriesAfterMutation(payload.host, { preserveFeedback: true });
  } catch (e) {
    showSshError(e instanceof Error ? e.message : String(e));
  } finally {
    sshConfigSaving.value = false;
  }
}

async function deleteSshConfigEntry() {
  const host = sshForm.value.host;
  if (!host) return;
  sshConfigSaving.value = true;
  try {
    await sshConfigDeleteEntry(host);
    selectedSshHost.value = "";
    showSshSuccess("SSH Host 已删除");
    await refreshSshEntriesAfterMutation("", { preserveFeedback: true });
  } catch (e) {
    showSshError(e instanceof Error ? e.message : String(e));
  } finally {
    sshConfigSaving.value = false;
  }
}

async function load() {
  try {
    const [level, settings, sshMode, autoSnapshot, autoSnapshotLimit, resourceDeployStrategy, terminalCacheLimit, activeLogLimit, nodeUsageEnabled, whitespaceEnabled, builtinGpuNames, customGpuRules] = await Promise.all([
      logGetLevel(),
      logGetDisplaySettings(),
      appSettingsGetSshTunnelMode(),
      appSettingsGetAutoSnapshotEnabled(),
      appSettingsGetAutoSnapshotLimitPerResource(),
      appSettingsGetResourceDeployStrategy(),
      appSettingsGetTerminalInstanceCacheLimit(),
      appSettingsGetLogActiveStreamLimit(),
      appSettingsGetNodeResourceUsageEnabled(),
      appSettingsGetWhitespaceRenderEnabled(),
      appSettingsGetBuiltinGpuResourceNames(),
      appSettingsGetCustomGpuResourceRules(),
    ]);
    currentLevel.value = level;
    currentOrder.value = settings.order;
    currentFormat.value = settings.format;
    currentLogTailLines.value = settings.tailLines;
    currentSshTunnelMode.value = sshMode;
    currentAutoSnapshotEnabled.value = autoSnapshot;
    currentAutoSnapshotLimitPerResource.value = Math.max(0, Math.floor(autoSnapshotLimit || 0));
    currentResourceDeployStrategy.value = resourceDeployStrategy;
    currentTerminalInstanceCacheLimit.value = Math.min(20, Math.max(1, Math.floor(terminalCacheLimit || 6)));
    currentLogActiveStreamLimit.value = Math.min(12, Math.max(1, Math.floor(activeLogLimit || 3)));
    currentNodeResourceUsageEnabled.value = !!nodeUsageEnabled;
    currentWhitespaceRenderEnabled.value = !!whitespaceEnabled;
    builtinGpuResourceNames.value = builtinGpuNames;
    customGpuResourceRules.value = customGpuRules.length ? customGpuRules : [{ display_name: "", resource_name: "" }];
    autoSnapshotEnabled.value = autoSnapshot;
    autoSnapshotLimitPerResource.value = currentAutoSnapshotLimitPerResource.value;
    terminalInstanceCacheLimit.value = currentTerminalInstanceCacheLimit.value;
    logActiveStreamLimit.value = currentLogActiveStreamLimit.value;
    nodeResourceUsageEnabled.value = currentNodeResourceUsageEnabled.value;
    whitespaceRenderEnabled.value = currentWhitespaceRenderEnabled.value;
  } catch {
    currentLevel.value = "off";
    currentResourceDeployStrategy.value = "create_replace";
    currentTerminalInstanceCacheLimit.value = 6;
    currentLogActiveStreamLimit.value = 3;
    currentNodeResourceUsageEnabled.value = false;
    currentWhitespaceRenderEnabled.value = true;
    builtinGpuResourceNames.value = ["*/gpu"];
    customGpuResourceRules.value = [{ display_name: "", resource_name: "" }];
  }
  await loadEnvironments().catch(() => {});
  await loadSshConfigEntries().catch(() => {});
}

async function saveSshTunnelMode(mode: TunnelMappingMode) {
  await runSave(async () => {
    await appSettingsSetSshTunnelMode(mode);
    currentSshTunnelMode.value = mode;
  });
}

async function saveAutoSnapshotEnabled(enabled: boolean) {
  await runSave(async () => {
    await appSettingsSetAutoSnapshotEnabled(enabled);
    currentAutoSnapshotEnabled.value = enabled;
    autoSnapshotEnabled.value = enabled;
  });
}

async function saveAutoSnapshotLimitPerResource(limit: number) {
  await runSave(async () => {
    const normalized = Math.max(0, Math.min(100, Math.floor(Number.isFinite(limit) ? limit : 10)));
    await appSettingsSetAutoSnapshotLimitPerResource(normalized);
    currentAutoSnapshotLimitPerResource.value = normalized;
    autoSnapshotLimitPerResource.value = normalized;
  });
}

async function saveTerminalInstanceCacheLimit(limit: number) {
  const normalized = Math.min(20, Math.max(1, Math.floor(Number.isFinite(limit) ? limit : 6)));
  currentTerminalInstanceCacheLimit.value = normalized;
  await runSave(async () => {
    await appSettingsSetTerminalInstanceCacheLimit(normalized);
    terminalInstanceCacheLimit.value = normalized;
  });
}

async function saveResourceDeployStrategy(strategy: ResourceDeployStrategy) {
  await runSave(async () => {
    await appSettingsSetResourceDeployStrategy(strategy);
    currentResourceDeployStrategy.value = strategy;
  });
}

async function saveNodeResourceUsageEnabled(enabled: boolean) {
  await runSave(async () => {
    await appSettingsSetNodeResourceUsageEnabled(enabled);
    currentNodeResourceUsageEnabled.value = enabled;
    nodeResourceUsageEnabled.value = enabled;
  });
}

async function saveWhitespaceRenderEnabled(enabled: boolean) {
  await runSave(async () => {
    await appSettingsSetWhitespaceRenderEnabled(enabled);
    currentWhitespaceRenderEnabled.value = enabled;
    whitespaceRenderEnabled.value = enabled;
  });
}

function addGpuRuleRow() {
  customGpuResourceRules.value = [...customGpuResourceRules.value, { display_name: "", resource_name: "" }];
}

function removeGpuRuleRow(index: number) {
  customGpuResourceRules.value = customGpuResourceRules.value.filter((_, idx) => idx !== index);
  if (!customGpuResourceRules.value.length) {
    customGpuResourceRules.value = [{ display_name: "", resource_name: "" }];
  }
}

async function saveCustomGpuResourceNames() {
  await runSave(async () => {
    const rules = customGpuResourceRules.value
      .map((item) => ({
        display_name: item.display_name.trim(),
        resource_name: item.resource_name.trim(),
      }))
      .filter((item) => item.resource_name.length > 0);
    await appSettingsSetCustomGpuResourceRules(rules);
    customGpuResourceRules.value = rules.length ? rules : [{ display_name: "", resource_name: "" }];
  });
}

async function saveLogActiveStreamLimit(limit: number) {
  const normalized = Math.min(12, Math.max(1, Math.floor(Number.isFinite(limit) ? limit : 3)));
  currentLogActiveStreamLimit.value = normalized;
  await runSave(async () => {
    await appSettingsSetLogActiveStreamLimit(normalized);
    logActiveStreamLimit.value = normalized;
  });
}

async function saveLevel(level: LogLevel) {
  await runSave(async () => {
    await logSetLevel(level);
    currentLevel.value = level;
    triggerLogRefresh();
  });
}

async function saveDisplaySettings(order: LogDisplayOrder, format: LogDisplayFormat) {
  await runSave(async () => {
    await logSetDisplaySettings(order, format, currentLogTailLines.value);
    currentOrder.value = order;
    currentFormat.value = format;
    triggerLogRefresh();
  });
}

async function saveLogTailLines(lines: number) {
  await runSave(async () => {
    const normalized = Math.max(1, Math.min(5000, Math.floor(Number.isFinite(lines) ? lines : 100)));
    await logSetDisplaySettings(currentOrder.value, currentFormat.value, normalized);
    currentLogTailLines.value = normalized;
    triggerLogRefresh();
  });
}

onMounted(() => {
  load();
  if (activeCategory.value === "ssh" && sshActiveTab.value === "hosts") {
    nextTick(() => bindSshHostListResize());
  }
});

onUnmounted(() => {
  unbindSshHostListResize();
});

watch(activeCategory, async (value) => {
  window.sessionStorage.setItem("kube-flow:settings-category", value);
  if (value === "ssh" && sshActiveTab.value === "hosts") {
    await nextTick();
    bindSshHostListResize();
    return;
  }
  unbindSshHostListResize();
});

const menuOptions = computed<MenuOption[]>(() =>
  CATEGORIES.map((c) => ({
    key: c.id,
    label: `${c.icon} ${c.label}`,
  }))
);


</script>

<template>
  <div class="settings">
    <aside class="settings-nav">
      <NMenu v-model:value="activeCategory" :options="menuOptions" class="settings-menu" />
    </aside>
    <main class="settings-content">
      <header class="page-header">
        <h1 class="page-title">
          {{ CATEGORIES.find((c) => c.id === activeCategory)?.label ?? "设置" }}
        </h1>
      </header>

      <!-- 工作流 -->
      <template v-if="activeCategory === 'workspace'">
        <NCard title="快照工作流" size="small" class="settings-card" :bordered="true">
          <p class="card-desc">
            自动快照会在应用配置、编辑 YAML、修改镜像前自动生成历史快照。关闭后不再自动生成，但仍可在快照中心手动生成。
          </p>
          <NSpace v-bind="kfSpace.buttonGroup">
            <NButton
              :type="currentAutoSnapshotEnabled ? 'primary' : 'default'"
              :secondary="!currentAutoSnapshotEnabled"
              :disabled="saving"
              @click="saveAutoSnapshotEnabled(true)"
            >开启自动快照</NButton>
            <NButton
              :type="!currentAutoSnapshotEnabled ? 'primary' : 'default'"
              :secondary="currentAutoSnapshotEnabled"
              :disabled="saving"
              @click="saveAutoSnapshotEnabled(false)"
            >关闭自动快照</NButton>
          </NSpace>
          <div class="setting-row">
            <div class="setting-copy">
              <div class="setting-title">每个资源自动快照上限</div>
              <div class="setting-desc">默认 10 个。超过上限后，会自动删除最旧的自动快照，手动快照不受影响；设置为 0 表示不自动淘汰。</div>
            </div>
            <NSpace v-bind="kfSpace.settingInline" class="setting-input-wrap">
              <NInputNumber
                v-model:value="currentAutoSnapshotLimitPerResource"
                :min="0"
                :max="100"
                :disabled="saving"
                :show-button="false"
                class="num-compact"
                @blur="saveAutoSnapshotLimitPerResource(currentAutoSnapshotLimitPerResource)"
              />
              <NButton :disabled="saving" @click="saveAutoSnapshotLimitPerResource(currentAutoSnapshotLimitPerResource)">保存上限</NButton>
            </NSpace>
          </div>
          <NSpace v-bind="kfSpace.settingActions" class="setting-inline-actions">
            <NButton @click="requestSwitchToSnapshotCenter">打开快照中心</NButton>
          </NSpace>
          <NAlert v-if="message" class="msg-alert" :type="message === '已保存' ? 'success' : 'error'" :show-icon="true">{{ message }}</NAlert>
        </NCard>

        <NCard title="终端工作流" size="small" class="settings-card" :bordered="true">
          <p class="card-desc">控制终端中心在前端保留多少个最近活跃的终端实例，用来平衡切换体验和内存占用。</p>
          <div class="setting-row">
            <div class="setting-copy">
              <div class="setting-title">终端实例缓存数量</div>
              <div class="setting-desc">默认 6 个。终端中心会保留最近活跃的终端实例屏幕历史（含断开后的交互缓冲），超出后仅回收前端实例，不会关闭会话本身。</div>
            </div>
            <NSpace v-bind="kfSpace.settingInline" class="setting-input-wrap">
              <NInputNumber
                v-model:value="currentTerminalInstanceCacheLimit"
                :min="1"
                :max="20"
                :disabled="saving"
                :show-button="false"
                class="num-compact"
                @blur="saveTerminalInstanceCacheLimit(currentTerminalInstanceCacheLimit)"
              />
              <NButton :disabled="saving" @click="saveTerminalInstanceCacheLimit(currentTerminalInstanceCacheLimit)">保存数量</NButton>
            </NSpace>
          </div>
          <NAlert v-if="message" class="msg-alert" :type="message === '已保存' ? 'success' : 'error'" :show-icon="true">{{ message }}</NAlert>
        </NCard>

        <NCard title="编排中心下发" size="small" class="settings-card" :bordered="true">
          <p class="card-desc">控制编排中心把 YAML 投放到目标环境时采用的策略。工作台编辑现有资源仍保持原有覆盖逻辑，不受这里影响。</p>
          <NSpace v-bind="kfSpace.buttonGroup">
            <NButton
              :type="currentResourceDeployStrategy === 'create_replace' ? 'primary' : 'default'"
              :secondary="currentResourceDeployStrategy !== 'create_replace'"
              :disabled="saving"
              @click="saveResourceDeployStrategy('create_replace')"
            >Create + Replace</NButton>
            <NButton
              :type="currentResourceDeployStrategy === 'apply' ? 'primary' : 'default'"
              :secondary="currentResourceDeployStrategy !== 'apply'"
              :disabled="saving"
              @click="saveResourceDeployStrategy('apply')"
            >Apply</NButton>
          </NSpace>
          <NAlert title="Create + Replace" type="default" class="hint-alert">
            目标资源不存在时先创建，已存在时按完整 YAML 覆盖，更接近模板投放。
          </NAlert>
          <NAlert title="Apply" type="default" class="hint-alert">
            使用 server-side apply 合并字段，更适合与其他控制器共享对象所有权。
          </NAlert>
          <NAlert v-if="message" class="msg-alert" :type="message === '已保存' ? 'success' : 'error'" :show-icon="true">{{ message }}</NAlert>
        </NCard>

        <NCard title="Node 资源统计" size="small" class="settings-card" :bordered="true">
          <p class="card-desc">控制工作台 Node 列表是否展示资源统计信息。默认关闭，按需开启。</p>
          <NSpace v-bind="kfSpace.buttonGroup">
            <NButton
              :type="currentNodeResourceUsageEnabled ? 'primary' : 'default'"
              :secondary="!currentNodeResourceUsageEnabled"
              :disabled="saving"
              @click="saveNodeResourceUsageEnabled(true)"
            >开启统计</NButton>
            <NButton
              :type="!currentNodeResourceUsageEnabled ? 'primary' : 'default'"
              :secondary="currentNodeResourceUsageEnabled"
              :disabled="saving"
              @click="saveNodeResourceUsageEnabled(false)"
            >关闭统计</NButton>
          </NSpace>
          <NAlert title="关闭时" type="default" class="hint-alert">只展示节点基础状态，不启动额外的资源统计请求。</NAlert>
          <NAlert title="开启时" type="default" class="hint-alert">在 Node 列表中展示资源统计信息，帮助观察节点容量分布。</NAlert>
          <div class="setting-row">
            <div class="setting-copy">
              <div class="setting-title">内置 GPU 识别规则</div>
              <div class="setting-desc">默认支持通用的 GPU 资源模式识别，自定义规则用于补充特殊资源名或自定义显示名称。</div>
            </div>
            <NSpace v-bind="kfSpace.settingStack" class="setting-input-wrap setting-stack-wrap">
              <NSpace wrap size="small">
                <NTag v-for="name in builtinGpuResourceNames" :key="name" size="small" round :bordered="false">{{ name }}</NTag>
              </NSpace>
            </NSpace>
          </div>
          <div class="setting-row">
            <div class="setting-copy">
              <div class="setting-title">自定义 GPU 规则</div>
              <div class="setting-desc">用于补充特殊资源名，或为已识别的资源配置更友好的显示名称。</div>
            </div>
            <NSpace v-bind="kfSpace.settingStackLoose" class="setting-input-wrap setting-stack-wrap">
              <div
                v-for="(rule, index) in customGpuResourceRules"
                :key="index"
                class="gpu-rule-row"
              >
                <NInput
                  v-model:value="rule.display_name"
                  :disabled="saving"
                  placeholder="显示名称，例如 A100"
                />
                <NInput
                  v-model:value="rule.resource_name"
                  :disabled="saving"
                  placeholder="资源名称，例如 vendor.com/gpu"
                />
                <NButton quaternary type="error" :disabled="saving" @click="removeGpuRuleRow(index)">删除</NButton>
              </div>
              <NSpace v-bind="kfSpace.settingActions" class="setting-inline-actions">
                <NButton :disabled="saving" @click="addGpuRuleRow">增加规则</NButton>
                <NButton type="primary" :disabled="saving" @click="saveCustomGpuResourceNames">保存规则</NButton>
              </NSpace>
            </NSpace>
          </div>
          <NAlert v-if="message" class="msg-alert" :type="message === '已保存' ? 'success' : 'error'" :show-icon="true">{{ message }}</NAlert>
        </NCard>
      </template>

      <!-- 调试 -->
      <template v-if="activeCategory === 'debug'">
        <NCard title="应用日志采集" size="small" class="settings-card" :bordered="true">
          <p class="card-desc">日志级别决定写入应用日志的内容量，用于排查资源列表、连接与后端行为问题。</p>
          <NSpace v-bind="kfSpace.buttonGroup">
            <NButton
              v-for="opt in LOG_LEVELS"
              :key="opt.value"
              :type="currentLevel === opt.value ? 'primary' : 'default'"
              :secondary="currentLevel !== opt.value"
              :disabled="saving"
              @click="saveLevel(opt.value)"
            >{{ opt.label }}</NButton>
          </NSpace>
          <p class="card-desc" style="margin-top: 0.5rem">应用日志格式：仅作用于应用日志页，不影响 Pod 或 Workload 日志输出。</p>
          <NSpace v-bind="kfSpace.optionGroup" class="option-group">
            <div class="option-label">格式</div>
            <NSpace v-bind="kfSpace.buttonGroup">
              <NButton
                v-for="opt in LOG_DISPLAY_FORMATS"
                :key="opt.value"
                :type="currentFormat === opt.value ? 'primary' : 'default'"
                :secondary="currentFormat !== opt.value"
                :disabled="saving"
                @click="saveDisplaySettings(currentOrder, opt.value)"
              >{{ opt.label }}</NButton>
            </NSpace>
          </NSpace>
        </NCard>

        <NCard title="全局日志显示" size="small" class="settings-card" :bordered="true">
          <p class="card-desc">这里的顺序会同时作用于日志中心里的资源日志和应用日志。</p>
          <NSpace v-bind="kfSpace.optionGroup" class="option-group">
            <div class="option-label">顺序</div>
            <NSpace v-bind="kfSpace.buttonGroup">
              <NButton
                v-for="opt in LOG_DISPLAY_ORDERS"
                :key="opt.value"
                :type="currentOrder === opt.value ? 'primary' : 'default'"
                :secondary="currentOrder !== opt.value"
                :disabled="saving"
                @click="saveDisplaySettings(opt.value, currentFormat)"
              >{{ opt.label }}</NButton>
            </NSpace>
          </NSpace>
          <div class="setting-row">
            <div class="setting-copy">
              <div class="setting-title">默认展示行数</div>
              <div class="setting-desc">新打开的资源日志默认按这个行数加载，默认 100 行。</div>
            </div>
            <NSpace v-bind="kfSpace.settingInline" class="setting-input-wrap">
              <NInputNumber
                v-model:value="currentLogTailLines"
                :min="1"
                :max="5000"
                :disabled="saving"
                :show-button="false"
                class="num-compact"
                @blur="saveLogTailLines(currentLogTailLines)"
              />
              <NButton :disabled="saving" @click="saveLogTailLines(currentLogTailLines)">保存行数</NButton>
            </NSpace>
          </div>
          <NAlert v-if="message" class="msg-alert" :type="message === '已保存' ? 'success' : 'error'" :show-icon="true">{{ message }}</NAlert>
        </NCard>

        <NCard title="日志中心" size="small" class="settings-card" :bordered="true">
          <p class="card-desc">控制日志中心同时保活的实时 follow 日志流数量。超出上限的会话会保留已加载内容，但暂停实时流。</p>
          <div class="setting-row">
            <div class="setting-copy">
              <div class="setting-title">活跃日志流数量</div>
              <div class="setting-desc">默认 3 个。最近活跃的日志会话会优先保留实时 follow，切换回来时会自动恢复。</div>
            </div>
            <NSpace v-bind="kfSpace.settingInline" class="setting-input-wrap">
              <NInputNumber
                v-model:value="currentLogActiveStreamLimit"
                :min="1"
                :max="12"
                :disabled="saving"
                :show-button="false"
                class="num-compact"
                @blur="saveLogActiveStreamLimit(currentLogActiveStreamLimit)"
              />
              <NButton :disabled="saving" @click="saveLogActiveStreamLimit(currentLogActiveStreamLimit)">保存数量</NButton>
            </NSpace>
          </div>
          <NAlert v-if="message" class="msg-alert" :type="message === '已保存' ? 'success' : 'error'" :show-icon="true">{{ message }}</NAlert>
        </NCard>
      </template>

      <!-- SSH 隧道 -->
      <template v-if="activeCategory === 'ssh'">
        <NTabs
          :value="sshActiveTab"
          type="line"
          animated
          class="ssh-settings-tabs"
          @update:value="onSshTabUpdate"
        >
          <NTabPane name="tunnel" tab="映射方式">
            <NCard title="默认映射方式" size="small" class="settings-card" :bordered="true">
              <p class="card-desc">
                控制新建或未显式配置的 SSH 隧道默认实现：ssh 使用系统 ssh -L 子进程（兼容性最好），builtin 使用 libssh2 内置转发（无子进程）。
              </p>
              <NSpace v-bind="kfSpace.buttonGroup">
                <NButton
                  :type="currentSshTunnelMode === 'ssh' ? 'primary' : 'default'"
                  :secondary="currentSshTunnelMode !== 'ssh'"
                  :disabled="saving"
                  @click="saveSshTunnelMode('ssh')"
                >ssh（子进程）</NButton>
                <NButton
                  :type="currentSshTunnelMode === 'builtin' ? 'primary' : 'default'"
                  :secondary="currentSshTunnelMode !== 'builtin'"
                  :disabled="saving"
                  @click="saveSshTunnelMode('builtin')"
                >builtin（内置）</NButton>
              </NSpace>
              <NAlert v-if="message" class="msg-alert" :type="message === '已保存' ? 'success' : 'error'" :show-icon="true">{{ message }}</NAlert>
            </NCard>
          </NTabPane>

          <NTabPane name="hosts" tab="Host 配置" display-directive="show">
            <div ref="sshHostPanelRef" class="ssh-host-panel">
              <p class="card-desc ssh-host-panel-desc">
                编辑本机 {{ sshConfigPath || "~/.ssh/config" }} 中的 Host，供新建 SSH 隧道环境选择。保存时写入该文件，并在同目录生成 kube-flow 备份。
              </p>
              <div
                class="ssh-config-layout"
                :style="{ height: `${sshHostListHeight}px` }"
              >
                <aside
                  class="ssh-host-list"
                  :style="{ height: `${sshHostListHeight}px` }"
                >
                  <div class="ssh-host-list-header">
                    <span>{{ sshHostListCountLabel }}</span>
                    <NButton
                      size="small"
                      :disabled="sshConfigLoading || sshConfigSaving"
                      @click="requestStartNewSshEntry"
                    >新增</NButton>
                  </div>
                  <div class="ssh-host-list-filter">
                    <NInput
                      v-model:value="sshHostFilter"
                      clearable
                      size="small"
                      placeholder="筛选 Host…"
                      :disabled="sshConfigLoading"
                    />
                  </div>
                  <div ref="sshHostListBodyRef" class="ssh-host-list-body">
                    <button
                      v-for="entry in filteredSshConfigEntries"
                      :key="entry.host"
                      type="button"
                      class="ssh-host-item"
                      :data-ssh-host="entry.host"
                      :class="{
                        active: sshFormMode !== 'create' && selectedSshHost === entry.host,
                        disabled: !entry.editable,
                      }"
                      @click="requestSelectSshEntry(entry)"
                    >
                      <span class="ssh-host-name" :title="entry.host">{{ entry.host }}</span>
                      <span
                        v-if="formatSshEntrySubtitle(entry)"
                        class="ssh-host-target"
                        :title="formatSshEntrySubtitle(entry)"
                      >{{ formatSshEntrySubtitle(entry) }}</span>
                      <div v-if="!entry.editable || sshEntryHasProxy(entry)" class="ssh-host-badges">
                        <NTag v-if="!entry.editable" size="tiny" :bordered="false">只读</NTag>
                        <NTag v-if="sshEntryHasProxy(entry)" size="tiny" :bordered="false" type="info">Proxy</NTag>
                      </div>
                    </button>
                    <div
                      v-if="!filteredSshConfigEntries.length && !sshConfigLoading"
                      class="ssh-host-empty"
                    >
                      <template v-if="sshHostFilter.trim() && sshConfigEntries.length">
                        无匹配结果。
                        <button type="button" class="ssh-host-clear-filter" @click="sshHostFilter = ''">
                          清除筛选
                        </button>
                      </template>
                      <template v-else>
                        还没有 Host，新增一个即可用于 SSH 隧道环境。
                      </template>
                    </div>
                  </div>
                </aside>

                <section class="ssh-config-form" :style="{ maxHeight: `${sshHostListHeight}px` }">
                  <div class="ssh-form-mode-bar">
                    <div class="ssh-form-mode-title">{{ sshFormModeTitle }}</div>
                    <NTag v-if="sshFormDirty" size="tiny" type="warning" :bordered="false">未保存</NTag>
                  </div>
                  <NAlert
                    v-if="sshFormMode === 'readonly'"
                    type="warning"
                    :show-icon="false"
                    size="small"
                    class="ssh-readonly-alert"
                  >
                    {{ sshReadonlyReason }}
                  </NAlert>

                  <div class="ssh-form-grid">
                    <label class="ssh-form-field">
                      <span class="field-label">Host 别名</span>
                      <NInput
                        v-model:value="sshForm.host"
                        :disabled="sshConfigSaving || sshHostAliasLocked"
                        placeholder="prod-bastion"
                      />
                      <span v-if="sshFormMode === 'edit'" class="setting-desc">改名请删除后新建，以免写入重复 Host 块。</span>
                    </label>
                    <label class="ssh-form-field">
                      <span class="field-label">HostName</span>
                      <NInput
                        v-model:value="sshForm.hostname"
                        :disabled="sshFormFieldsDisabled"
                        placeholder="10.0.0.12 或 bastion.example.com"
                      />
                    </label>
                    <label class="ssh-form-field">
                      <span class="field-label">User</span>
                      <NInput v-model:value="sshForm.user" :disabled="sshFormFieldsDisabled" placeholder="root" />
                    </label>
                    <label class="ssh-form-field">
                      <span class="field-label">Port</span>
                      <NInputNumber
                        v-model:value="sshForm.port"
                        :min="1"
                        :max="65535"
                        :show-button="false"
                        :disabled="sshFormFieldsDisabled"
                        placeholder="22"
                      />
                    </label>
                  </div>

                  <label class="ssh-form-field">
                    <span class="field-label">Aliases</span>
                    <NDynamicTags v-model:value="sshForm.aliases" :disabled="sshFormFieldsDisabled" />
                    <span class="setting-desc">同一 Host 块的额外别名，保存时写入 Host 行。</span>
                  </label>

                  <label class="ssh-form-field">
                    <span class="field-label">IdentityFile</span>
                    <NInput
                      v-model:value="sshForm.identity_file"
                      :disabled="sshFormFieldsDisabled"
                      placeholder="~/.ssh/id_rsa"
                    />
                  </label>

                  <div class="ssh-form-grid">
                    <label class="ssh-form-field">
                      <span class="field-label">ProxyJump</span>
                      <NInput
                        v-model:value="sshForm.proxy_jump"
                        :disabled="sshFormFieldsDisabled || !!sshForm.proxy_command"
                        placeholder="jump-host"
                      />
                    </label>
                    <label class="ssh-form-field">
                      <span class="field-label">ProxyCommand</span>
                      <NInput
                        v-model:value="sshForm.proxy_command"
                        :disabled="sshFormFieldsDisabled || !!sshForm.proxy_jump"
                        placeholder="ssh -W %h:%p jump-host"
                      />
                    </label>
                  </div>

                  <div class="ssh-options">
                    <div class="ssh-options-head">
                      <button
                        type="button"
                        class="ssh-options-toggle"
                        :disabled="sshFormFieldsDisabled && !(sshForm.options?.length)"
                        @click="sshAdvancedOpen = !sshAdvancedOpen"
                      >
                        <span class="setting-title">高级选项</span>
                        <NTag
                          v-if="sshForm.options?.length"
                          size="tiny"
                          :bordered="false"
                        >{{ sshForm.options.length }}</NTag>
                        <span class="ssh-options-chevron">{{ sshAdvancedOpen ? "▾" : "▸" }}</span>
                      </button>
                      <NButton
                        v-if="sshAdvancedOpen"
                        size="small"
                        :disabled="sshFormFieldsDisabled"
                        @click="addSshOption"
                      >增加选项</NButton>
                    </div>
                    <template v-if="sshAdvancedOpen">
                      <div
                        v-for="(option, index) in sshForm.options"
                        :key="index"
                        class="ssh-option-row"
                      >
                        <NInput
                          v-model:value="option.key"
                          :disabled="sshFormFieldsDisabled"
                          placeholder="ConnectTimeout"
                        />
                        <NInput
                          v-model:value="option.value"
                          :disabled="sshFormFieldsDisabled"
                          placeholder="10"
                        />
                        <NButton
                          quaternary
                          type="error"
                          :disabled="sshFormFieldsDisabled"
                          @click="removeSshOption(index)"
                        >删除</NButton>
                      </div>
                      <p v-if="!sshForm.options.length" class="setting-desc">
                        可补充 ServerAliveInterval、ConnectTimeout、IdentitiesOnly 等 OpenSSH 选项。
                      </p>
                    </template>
                  </div>

                  <div class="ssh-host-feedback-rail" aria-live="polite">
                    <Transition name="ssh-host-feedback">
                      <NAlert
                        v-if="sshHostFeedback"
                        :type="sshHostFeedback.type"
                        :show-icon="false"
                        size="small"
                        class="ssh-host-feedback-alert"
                      >
                        {{ sshHostFeedback.text }}
                      </NAlert>
                    </Transition>
                  </div>

                  <div class="ssh-config-actions">
                    <template v-if="sshFormMode === 'create'">
                      <NButton :disabled="sshConfigSaving" @click="cancelNewSshEntry">取消</NButton>
                      <NButton
                        type="primary"
                        :loading="sshConfigSaving"
                        @click="saveSshConfigEntry"
                      >{{ sshSaveButtonLabel }}</NButton>
                    </template>
                    <template v-else-if="sshFormMode === 'readonly'">
                      <NButton
                        :loading="sshConfigLoading"
                        :disabled="sshConfigSaving"
                        @click="requestReloadSshConfig"
                      >重新加载</NButton>
                    </template>
                    <template v-else>
                      <NButton
                        :loading="sshConfigLoading"
                        :disabled="sshConfigSaving"
                        @click="requestReloadSshConfig"
                      >重新加载</NButton>
                      <NPopconfirm
                        :disabled="sshConfigSaving || !sshForm.editable"
                        @positive-click="deleteSshConfigEntry"
                      >
                        <template #trigger>
                          <NButton type="error" ghost :disabled="sshConfigSaving || !sshForm.editable">
                            删除 Host
                          </NButton>
                        </template>
                        删除后会从 ~/.ssh/config 移除该 Host 块，确认删除？
                      </NPopconfirm>
                      <NButton
                        type="primary"
                        :loading="sshConfigSaving"
                        :disabled="!sshForm.editable"
                        @click="saveSshConfigEntry"
                      >{{ sshSaveButtonLabel }}</NButton>
                    </template>
                  </div>
                </section>
              </div>
            </div>
          </NTabPane>
        </NTabs>
      </template>

      <!-- 安全与凭证 -->
      <template v-if="activeCategory === 'security'">
        <SettingsSecurityPanel />
      </template>

      <!-- MCP -->
      <template v-if="activeCategory === 'mcp'">
        <SettingsMcpPanel />
      </template>

      <!-- 数据与存储 -->
      <template v-if="activeCategory === 'data'">
        <SettingsDataPanel
          @open-workspace="activeCategory = 'workspace'"
          @open-debug="activeCategory = 'debug'"
        />
      </template>

      <!-- 外观 -->
      <template v-if="activeCategory === 'appearance'">
        <NCard title="界面主题" size="small" class="settings-card" :bordered="true">
          <p class="card-desc">主窗口、列表与对话框的明暗。与下方「代码主题」（YAML/Monaco 高亮）相互独立。</p>
          <NSelect
            :value="appChromeScheme"
            :options="APP_CHROME_OPTIONS"
            class="theme-select-naive"
            @update:value="setAppChromeScheme"
          />
        </NCard>
        <NCard title="代码主题" size="small" class="settings-card" :bordered="true">
          <p class="card-desc">
            为浅色、深色应用壳各选一套 YAML/Monaco 语法高亮；切换「界面主题」或系统明暗时，编辑器会在二者间自动选用。
          </p>
          <NSpace v-bind="kfSpace.editorThemeRow" class="editor-theme-row">
            <span class="editor-theme-label">浅色应用壳</span>
            <NSelect
              v-model:value="themeIdLight"
              :options="EDITOR_LIGHT_THEME_OPTIONS"
              class="theme-select-naive editor-theme-select"
              filterable
            />
          </NSpace>
          <NSpace v-bind="kfSpace.editorThemeRow" class="editor-theme-row">
            <span class="editor-theme-label">深色应用壳</span>
            <NSelect
              v-model:value="themeIdDark"
              :options="EDITOR_DARK_THEME_OPTIONS"
              class="theme-select-naive editor-theme-select"
              filterable
            />
          </NSpace>
          <p class="card-desc preview-desc">示例 YAML 会随当前应用壳使用上述对应侧主题，便于对比效果。</p>
          <div class="yaml-preview-wrap">
            <CodeEditor
              :key="`yaml-theme-preview-${activeYamlThemeId}`"
              :value="yamlThemePreview"
              language="yaml"
              :theme="monacoTheme"
              :options="previewOptions"
              class="yaml-preview-editor"
            />
          </div>
        </NCard>
        <NCard title="编辑器空白字符" size="small" class="settings-card" :bordered="true">
          <p class="card-desc">控制 ConfigMap / Secret 编辑器是否默认显示空白字符（空格显示为 ·，制表符显示为 →），同时开启保存时的控制字符校验。编辑器内也可临时切换。</p>
          <NSpace v-bind="kfSpace.buttonGroup">
            <NButton
              :type="currentWhitespaceRenderEnabled ? 'primary' : 'default'"
              :secondary="!currentWhitespaceRenderEnabled"
              :disabled="saving"
              @click="saveWhitespaceRenderEnabled(true)"
            >开启</NButton>
            <NButton
              :type="!currentWhitespaceRenderEnabled ? 'primary' : 'default'"
              :secondary="currentWhitespaceRenderEnabled"
              :disabled="saving"
              @click="saveWhitespaceRenderEnabled(false)"
            >关闭</NButton>
          </NSpace>
          <NAlert title="开启时" type="default" class="hint-alert">编辑器默认渲染空白字符，保存时会校验非常规控制字符（如 NUL、连续空行）并弹出确认提示。</NAlert>
          <NAlert title="关闭时" type="default" class="hint-alert">编辑器不显示空白字符，保存时不做控制字符校验。编辑器内仍可通过工具栏按钮临时开启。</NAlert>
          <NAlert v-if="message" class="msg-alert" :type="message === '已保存' ? 'success' : 'error'" :show-icon="true">{{ message }}</NAlert>
        </NCard>
      </template>
    </main>
  </div>
</template>

<style scoped>
.settings {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  background: var(--kf-bg-soft, #f8fafc);
}
.settings-nav {
  width: 200px;
  flex-shrink: 0;
  background: var(--kf-surface-strong, #fff);
  border-right: 1px solid var(--kf-border, #e2e8f0);
  padding: 0.75rem 0;
}
.settings-menu :deep(.n-menu-item-content) {
  font-size: 0.9375rem;
}
.settings-content {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: auto;
  padding: 1.5rem 1.5rem 1.25rem;
  box-sizing: border-box;
}
.settings-content:has(.ssh-host-panel) {
  /* Host 配置按视口适配高度，避免外层再滚出一条空白滚动条。 */
  overflow: hidden;
}
.page-header {
  margin-bottom: 1rem;
  flex-shrink: 0;
}
.page-title {
  margin: 0;
  font-size: 1.375rem;
  font-weight: 600;
  color: var(--kf-text-primary, #0f172a);
  letter-spacing: -0.02em;
}
.settings-card {
  max-width: 520px;
  margin-bottom: 1rem;
}
.ssh-settings-tabs {
  max-width: 960px;
  min-height: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
}
.ssh-settings-tabs :deep(.n-tabs-pane-wrapper) {
  padding-top: 0.75rem;
  flex: 1;
  min-height: 0;
}
.ssh-settings-tabs :deep(.n-tab-pane) {
  height: 100%;
}
.ssh-host-panel {
  max-width: 960px;
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.ssh-host-panel-desc {
  margin-bottom: 0.75rem;
  flex-shrink: 0;
}
.card-desc {
  margin: 0 0 1rem;
  font-size: 0.875rem;
  color: var(--kf-text-secondary, #64748b);
  line-height: 1.55;
}
.setting-row {
  margin-top: 1rem;
  padding-top: 1rem;
  border-top: 1px solid var(--kf-border);
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;
}
.setting-copy {
  flex: 1;
  min-width: 220px;
}
.setting-title {
  font-size: 0.875rem;
  font-weight: 600;
  color: #1e293b;
}
.setting-desc {
  margin-top: 0.3rem;
  font-size: 0.8125rem;
  line-height: 1.5;
  color: #64748b;
}
.setting-input-wrap {
  min-width: 0;
}
.setting-stack-wrap {
  min-width: 260px;
  flex: 1;
  width: 100%;
  align-items: stretch !important;
}
.setting-inline-actions {
  margin-top: 0.25rem;
}
.option-group {
  margin-top: 0.75rem;
}
.option-label {
  font-size: 0.8125rem;
  font-weight: 500;
  color: #64748b;
}
.hint-alert {
  margin-top: 0.75rem;
  background: var(--kf-bg-soft, #f8fafc);
}
.msg-alert {
  margin-top: 0.75rem;
}
.ssh-host-feedback-rail {
  min-height: 2.875rem;
  margin-top: 0.75rem;
}
.ssh-host-feedback-alert {
  margin-top: 0;
}
.ssh-host-feedback-enter-active,
.ssh-host-feedback-leave-active {
  transition: opacity 0.2s ease;
}
.ssh-host-feedback-enter-from,
.ssh-host-feedback-leave-to {
  opacity: 0;
}
.num-compact {
  width: 100px;
}
.gpu-rule-row {
  display: grid;
  grid-template-columns: 1fr 1.4fr auto;
  gap: 0.5rem;
  align-items: center;
  margin-bottom: 0.5rem;
}
.preview-desc {
  margin-top: 1rem;
  margin-bottom: 0.6rem;
}
.editor-theme-row {
  margin-bottom: 0.75rem;
  width: 100%;
}
.editor-theme-label {
  flex: 0 0 6.5rem;
  font-size: 0.8125rem;
  color: var(--kf-text-secondary, #64748b);
}
.editor-theme-select {
  flex: 1;
  min-width: 200px;
  max-width: 100%;
}
.yaml-preview-wrap {
  height: 300px;
  border: 1px solid var(--kf-border, #e2e8f0);
  border-radius: 8px;
  overflow: hidden;
}
.yaml-preview-editor {
  height: 100%;
}
.theme-select-naive {
  max-width: 280px;
}
.ssh-config-layout {
  display: grid;
  grid-template-columns: 260px minmax(0, 1fr);
  gap: 1rem;
  align-items: stretch;
}
.ssh-host-list {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--kf-border, #e2e8f0);
  border-radius: 8px;
  overflow: hidden;
  background: var(--kf-bg-soft, #f8fafc);
}
.ssh-host-list-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
}
.ssh-host-list-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
  padding: 0.6rem 0.7rem;
  border-bottom: 1px solid var(--kf-border, #e2e8f0);
  font-size: 0.8125rem;
  font-weight: 600;
  color: var(--kf-text-secondary, #64748b);
}
.ssh-host-list-filter {
  flex-shrink: 0;
  padding: 0.5rem 0.7rem;
  border-bottom: 1px solid var(--kf-border, #e2e8f0);
}
.ssh-host-item {
  width: 100%;
  border: 0;
  border-bottom: 1px solid var(--kf-border, #e2e8f0);
  border-left: 2px solid transparent;
  background: transparent;
  color: inherit;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.2rem;
  padding: 0.65rem 0.7rem;
  text-align: left;
  max-width: 100%;
  overflow: hidden;
}
.ssh-host-item:last-child {
  border-bottom: 0;
}
.ssh-host-item:hover {
  background: var(--kf-surface-strong, #fff);
}
.ssh-host-item.active {
  background: var(--kf-surface-strong, #fff);
  border-left-color: var(--kf-accent, #2563eb);
}
.ssh-host-item.disabled {
  opacity: 0.72;
}
.ssh-host-name,
.ssh-host-target {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ssh-host-name {
  font-size: 0.875rem;
  font-weight: 600;
  color: var(--kf-text-primary, #0f172a);
}
.ssh-host-target,
.ssh-host-empty {
  font-size: 0.75rem;
  line-height: 1.4;
  color: var(--kf-text-muted, #94a3b8);
}
.ssh-host-badges {
  display: flex;
  flex-wrap: wrap;
  gap: 0.25rem;
  margin-top: 0.15rem;
}
.ssh-host-empty {
  padding: 0.8rem;
}
.ssh-host-clear-filter {
  border: 0;
  padding: 0;
  margin-left: 0.25rem;
  background: transparent;
  color: var(--kf-accent, #2563eb);
  cursor: pointer;
  font: inherit;
  text-decoration: underline;
}
.ssh-config-form {
  min-width: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding-right: 0.25rem;
}
.ssh-form-mode-bar {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.75rem;
}
.ssh-form-mode-title {
  font-size: 0.9375rem;
  font-weight: 600;
  color: var(--kf-text-primary, #0f172a);
}
.ssh-readonly-alert {
  margin-bottom: 0.75rem;
}
.ssh-form-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.75rem;
  margin-bottom: 0.75rem;
}
.ssh-form-field {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  margin-bottom: 0.75rem;
}
.ssh-options {
  margin-top: 0.25rem;
  padding-top: 0.9rem;
  border-top: 1px solid var(--kf-border, #e2e8f0);
}
.ssh-options-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  margin-bottom: 0.6rem;
}
.ssh-options-toggle {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  border: 0;
  padding: 0;
  background: transparent;
  color: inherit;
  cursor: pointer;
  font: inherit;
}
.ssh-options-chevron {
  font-size: 0.75rem;
  color: var(--kf-text-muted, #94a3b8);
}
.ssh-option-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1.4fr) auto;
  gap: 0.5rem;
  align-items: center;
  margin-bottom: 0.5rem;
}
.ssh-config-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 1rem;
  flex-wrap: wrap;
}
@media (max-width: 900px) {
  .ssh-config-layout {
    grid-template-columns: 1fr;
  }
  .ssh-config-form {
    max-height: none !important;
  }
  .ssh-form-grid,
  .ssh-option-row {
    grid-template-columns: 1fr;
  }
}
</style>
