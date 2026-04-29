<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import type { MenuOption } from "naive-ui";
import {
  NAlert,
  NButton,
  NCard,
  NInput,
  NInputNumber,
  NMenu,
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
  appSettingsSetAutoSnapshotEnabled,
  appSettingsSetAutoSnapshotLimitPerResource,
  appSettingsSetCustomGpuResourceRules,
  appSettingsSetLogActiveStreamLimit,
  appSettingsSetNodeResourceUsageEnabled,
  appSettingsSetResourceDeployStrategy,
  appSettingsSetTerminalInstanceCacheLimit,
  appSettingsSetSshTunnelMode,
  type GpuResourceRule,
  type ResourceDeployStrategy,
  type TunnelMappingMode,
} from "../api/config";
import { useAppSettingsStore } from "../stores/appSettings";
import { useEnvStore } from "../stores/env";
import { appChromeScheme, APP_CHROME_OPTIONS } from "../stores/appChromeTheme";
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
const { autoSnapshotEnabled, autoSnapshotLimitPerResource, terminalInstanceCacheLimit, logActiveStreamLimit, nodeResourceUsageEnabled } = useAppSettingsStore();
const { loadEnvironments } = useEnvStore();
const activeCategory = ref<CategoryId>("appearance");
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
const builtinGpuResourceNames = ref<string[]>([]);
const customGpuResourceRules = ref<GpuResourceRule[]>([]);
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

async function load() {
  try {
    const [level, settings, sshMode, autoSnapshot, autoSnapshotLimit, resourceDeployStrategy, terminalCacheLimit, activeLogLimit, nodeUsageEnabled, builtinGpuNames, customGpuRules] = await Promise.all([
      logGetLevel(),
      logGetDisplaySettings(),
      appSettingsGetSshTunnelMode(),
      appSettingsGetAutoSnapshotEnabled(),
      appSettingsGetAutoSnapshotLimitPerResource(),
      appSettingsGetResourceDeployStrategy(),
      appSettingsGetTerminalInstanceCacheLimit(),
      appSettingsGetLogActiveStreamLimit(),
      appSettingsGetNodeResourceUsageEnabled(),
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
    builtinGpuResourceNames.value = builtinGpuNames;
    customGpuResourceRules.value = customGpuRules.length ? customGpuRules : [{ display_name: "", resource_name: "" }];
    autoSnapshotEnabled.value = autoSnapshot;
    autoSnapshotLimitPerResource.value = currentAutoSnapshotLimitPerResource.value;
    terminalInstanceCacheLimit.value = currentTerminalInstanceCacheLimit.value;
    logActiveStreamLimit.value = currentLogActiveStreamLimit.value;
    nodeResourceUsageEnabled.value = currentNodeResourceUsageEnabled.value;
  } catch {
    currentLevel.value = "off";
    currentResourceDeployStrategy.value = "create_replace";
    currentTerminalInstanceCacheLimit.value = 6;
    currentLogActiveStreamLimit.value = 3;
    currentNodeResourceUsageEnabled.value = false;
    builtinGpuResourceNames.value = ["*/gpu"];
    customGpuResourceRules.value = [{ display_name: "", resource_name: "" }];
  }
  await loadEnvironments().catch(() => {});
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
        <NCard title="调试日志采集" size="small" class="settings-card" :bordered="true">
          <p class="card-desc">日志级别决定写入 kube-flow-debug.log 的内容量，用于排查资源列表、连接与后端行为问题。</p>
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
          <p class="card-desc" style="margin-top: 0.5rem">调试日志格式：仅作用于调试日志页，不影响 Pod 或 Workload 日志输出。</p>
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
          <p class="card-desc">这里的顺序会同时作用于日志中心里的资源日志和调试日志。</p>
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
            v-model:value="appChromeScheme"
            :options="APP_CHROME_OPTIONS"
            class="theme-select-naive"
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
</style>
