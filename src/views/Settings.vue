<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import type { MenuOption } from "naive-ui";
import {
  NAlert,
  NButton,
  NCard,
  NInput,
  NInputNumber,
  NMenu,
  NPopconfirm,
  NSelect,
  NSpace,
  NTag,
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

type CategoryId = "appearance" | "workspace" | "debug" | "ssh" | "security";

const CATEGORIES: { id: CategoryId; label: string; icon: string }[] = [
  { id: "appearance", label: "外观", icon: "🎨" },
  { id: "workspace", label: "工作流", icon: "🧭" },
  { id: "debug", label: "调试", icon: "🔧" },
  { id: "ssh", label: "SSH 隧道", icon: "📡" },
  { id: "security", label: "安全与凭证", icon: "🔒" },
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
const currentWhitespaceRenderEnabled = ref(false);
const builtinGpuResourceNames = ref<string[]>([]);
const customGpuResourceRules = ref<GpuResourceRule[]>([]);
const sshConfigPath = ref("");
const sshConfigEntries = ref<SshConfigEntry[]>([]);
const selectedSshHost = ref("");
const sshConfigLoading = ref(false);
const sshConfigSaving = ref(false);
const sshConfigError = ref("");
const sshConfigMessage = ref("");
const sshForm = ref<SshConfigEntry>(emptySshConfigEntry());
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
    editable: true,
    issues: [],
  };
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

function selectSshEntry(entry: SshConfigEntry, opts?: { preserveFeedback?: boolean }) {
  selectedSshHost.value = entry.host;
  sshForm.value = cloneSshEntry(entry);
  if (!opts?.preserveFeedback) {
    clearSshFeedback();
  }
}

function startNewSshEntry(opts?: { preserveFeedback?: boolean }) {
  selectedSshHost.value = "";
  sshForm.value = emptySshConfigEntry();
  if (!opts?.preserveFeedback) {
    clearSshFeedback();
  }
}

/** 保存/删除后静默刷新列表，避免整表重载导致表单与提示条闪烁。 */
function syncSshEntryMetadata(entry: SshConfigEntry) {
  sshForm.value.issues = [...(entry.issues ?? [])];
  sshForm.value.editable = entry.editable;
  sshForm.value.source_file = entry.source_file;
  sshForm.value.aliases = [...(entry.aliases ?? [])];
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
    return;
  }
  const saved = entries.find((entry) => entry.host === host);
  if (saved && selectedSshHost.value === host) {
    syncSshEntryMetadata(saved);
    return;
  }
  if (saved) {
    selectSshEntry(saved, opts);
    return;
  }
  if (entries.length) selectSshEntry(entries[0], opts);
  else startNewSshEntry(opts);
}

function addSshOption() {
  sshForm.value.options = [...(sshForm.value.options ?? []), { key: "", value: "" }];
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
  } catch (e) {
    showSshError(e instanceof Error ? e.message : String(e));
    sshConfigEntries.value = [];
  } finally {
    sshConfigLoading.value = false;
  }
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
    currentWhitespaceRenderEnabled.value = false;
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
});

watch(activeCategory, (value) => {
  window.sessionStorage.setItem("kube-flow:settings-category", value);
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
              <div class="setting-desc">默认 6 个。终端中心会保留最近活跃的终端实例屏幕历史，超出后仅回收前端实例，不会关闭会话本身。</div>
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
        <NCard title="默认映射方式" size="small" class="settings-card" :bordered="true">
          <p class="card-desc">
            新建 SSH 隧道或未显式配置的隧道将使用此映射方式。ssh 使用系统 ssh -L 子进程（兼容性最好），builtin 使用 libssh2 内置转发（无子进程）。
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

        <NCard title="SSH Host 配置" size="small" class="settings-card ssh-config-card" :bordered="true">
          <p class="card-desc">
            管理本机 SSH 配置中的普通 Host。保存时会写入 {{ sshConfigPath || "~/.ssh/config" }}，并在同目录生成 kube-flow 备份。
          </p>
          <div class="ssh-config-layout">
            <aside class="ssh-host-list">
              <div class="ssh-host-list-header">
                <span>Host</span>
                <NButton size="small" :disabled="sshConfigLoading || sshConfigSaving" @click="() => startNewSshEntry()"
                  >新增</NButton>
              </div>
              <button
                v-for="entry in sshConfigEntries"
                :key="entry.host"
                type="button"
                class="ssh-host-item"
                :class="{ active: selectedSshHost === entry.host, disabled: !entry.editable }"
                @click="selectSshEntry(entry)"
              >
                <span class="ssh-host-name">{{ entry.host }}</span>
                <span class="ssh-host-target">{{ formatSshEntryLabel(entry) }}</span>
              </button>
              <div v-if="!sshConfigEntries.length && !sshConfigLoading" class="ssh-host-empty">
                还没有 Host，新增一个即可用于 SSH 隧道环境。
              </div>
            </aside>

            <section class="ssh-config-form">
              <div class="ssh-form-grid">
                <label class="ssh-form-field">
                  <span class="field-label">Host 别名</span>
                  <NInput v-model:value="sshForm.host" :disabled="sshConfigSaving || !sshForm.editable" placeholder="prod-bastion" />
                </label>
                <label class="ssh-form-field">
                  <span class="field-label">HostName</span>
                  <NInput v-model:value="sshForm.hostname" :disabled="sshConfigSaving || !sshForm.editable" placeholder="10.0.0.12 或 bastion.example.com" />
                </label>
                <label class="ssh-form-field">
                  <span class="field-label">User</span>
                  <NInput v-model:value="sshForm.user" :disabled="sshConfigSaving || !sshForm.editable" placeholder="root" />
                </label>
                <label class="ssh-form-field">
                  <span class="field-label">Port</span>
                  <NInputNumber
                    v-model:value="sshForm.port"
                    :min="1"
                    :max="65535"
                    :show-button="false"
                    :disabled="sshConfigSaving || !sshForm.editable"
                    placeholder="22"
                  />
                </label>
              </div>

              <label class="ssh-form-field">
                <span class="field-label">IdentityFile</span>
                <NInput v-model:value="sshForm.identity_file" :disabled="sshConfigSaving || !sshForm.editable" placeholder="~/.ssh/id_rsa" />
              </label>

              <div class="ssh-form-grid">
                <label class="ssh-form-field">
                  <span class="field-label">ProxyJump</span>
                  <NInput v-model:value="sshForm.proxy_jump" :disabled="sshConfigSaving || !sshForm.editable || !!sshForm.proxy_command" placeholder="jump-host" />
                </label>
                <label class="ssh-form-field">
                  <span class="field-label">ProxyCommand</span>
                  <NInput v-model:value="sshForm.proxy_command" :disabled="sshConfigSaving || !sshForm.editable || !!sshForm.proxy_jump" placeholder="ssh -W %h:%p jump-host" />
                </label>
              </div>

              <div class="ssh-options">
                <div class="ssh-options-head">
                  <span class="setting-title">高级选项</span>
                  <NButton size="small" :disabled="sshConfigSaving || !sshForm.editable" @click="addSshOption">增加选项</NButton>
                </div>
                <div
                  v-for="(option, index) in sshForm.options"
                  :key="index"
                  class="ssh-option-row"
                >
                  <NInput v-model:value="option.key" :disabled="sshConfigSaving || !sshForm.editable" placeholder="ConnectTimeout" />
                  <NInput v-model:value="option.value" :disabled="sshConfigSaving || !sshForm.editable" placeholder="10" />
                  <NButton quaternary type="error" :disabled="sshConfigSaving || !sshForm.editable" @click="removeSshOption(index)">删除</NButton>
                </div>
                <p v-if="!sshForm.options.length" class="setting-desc">可补充 ServerAliveInterval、ConnectTimeout、IdentitiesOnly 等 OpenSSH 选项。</p>
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
                <NButton :loading="sshConfigLoading" :disabled="sshConfigSaving" @click="() => loadSshConfigEntries()"
                  >重新加载</NButton>
                <NPopconfirm
                  v-if="selectedSshHost"
                  :disabled="sshConfigSaving || !sshForm.editable"
                  @positive-click="deleteSshConfigEntry"
                >
                  <template #trigger>
                    <NButton type="error" ghost :disabled="sshConfigSaving || !sshForm.editable">删除 Host</NButton>
                  </template>
                  删除后会从 ~/.ssh/config 移除该 Host 块，确认删除？
                </NPopconfirm>
                <NButton type="primary" :loading="sshConfigSaving" :disabled="!sshForm.editable" @click="saveSshConfigEntry">保存 Host</NButton>
              </div>
            </section>
          </div>
        </NCard>
      </template>

      <!-- 安全与凭证 -->
      <template v-if="activeCategory === 'security'">
        <SettingsSecurityPanel />
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
  overflow: auto;
  padding: 2rem 1.5rem;
}
.page-header {
  margin-bottom: 1.5rem;
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
.ssh-config-card {
  max-width: 920px;
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
  grid-template-columns: 250px minmax(0, 1fr);
  gap: 1rem;
  align-items: start;
}
.ssh-host-list {
  border: 1px solid var(--kf-border, #e2e8f0);
  border-radius: 8px;
  overflow: hidden;
  background: var(--kf-bg-soft, #f8fafc);
}
.ssh-host-list-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.6rem 0.7rem;
  border-bottom: 1px solid var(--kf-border, #e2e8f0);
  font-size: 0.8125rem;
  font-weight: 600;
  color: var(--kf-text-secondary, #64748b);
}
.ssh-host-item {
  width: 100%;
  border: 0;
  border-bottom: 1px solid var(--kf-border, #e2e8f0);
  background: transparent;
  color: inherit;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.2rem;
  padding: 0.7rem;
  text-align: left;
}
.ssh-host-item:last-child {
  border-bottom: 0;
}
.ssh-host-item:hover,
.ssh-host-item.active {
  background: var(--kf-surface-strong, #fff);
}
.ssh-host-item.disabled {
  opacity: 0.62;
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
.ssh-host-empty {
  padding: 0.8rem;
}
.ssh-config-form {
  min-width: 0;
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
  .ssh-form-grid,
  .ssh-option-row {
    grid-template-columns: 1fr;
  }
}
</style>
