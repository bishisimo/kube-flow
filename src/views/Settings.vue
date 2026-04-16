<script setup lang="ts">
import { ref, onMounted } from "vue";
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
import { useYamlTheme, useYamlMonacoTheme, YAML_THEMES } from "../stores/yamlTheme";
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
import { extractErrorMessage } from "../utils/errorMessage";
import { useAppSettingsStore } from "../stores/appSettings";
import { useEnvStore } from "../stores/env";
import { useStrongholdStatusStore } from "../stores/strongholdStatus";
import {
  securityGetSettings,
  securitySetCredentialStore,
  securitySetAutoLockMinutes,
  securitySetStrongholdPath,
  strongholdGetStatus,
  strongholdInitialize,
  strongholdUnlock,
  strongholdLock,
  credentialList,
  credentialDelete,
  type SecurityConfig,
  type CredentialInfo,
  type CredentialStoreKind,
} from "../api/credential";

type CategoryId = "appearance" | "workspace" | "debug" | "ssh" | "security";

const CATEGORIES: { id: CategoryId; label: string; icon: string }[] = [
  { id: "appearance", label: "外观", icon: "🎨" },
  { id: "workspace", label: "工作流", icon: "🧭" },
  { id: "debug", label: "调试", icon: "🔧" },
  { id: "ssh", label: "SSH 隧道", icon: "📡" },
  { id: "security", label: "安全与凭证", icon: "🔒" },
];

const { themeId } = useYamlTheme();
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

// 安全与凭证
const securityCfg = ref<SecurityConfig>({
  credential_store: "stronghold",
  stronghold_snapshot_path: "",
  auto_lock_minutes: 30,
});
const { strongholdStatus, refreshStrongholdStatus, setStrongholdStatus } = useStrongholdStatusStore();
const savedCredentials = ref<CredentialInfo[]>([]);
const masterPasswordInput = ref("");
const masterPasswordConfirm = ref("");
const showMasterPassword = ref(false);
const securityMsg = ref<string | null>(null);
const securityMsgIsError = ref(false);
const securityLoading = ref(false);
const editingStrongholdPath = ref(false);
const tempStrongholdPath = ref("");

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

async function loadSecurity() {
  try {
    const [cfg, status, credentials] = await Promise.all([
      securityGetSettings(),
      strongholdGetStatus(),
      credentialList(),
    ]);
    securityCfg.value = cfg;
    setStrongholdStatus(status);
    savedCredentials.value = credentials;
  } catch {
    // 静默处理
  }
}

function showSecurityMsg(msg: string, isError = false) {
  securityMsg.value = msg;
  securityMsgIsError.value = isError;
  setTimeout(() => (securityMsg.value = null), 3000);
}

async function handleSetCredentialStore(store: CredentialStoreKind) {
  securityLoading.value = true;
  try {
    await securitySetCredentialStore(store);
    securityCfg.value.credential_store = store;
    await refreshStrongholdStatus();
    showSecurityMsg("已保存");
  } catch (e) {
    showSecurityMsg(extractErrorMessage(e), true);
  } finally {
    securityLoading.value = false;
  }
}

async function handleAutoLockChange(minutes: number) {
  try {
    await securitySetAutoLockMinutes(minutes);
    securityCfg.value.auto_lock_minutes = minutes;
    showSecurityMsg("已保存");
  } catch (e) {
    showSecurityMsg(extractErrorMessage(e), true);
  }
}

async function handleSaveStrongholdPath() {
  if (!tempStrongholdPath.value.trim()) return;
  securityLoading.value = true;
  try {
    await securitySetStrongholdPath(tempStrongholdPath.value.trim());
    securityCfg.value.stronghold_snapshot_path = tempStrongholdPath.value.trim();
    await refreshStrongholdStatus();
    editingStrongholdPath.value = false;
    showSecurityMsg("路径已更新");
  } catch (e) {
    showSecurityMsg(extractErrorMessage(e), true);
  } finally {
    securityLoading.value = false;
  }
}

async function handleStrongholdInit() {
  if (!masterPasswordInput.value || masterPasswordInput.value !== masterPasswordConfirm.value) {
    showSecurityMsg("两次密码不一致", true);
    return;
  }
  securityLoading.value = true;
  try {
    await strongholdInitialize(masterPasswordInput.value);
    setStrongholdStatus("unlocked");
    masterPasswordInput.value = "";
    masterPasswordConfirm.value = "";
    showSecurityMsg("Stronghold 已初始化并解锁");
  } catch (e) {
    showSecurityMsg(extractErrorMessage(e), true);
  } finally {
    securityLoading.value = false;
  }
}

async function handleStrongholdUnlock() {
  if (!masterPasswordInput.value) {
    showSecurityMsg("请输入主密码", true);
    return;
  }
  securityLoading.value = true;
  try {
    await strongholdUnlock(masterPasswordInput.value);
    setStrongholdStatus("unlocked");
    masterPasswordInput.value = "";
    savedCredentials.value = await credentialList();
    showSecurityMsg("已解锁");
  } catch (e) {
    showSecurityMsg(extractErrorMessage(e), true);
  } finally {
    securityLoading.value = false;
  }
}

async function handleStrongholdLock() {
  await strongholdLock();
  setStrongholdStatus("locked");
  savedCredentials.value = [];
  showSecurityMsg("已锁定");
}

async function handleDeleteCredential(tunnelId: string) {
  securityLoading.value = true;
  try {
    await credentialDelete(tunnelId);
    savedCredentials.value = savedCredentials.value.filter((c) => c.tunnel_id !== tunnelId);
    showSecurityMsg("凭证已删除");
  } catch (e) {
    showSecurityMsg(extractErrorMessage(e), true);
  } finally {
    securityLoading.value = false;
  }
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
  loadSecurity();
});

</script>

<template>
  <div class="settings">
    <aside class="settings-nav">
      <nav class="nav-list">
        <button
          v-for="cat in CATEGORIES"
          :key="cat.id"
          type="button"
          class="nav-item"
          :class="{ active: activeCategory === cat.id }"
          @click="activeCategory = cat.id"
        >
          <span class="nav-icon">{{ cat.icon }}</span>
          <span class="nav-label">{{ cat.label }}</span>
        </button>
      </nav>
    </aside>
    <main class="settings-content">
      <header class="page-header">
        <h1 class="page-title">
          {{ CATEGORIES.find((c) => c.id === activeCategory)?.label ?? "设置" }}
        </h1>
      </header>

      <!-- 调试 -->
      <template v-if="activeCategory === 'workspace'">
        <section class="card">
          <h2 class="card-title">快照工作流</h2>
          <p class="card-desc">
            自动快照会在应用配置、编辑 YAML、修改镜像前自动生成历史快照。关闭后不再自动生成，但仍可在快照中心手动生成。
          </p>
          <div class="level-options">
            <button
              type="button"
              class="level-btn"
              :class="{ active: currentAutoSnapshotEnabled }"
              :disabled="saving"
              @click="saveAutoSnapshotEnabled(true)"
            >
              开启自动快照
            </button>
            <button
              type="button"
              class="level-btn"
              :class="{ active: !currentAutoSnapshotEnabled }"
              :disabled="saving"
              @click="saveAutoSnapshotEnabled(false)"
            >
              关闭自动快照
            </button>
          </div>
          <div class="setting-row">
            <div class="setting-copy">
              <div class="setting-title">每个资源自动快照上限</div>
              <div class="setting-desc">默认 10 个。超过上限后，会自动删除最旧的自动快照，手动快照不受影响；设置为 0 表示不自动淘汰。</div>
            </div>
            <div class="setting-input-wrap">
              <input
                v-model.number="currentAutoSnapshotLimitPerResource"
                type="number"
                min="0"
                max="100"
                step="1"
                class="setting-number-input"
                :disabled="saving"
                @blur="saveAutoSnapshotLimitPerResource(currentAutoSnapshotLimitPerResource)"
              />
              <button
                type="button"
                class="level-btn"
                :disabled="saving"
                @click="saveAutoSnapshotLimitPerResource(currentAutoSnapshotLimitPerResource)"
              >
                保存上限
              </button>
            </div>
          </div>
          <p v-if="message" class="message" :class="{ error: message !== '已保存' }">
            <span v-if="message === '已保存'" class="message-icon">✓</span>
            {{ message }}
          </p>
        </section>

        <section class="card">
          <h2 class="card-title">终端工作流</h2>
          <p class="card-desc">控制终端中心在前端保留多少个最近活跃的终端实例，用来平衡切换体验和内存占用。</p>
          <div class="setting-row">
            <div class="setting-copy">
              <div class="setting-title">终端实例缓存数量</div>
              <div class="setting-desc">默认 6 个。终端中心会保留最近活跃的终端实例屏幕历史，超出后仅回收前端实例，不会关闭会话本身。</div>
            </div>
            <div class="setting-input-wrap">
              <input
                v-model.number="currentTerminalInstanceCacheLimit"
                type="number"
                min="1"
                max="20"
                step="1"
                class="setting-number-input"
                :disabled="saving"
                @blur="saveTerminalInstanceCacheLimit(currentTerminalInstanceCacheLimit)"
              />
              <button
                type="button"
                class="level-btn"
                :disabled="saving"
                @click="saveTerminalInstanceCacheLimit(currentTerminalInstanceCacheLimit)"
              >
                保存数量
              </button>
            </div>
          </div>
          <p v-if="message" class="message" :class="{ error: message !== '已保存' }">
            <span v-if="message === '已保存'" class="message-icon">✓</span>
            {{ message }}
          </p>
        </section>

        <section class="card">
          <h2 class="card-title">编排中心下发</h2>
          <p class="card-desc">控制编排中心把 YAML 投放到目标环境时采用的策略。工作台编辑现有资源仍保持原有覆盖逻辑，不受这里影响。</p>
          <div class="level-options">
            <button
              type="button"
              class="level-btn"
              :class="{ active: currentResourceDeployStrategy === 'create_replace' }"
              :disabled="saving"
              @click="saveResourceDeployStrategy('create_replace')"
            >
              Create + Replace
            </button>
            <button
              type="button"
              class="level-btn"
              :class="{ active: currentResourceDeployStrategy === 'apply' }"
              :disabled="saving"
              @click="saveResourceDeployStrategy('apply')"
            >
              Apply
            </button>
          </div>
          <div class="setting-hint-block">
            <div class="setting-hint-title">Create + Replace</div>
            <div class="setting-hint-desc">目标资源不存在时先创建，已存在时按完整 YAML 覆盖，更接近模板投放。</div>
          </div>
          <div class="setting-hint-block">
            <div class="setting-hint-title">Apply</div>
            <div class="setting-hint-desc">使用 server-side apply 合并字段，更适合与其他控制器共享对象所有权。</div>
          </div>
          <p v-if="message" class="message" :class="{ error: message !== '已保存' }">
            <span v-if="message === '已保存'" class="message-icon">✓</span>
            {{ message }}
          </p>
        </section>

        <section class="card">
          <h2 class="card-title">Node 资源统计</h2>
          <p class="card-desc">控制工作台 Node 列表是否展示资源统计信息。默认关闭，按需开启。</p>
          <div class="level-options">
            <button
              type="button"
              class="level-btn"
              :class="{ active: currentNodeResourceUsageEnabled }"
              :disabled="saving"
              @click="saveNodeResourceUsageEnabled(true)"
            >
              开启统计
            </button>
            <button
              type="button"
              class="level-btn"
              :class="{ active: !currentNodeResourceUsageEnabled }"
              :disabled="saving"
              @click="saveNodeResourceUsageEnabled(false)"
            >
              关闭统计
            </button>
          </div>
          <div class="setting-hint-block">
            <div class="setting-hint-title">关闭时</div>
            <div class="setting-hint-desc">只展示节点基础状态，不启动额外的资源统计请求。</div>
          </div>
          <div class="setting-hint-block">
            <div class="setting-hint-title">开启时</div>
            <div class="setting-hint-desc">在 Node 列表中展示资源统计信息，帮助观察节点容量分布。</div>
          </div>
          <div class="setting-row">
            <div class="setting-copy">
              <div class="setting-title">内置 GPU 识别规则</div>
              <div class="setting-desc">默认支持通用的 GPU 资源模式识别，自定义规则用于补充特殊资源名或自定义显示名称。</div>
            </div>
            <div class="setting-input-wrap setting-stack-wrap">
              <div class="tag-list">
                <span v-for="name in builtinGpuResourceNames" :key="name" class="hint-chip">{{ name }}</span>
              </div>
            </div>
          </div>
          <div class="setting-row">
            <div class="setting-copy">
              <div class="setting-title">自定义 GPU 规则</div>
              <div class="setting-desc">用于补充特殊资源名，或为已识别的资源配置更友好的显示名称。</div>
            </div>
            <div class="setting-input-wrap setting-stack-wrap">
              <div class="gpu-rule-list">
                <div v-for="(rule, index) in customGpuResourceRules" :key="index" class="gpu-rule-row">
                  <input
                    v-model="rule.display_name"
                    type="text"
                    class="setting-text-input"
                    :disabled="saving"
                    placeholder="显示名称，例如 A100"
                  />
                  <input
                    v-model="rule.resource_name"
                    type="text"
                    class="setting-text-input"
                    :disabled="saving"
                    placeholder="资源名称，例如 vendor.com/gpu"
                  />
                  <button
                    type="button"
                    class="btn-row-remove"
                    :disabled="saving"
                    @click="removeGpuRuleRow(index)"
                  >
                    删除
                  </button>
                </div>
              </div>
              <div class="setting-inline-actions">
                <button
                  type="button"
                  class="level-btn"
                  :disabled="saving"
                  @click="addGpuRuleRow"
                >
                  增加规则
                </button>
                <button
                  type="button"
                  class="level-btn"
                  :disabled="saving"
                  @click="saveCustomGpuResourceNames"
                >
                  保存规则
                </button>
              </div>
            </div>
          </div>
          <p v-if="message" class="message" :class="{ error: message !== '已保存' }">
            <span v-if="message === '已保存'" class="message-icon">✓</span>
            {{ message }}
          </p>
        </section>
      </template>

      <!-- 调试 -->
      <template v-if="activeCategory === 'debug'">
        <section class="card">
          <h2 class="card-title">调试日志采集</h2>
          <p class="card-desc">日志级别决定写入 kube-flow-debug.log 的内容量，用于排查资源列表、连接与后端行为问题。</p>
          <div class="level-options">
            <button
              v-for="opt in LOG_LEVELS"
              :key="opt.value"
              type="button"
              class="level-btn"
              :class="{ active: currentLevel === opt.value }"
              :disabled="saving"
              @click="saveLevel(opt.value)"
            >
              {{ opt.label }}
            </button>
          </div>
          <p class="card-desc" style="margin-top: 0.5rem">调试日志格式：仅作用于调试日志页，不影响 Pod 或 Workload 日志输出。</p>
          <div class="option-group">
            <label class="option-label">格式</label>
            <div class="option-buttons">
              <button
                v-for="opt in LOG_DISPLAY_FORMATS"
                :key="opt.value"
                type="button"
                class="level-btn"
                :class="{ active: currentFormat === opt.value }"
                :disabled="saving"
                @click="saveDisplaySettings(currentOrder, opt.value)"
              >
                {{ opt.label }}
              </button>
            </div>
          </div>
        </section>

        <section class="card">
          <h2 class="card-title">全局日志显示</h2>
          <p class="card-desc">这里的顺序会同时作用于日志中心里的资源日志和调试日志。</p>
          <div class="display-options">
            <div class="option-group">
              <label class="option-label">顺序</label>
              <div class="option-buttons">
                <button
                  v-for="opt in LOG_DISPLAY_ORDERS"
                  :key="opt.value"
                  type="button"
                  class="level-btn"
                  :class="{ active: currentOrder === opt.value }"
                  :disabled="saving"
                  @click="saveDisplaySettings(opt.value, currentFormat)"
                >
                  {{ opt.label }}
                </button>
              </div>
            </div>
          </div>
          <div class="setting-row">
            <div class="setting-copy">
              <div class="setting-title">默认展示行数</div>
              <div class="setting-desc">新打开的资源日志默认按这个行数加载，默认 100 行。</div>
            </div>
            <div class="setting-input-wrap">
              <input
                v-model.number="currentLogTailLines"
                type="number"
                min="1"
                max="5000"
                step="1"
                class="setting-number-input"
                :disabled="saving"
                @blur="saveLogTailLines(currentLogTailLines)"
              />
              <button
                type="button"
                class="level-btn"
                :disabled="saving"
                @click="saveLogTailLines(currentLogTailLines)"
              >
                保存行数
              </button>
            </div>
          </div>
          <p v-if="message" class="message" :class="{ error: message !== '已保存' }">
            <span v-if="message === '已保存'" class="message-icon">✓</span>
            {{ message }}
          </p>
        </section>

        <section class="card">
          <h2 class="card-title">日志中心</h2>
          <p class="card-desc">控制日志中心同时保活的实时 follow 日志流数量。超出上限的会话会保留已加载内容，但暂停实时流。</p>
          <div class="setting-row">
            <div class="setting-copy">
              <div class="setting-title">活跃日志流数量</div>
              <div class="setting-desc">默认 3 个。最近活跃的日志会话会优先保留实时 follow，切换回来时会自动恢复。</div>
            </div>
            <div class="setting-input-wrap">
              <input
                v-model.number="currentLogActiveStreamLimit"
                type="number"
                min="1"
                max="12"
                step="1"
                class="setting-number-input"
                :disabled="saving"
                @blur="saveLogActiveStreamLimit(currentLogActiveStreamLimit)"
              />
              <button
                type="button"
                class="level-btn"
                :disabled="saving"
                @click="saveLogActiveStreamLimit(currentLogActiveStreamLimit)"
              >
                保存数量
              </button>
            </div>
          </div>
          <p v-if="message" class="message" :class="{ error: message !== '已保存' }">
            <span v-if="message === '已保存'" class="message-icon">✓</span>
            {{ message }}
          </p>
        </section>
      </template>

      <!-- SSH 隧道 -->
      <template v-if="activeCategory === 'ssh'">
        <section class="card">
          <h2 class="card-title">默认映射方式</h2>
          <p class="card-desc">
            新建 SSH 隧道或未显式配置的隧道将使用此映射方式。ssh 使用系统 ssh -L 子进程（兼容性最好），builtin 使用 libssh2 内置转发（无子进程）。
          </p>
          <div class="level-options">
            <button
              type="button"
              class="level-btn"
              :class="{ active: currentSshTunnelMode === 'ssh' }"
              :disabled="saving"
              @click="saveSshTunnelMode('ssh')"
            >
              ssh（子进程）
            </button>
            <button
              type="button"
              class="level-btn"
              :class="{ active: currentSshTunnelMode === 'builtin' }"
              :disabled="saving"
              @click="saveSshTunnelMode('builtin')"
            >
              builtin（内置）
            </button>
          </div>
          <p v-if="message" class="message" :class="{ error: message !== '已保存' }">
            <span v-if="message === '已保存'" class="message-icon">✓</span>
            {{ message }}
          </p>
        </section>
      </template>

      <!-- 安全与凭证 -->
      <template v-if="activeCategory === 'security'">
        <!-- 凭证存储后端 -->
        <section class="card">
          <h2 class="card-title">凭证存储后端</h2>
          <p class="card-desc">选择 SSH 密码的持久化方式。切换后端不会迁移已有凭证。</p>
          <div class="level-options">
            <button
              type="button"
              class="level-btn"
              :class="{ active: securityCfg.credential_store === 'stronghold' }"
              :disabled="securityLoading"
              @click="handleSetCredentialStore('stronghold')"
            >
              本地加密文件（默认）
            </button>
            <button
              type="button"
              class="level-btn"
              :class="{ active: securityCfg.credential_store === 'os_keychain' }"
              :disabled="securityLoading"
              @click="handleSetCredentialStore('os_keychain')"
            >
              系统钥匙串
            </button>
          </div>
          <p class="backend-desc">
            <template v-if="securityCfg.credential_store === 'stronghold'">
              使用 AES-256-GCM 加密的本地文件，主密码通过 Argon2id 派生密钥，跨平台无依赖。
            </template>
            <template v-else>
              使用操作系统凭证管理器（macOS Keychain / Windows Credential Manager / libsecret），无需主密码。
            </template>
          </p>
        </section>

        <!-- Stronghold 配置（仅 stronghold 模式显示） -->
        <template v-if="securityCfg.credential_store === 'stronghold'">
          <section class="card">
            <h2 class="card-title">
              Stronghold 加密文件
              <span
                class="status-badge"
                :class="{
                  'badge-init': strongholdStatus === 'uninitialized',
                  'badge-locked': strongholdStatus === 'locked',
                  'badge-unlocked': strongholdStatus === 'unlocked',
                }"
              >
                {{
                  strongholdStatus === "uninitialized"
                    ? "未初始化"
                    : strongholdStatus === "locked"
                    ? "已锁定"
                    : "已解锁"
                }}
              </span>
            </h2>

            <!-- 文件路径 -->
            <div class="path-row">
              <span class="path-label">快照路径</span>
              <template v-if="!editingStrongholdPath">
                <span class="path-value">
                  {{ securityCfg.stronghold_snapshot_path || "{app_data_dir}/credentials.hold（默认）" }}
                </span>
                <button
                  type="button"
                  class="link-btn"
                  @click="() => { editingStrongholdPath = true; tempStrongholdPath = securityCfg.stronghold_snapshot_path; }"
                >
                  修改
                </button>
              </template>
              <template v-else>
                <input
                  v-model="tempStrongholdPath"
                  class="path-input"
                  placeholder="留空使用默认路径"
                />
                <button type="button" class="link-btn" @click="handleSaveStrongholdPath">保存</button>
                <button type="button" class="link-btn" @click="editingStrongholdPath = false">取消</button>
              </template>
            </div>

            <!-- 自动锁定 -->
            <div class="option-group" style="margin-top: 1rem">
              <label class="option-label">自动锁定（分钟，0 = 运行期间不自动锁定）</label>
              <div class="option-buttons">
                <button
                  v-for="opt in [0, 15, 30, 60]"
                  :key="opt"
                  type="button"
                  class="level-btn"
                  :class="{ active: securityCfg.auto_lock_minutes === opt }"
                  @click="handleAutoLockChange(opt)"
                >
                  {{ opt === 0 ? "不锁定" : `${opt} 分钟` }}
                </button>
              </div>
            </div>

            <!-- 初始化表单 -->
            <template v-if="strongholdStatus === 'uninitialized'">
              <p class="card-desc" style="margin-top: 1rem">首次使用需设置主密码以创建加密存储。</p>
              <div class="master-form">
                <input
                  :type="showMasterPassword ? 'text' : 'password'"
                  v-model="masterPasswordInput"
                  class="master-input"
                  placeholder="主密码"
                />
                <input
                  :type="showMasterPassword ? 'text' : 'password'"
                  v-model="masterPasswordConfirm"
                  class="master-input"
                  placeholder="确认主密码"
                />
                <label class="checkbox-row">
                  <input type="checkbox" v-model="showMasterPassword" />
                  显示密码
                </label>
                <button
                  type="button"
                  class="btn-action"
                  :disabled="securityLoading"
                  @click="handleStrongholdInit"
                >
                  初始化 Stronghold
                </button>
              </div>
            </template>

            <!-- 解锁表单 -->
            <template v-else-if="strongholdStatus === 'locked'">
              <p class="card-desc" style="margin-top: 1rem">输入主密码解锁以管理凭证。</p>
              <div class="master-form">
                <input
                  :type="showMasterPassword ? 'text' : 'password'"
                  v-model="masterPasswordInput"
                  class="master-input"
                  placeholder="主密码"
                  @keydown.enter="handleStrongholdUnlock"
                />
                <label class="checkbox-row">
                  <input type="checkbox" v-model="showMasterPassword" />
                  显示密码
                </label>
                <button
                  type="button"
                  class="btn-action"
                  :disabled="securityLoading"
                  @click="handleStrongholdUnlock"
                >
                  解锁
                </button>
              </div>
            </template>

            <!-- 已解锁操作 -->
            <template v-else>
              <button
                type="button"
                class="btn-action btn-secondary-action"
                style="margin-top: 1rem"
                @click="handleStrongholdLock"
              >
                锁定 Stronghold
              </button>
            </template>

            <p v-if="securityMsg" class="message" :class="{ error: securityMsgIsError }">
              <span v-if="!securityMsgIsError" class="message-icon">✓</span>
              {{ securityMsg }}
            </p>
          </section>
        </template>

        <!-- 已保存凭证管理 -->
        <section class="card" v-if="strongholdStatus === 'unlocked' || securityCfg.credential_store === 'os_keychain'">
          <h2 class="card-title">已保存的凭证</h2>
          <p class="card-desc">以下为持久化后端中已保存的 SSH 隧道密码。</p>
          <template v-if="savedCredentials.length === 0">
            <p class="empty-tip">暂无已保存的凭证。</p>
          </template>
          <ul v-else class="credential-list">
            <li v-for="cred in savedCredentials" :key="cred.tunnel_id" class="credential-item">
              <span class="cred-name">{{ cred.tunnel_id }}</span>
              <span class="cred-store">{{ cred.store }}</span>
              <button
                type="button"
                class="cred-delete"
                :disabled="securityLoading"
                @click="handleDeleteCredential(cred.tunnel_id)"
              >
                删除
              </button>
            </li>
          </ul>
          <p v-if="securityMsg && activeCategory === 'security'" class="message" :class="{ error: securityMsgIsError }">
            <span v-if="!securityMsgIsError" class="message-icon">✓</span>
            {{ securityMsg }}
          </p>
        </section>

        <!-- OS 钥匙串说明（无法枚举） -->
        <section class="card" v-if="securityCfg.credential_store === 'os_keychain'">
          <h2 class="card-title">系统钥匙串</h2>
          <p class="card-desc">
            凭证存储于操作系统的凭证管理器中，无需主密码，由系统负责加密保护。<br />
            可通过各隧道配置页的「清除已保存的密码」按钮单独删除。
          </p>
          <p v-if="securityMsg" class="message" :class="{ error: securityMsgIsError }">
            <span v-if="!securityMsgIsError" class="message-icon">✓</span>
            {{ securityMsg }}
          </p>
        </section>
      </template>

      <!-- 外观 -->
      <template v-if="activeCategory === 'appearance'">
        <section class="card">
          <h2 class="card-title">代码主题</h2>
          <p class="card-desc">YAML 与编辑页的语法高亮主题。</p>
          <div class="theme-select-wrap">
            <select v-model="themeId" class="theme-select">
              <option v-for="t in YAML_THEMES" :key="t.id" :value="t.id">{{ t.label }}</option>
            </select>
          </div>
          <p class="card-desc preview-desc">示例 YAML 会实时应用当前主题，便于对比视觉效果。</p>
          <div class="yaml-preview-wrap">
            <CodeEditor
              :key="`yaml-theme-preview-${themeId}`"
              :value="yamlThemePreview"
              language="yaml"
              :theme="monacoTheme"
              :options="previewOptions"
              class="yaml-preview-editor"
            />
          </div>
        </section>
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
  background: #f8fafc;
}
.settings-nav {
  width: 180px;
  flex-shrink: 0;
  background: #fff;
  border-right: 1px solid #e2e8f0;
  padding: 1rem 0;
}
.nav-list {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}
.nav-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
  padding: 0.6rem 1rem;
  border: none;
  background: transparent;
  font-size: 0.9375rem;
  color: #64748b;
  cursor: pointer;
  text-align: left;
  transition: background 0.15s, color 0.15s;
}
.nav-item:hover {
  background: #f8fafc;
  color: #334155;
}
.nav-item.active {
  background: rgba(37, 99, 235, 0.08);
  color: #2563eb;
  font-weight: 500;
}
.nav-icon {
  font-size: 1.1em;
  line-height: 1;
}
.nav-label {
  flex: 1;
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
  color: #0f172a;
  letter-spacing: -0.02em;
}
.card {
  background: #fff;
  border-radius: 12px;
  padding: 1.5rem;
  margin-bottom: 1rem;
  max-width: 480px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
  border: 1px solid #e2e8f0;
}
.card-title {
  margin: 0 0 0.5rem;
  font-size: 1rem;
  font-weight: 600;
  color: #1e293b;
}
.card-desc {
  margin: 0 0 1.25rem;
  font-size: 0.875rem;
  color: #64748b;
  line-height: 1.55;
}
.level-options {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}
.level-btn {
  padding: 0.5rem 1rem;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  background: #fff;
  font-size: 0.875rem;
  cursor: pointer;
  color: #475569;
  transition: border-color 0.15s, background 0.15s, color 0.15s;
}
.level-btn:hover:not(:disabled) {
  background: #f8fafc;
  border-color: #cbd5e1;
  color: #334155;
}
.level-btn.active {
  background: rgba(37, 99, 235, 0.08);
  border-color: #2563eb;
  color: #2563eb;
  font-weight: 500;
}
.level-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.display-options {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  margin-bottom: 1rem;
}
.option-group {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
.option-label {
  font-size: 0.8125rem;
  font-weight: 500;
  color: #64748b;
}
.option-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}
.message {
  margin-top: 1rem;
  font-size: 0.875rem;
  color: #16a34a;
  display: flex;
  align-items: center;
  gap: 0.35rem;
}
.message-icon {
  font-weight: 600;
}
.message.error {
  color: #dc2626;
}
.setting-row {
  margin-top: 1rem;
  padding-top: 1rem;
  border-top: 1px solid #e2e8f0;
  display: flex;
  align-items: center;
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
  display: flex;
  align-items: center;
  gap: 0.6rem;
}
.setting-stack-wrap {
  flex-direction: column;
  align-items: stretch;
  min-width: 260px;
}
.setting-inline-actions {
  display: flex;
  gap: 0.6rem;
  flex-wrap: wrap;
}
.tag-list {
  display: flex;
  flex-wrap: wrap;
  gap: 0.45rem;
}
.hint-chip {
  display: inline-flex;
  align-items: center;
  padding: 0.25rem 0.55rem;
  border-radius: 999px;
  background: #e2e8f0;
  color: #334155;
  font-size: 0.75rem;
  line-height: 1.2;
}
.setting-text-input,
.setting-textarea {
  min-width: 260px;
  padding: 0.65rem 0.75rem;
  border: 1px solid #d1d5db;
  border-radius: 10px;
  font-size: 0.875rem;
  line-height: 1.5;
  color: #1e293b;
  background: #fff;
}
.setting-textarea {
  min-height: 96px;
  resize: vertical;
}
.setting-text-input {
  min-width: 0;
  width: 100%;
}
.gpu-rule-list {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}
.gpu-rule-row {
  display: grid;
  grid-template-columns: minmax(140px, 1fr) minmax(220px, 1.4fr) auto;
  gap: 0.6rem;
  align-items: center;
}
.btn-row-remove {
  padding: 0.55rem 0.75rem;
  border: 1px solid #fecaca;
  border-radius: 10px;
  background: #fff1f2;
  color: #be123c;
  font-size: 0.8125rem;
  cursor: pointer;
}
.btn-row-remove:hover {
  background: #ffe4e6;
}
.setting-textarea:focus {
  outline: none;
  border-color: #2563eb;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
}
.setting-text-input:focus {
  outline: none;
  border-color: #2563eb;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
}
.setting-hint-block {
  margin-top: 0.85rem;
  padding: 0.85rem 1rem;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  background: #f8fafc;
}
.setting-hint-title {
  font-size: 0.875rem;
  font-weight: 600;
  color: #1e293b;
}
.setting-hint-desc {
  margin-top: 0.25rem;
  font-size: 0.8125rem;
  line-height: 1.5;
  color: #64748b;
}
.setting-number-input {
  width: 88px;
  padding: 0.5rem 0.65rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  font-size: 0.875rem;
  color: #1e293b;
  background: #fff;
}
.setting-number-input:focus {
  outline: none;
  border-color: #2563eb;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
}
.theme-select-wrap {
  max-width: 240px;
}
.theme-select {
  width: 100%;
  padding: 0.5rem 2.5rem 0.5rem 0.75rem;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  font-size: 0.875rem;
  background: #fff url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%2364748b' d='M6 8L1 3h10z'/%3E%3C/svg%3E") no-repeat right 0.75rem center;
  cursor: pointer;
  appearance: none;
  transition: border-color 0.15s;
}
.theme-select:hover {
  border-color: #cbd5e1;
}
.theme-select:focus {
  outline: none;
  border-color: #2563eb;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
}
.preview-desc {
  margin-top: 1rem;
  margin-bottom: 0.75rem;
}
.yaml-preview-wrap {
  height: 300px;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  overflow: hidden;
}
.yaml-preview-editor {
  height: 100%;
}

/* 安全与凭证 */
.backend-desc {
  margin: 0.75rem 0 0;
  font-size: 0.8125rem;
  color: #94a3b8;
  line-height: 1.5;
}
.status-badge {
  display: inline-block;
  margin-left: 0.5rem;
  padding: 0.15rem 0.5rem;
  border-radius: 999px;
  font-size: 0.75rem;
  font-weight: 500;
  vertical-align: middle;
}
.badge-init {
  background: #f1f5f9;
  color: #64748b;
}
.badge-locked {
  background: #fff7ed;
  color: #c2410c;
}
.badge-unlocked {
  background: #f0fdf4;
  color: #15803d;
}
.path-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.875rem;
  flex-wrap: wrap;
}
.path-label {
  font-weight: 500;
  color: #475569;
  flex-shrink: 0;
}
.path-value {
  flex: 1;
  color: #64748b;
  font-family: monospace;
  font-size: 0.8125rem;
  word-break: break-all;
}
.path-input {
  flex: 1;
  min-width: 0;
  padding: 0.375rem 0.625rem;
  border: 1px solid #d1d5db;
  border-radius: 6px;
  font-size: 0.8125rem;
  font-family: monospace;
  outline: none;
}
.path-input:focus {
  border-color: #2563eb;
}
.link-btn {
  background: none;
  border: none;
  color: #2563eb;
  font-size: 0.8125rem;
  cursor: pointer;
  padding: 0.25rem 0.25rem;
  flex-shrink: 0;
}
.link-btn:hover {
  text-decoration: underline;
}
.master-form {
  display: flex;
  flex-direction: column;
  gap: 0.625rem;
}
.master-input {
  padding: 0.5rem 0.75rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  font-size: 0.9375rem;
  outline: none;
  transition: border-color 0.15s;
}
.master-input:focus {
  border-color: #2563eb;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
}
.checkbox-row {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.8125rem;
  color: #64748b;
  cursor: pointer;
}
.checkbox-card {
  padding: 0.85rem 0.95rem;
  border: 1px solid #dbe3ee;
  border-radius: 12px;
  background: #fff;
}
.node-strategy-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.85rem;
}
.sync-field {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  font-size: 0.8125rem;
  color: #475569;
}
.sync-field-wide {
  grid-column: 1 / -1;
}
.filter-input {
  width: 100%;
  min-width: 0;
  padding: 0.55rem 0.75rem;
  border: 1px solid #d1d5db;
  border-radius: 10px;
  background: #fff;
  font-size: 0.8125rem;
  color: #0f172a;
  outline: none;
}
.filter-input:focus {
  border-color: #2563eb;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
}
.strategy-textarea {
  width: 100%;
  resize: vertical;
  line-height: 1.55;
}
.strategy-tip {
  margin-top: 0.7rem;
  font-size: 0.75rem;
  color: #64748b;
}
.strategy-preview {
  margin-top: 0.8rem;
  padding: 0.85rem 0.95rem;
  border-radius: 12px;
  background: #f8fafc;
  color: #334155;
  font-size: 0.8125rem;
  line-height: 1.6;
  word-break: break-word;
}
.strategy-preview-empty {
  color: #64748b;
}
.btn-action {
  padding: 0.5rem 1.25rem;
  border: none;
  border-radius: 8px;
  background: #2563eb;
  color: #fff;
  font-size: 0.875rem;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.15s;
  align-self: flex-start;
}
.btn-action:hover:not(:disabled) {
  background: #1d4ed8;
}
.btn-action:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
@media (max-width: 900px) {
  .node-strategy-grid {
    grid-template-columns: 1fr;
  }
}
.btn-secondary-action {
  background: #f1f5f9;
  color: #475569;
}
.btn-secondary-action:hover:not(:disabled) {
  background: #e2e8f0;
}
.credential-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
.credential-item {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.5rem 0.75rem;
  background: #f8fafc;
  border-radius: 8px;
  border: 1px solid #e2e8f0;
  font-size: 0.875rem;
}
.cred-name {
  flex: 1;
  font-family: monospace;
  color: #1e293b;
  font-size: 0.8125rem;
}
.cred-store {
  color: #94a3b8;
  font-size: 0.75rem;
}
.cred-delete {
  background: none;
  border: none;
  color: #dc2626;
  font-size: 0.8125rem;
  cursor: pointer;
  padding: 0.2rem 0.4rem;
}
.cred-delete:hover:not(:disabled) {
  text-decoration: underline;
}
.cred-delete:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.empty-tip {
  font-size: 0.875rem;
  color: #94a3b8;
  margin: 0;
}
</style>
