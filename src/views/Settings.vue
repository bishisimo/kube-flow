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
import { useAppSettingsStore } from "../stores/appSettings";
import { useEnvStore } from "../stores/env";
import SettingsSecurityPanel from "../components/settings/SettingsSecurityPanel.vue";

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

      <!-- 工作流 -->
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
        <SettingsSecurityPanel />
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

<style src="./settings-view.css" scoped></style>
