<script setup lang="ts">
/**
 * MCP 设置：顶部全局开关；左侧环境列表，右侧编辑选中环境的可见性与权限。
 * 新启用环境默认 Readonly（只读），可在右侧单独调整档位与危险能力。
 */
import { computed, onMounted, ref, watch } from "vue";
import {
  NAlert,
  NButton,
  NCard,
  NCheckbox,
  NEmpty,
  NInput,
  NInputNumber,
  NPopconfirm,
  NSelect,
  NSwitch,
  NTabPane,
  NTabs,
  NTag,
  NTooltip,
} from "naive-ui";
import { envList, type Environment } from "../../api/env";
import {
  MCP_DANGER_CAPS,
  MCP_PRESET_OPTIONS,
  mcpAuditRecent,
  mcpDiagnose,
  mcpEnsureToken,
  mcpExportClientConfigs,
  mcpGetPolicy,
  mcpRegenerateToken,
  mcpResolveBinary,
  mcpServiceStart,
  mcpServiceStatus,
  mcpServiceStop,
  mcpSetPolicy,
  type McpAuditEntry,
  type McpClientExport,
  type McpDiagnoseResult,
  type McpEnvBinding,
  type McpPolicy,
  type McpPreset,
} from "../../api/mcp";
import { extractErrorMessage } from "../../utils/errorMessage";

const PRESET_HINT: Record<McpPreset, string> = {
  readonly: "list / get YAML / describe / logs",
  read_write: "只读 + apply / patch / 工作负载启停",
  full: "读写 + create / delete / pod exec / 文件",
  custom: "仅使用下方显式勾选的能力",
};

const DANGER_CAP_LABELS: Record<string, string> = {
  "resource.delete": "删除资源",
  "pod.exec": "Pod 执行命令",
  "pod.files": "Pod 文件传输",
};

const loading = ref(false);
const saving = ref(false);
const message = ref<string | null>(null);
const messageIsError = ref(false);
const gatewayRunning = ref(false);
const httpRunning = ref(false);
const httpUrl = ref<string | null>(null);
const environments = ref<Environment[]>([]);
const policy = ref<McpPolicy | null>(null);
const audit = ref<McpAuditEntry[]>([]);
const tokenPreview = ref("");
const binaryPath = ref("");
const mcpArgsPreview = ref("mcp");
const selectedEnvId = ref<string | null>(null);
const clientConfigs = ref<McpClientExport[]>([]);
const clientTab = ref("cursor");
const configsLoading = ref(false);
const rotatingToken = ref(false);
const diagnosing = ref(false);
const startingService = ref(false);
const stoppingService = ref(false);
const diagnose = ref<McpDiagnoseResult | null>(null);
const envFilterQuery = ref("");
const envFilterScope = ref<"all" | "enabled" | "disabled">("all");
const clientsExpanded = ref(true);
const auditExpanded = ref(false);

const ENV_SCOPE_OPTIONS = [
  { label: "全部", value: "all" },
  { label: "已开放", value: "enabled" },
  { label: "未开放", value: "disabled" },
];

function showMsg(text: string, isError = false) {
  message.value = text;
  messageIsError.value = isError;
  window.setTimeout(() => {
    message.value = null;
  }, 3500);
}

function defaultBinding(envId: string): McpEnvBinding {
  return {
    env_id: envId,
    enabled: false,
    preset: "readonly",
    capabilities: [],
    deny_capabilities: [],
    namespaces: [],
    kinds_allow: [],
    require_approval: true,
    allow_secrets: false,
  };
}

function ensureBinding(envId: string): McpEnvBinding {
  const p = policy.value;
  if (!p) return defaultBinding(envId);
  let found = p.bindings.find((b) => b.env_id === envId);
  if (!found) {
    found = defaultBinding(envId);
    p.bindings.push(found);
  }
  return found;
}

const selectedEnv = computed(
  () => environments.value.find((e) => e.id === selectedEnvId.value) ?? null
);

const selectedBinding = computed(() => {
  if (!selectedEnvId.value || !policy.value) return null;
  return ensureBinding(selectedEnvId.value);
});

const enabledCount = computed(
  () => policy.value?.bindings.filter((b) => b.enabled).length ?? 0
);

function isEnvEnabled(envId: string): boolean {
  return policy.value?.bindings.some((b) => b.env_id === envId && b.enabled) ?? false;
}

const filteredEnvironments = computed(() => {
  const q = envFilterQuery.value.trim().toLowerCase();
  const scope = envFilterScope.value;
  return environments.value.filter((env) => {
    const on = isEnvEnabled(env.id);
    if (scope === "enabled" && !on) return false;
    if (scope === "disabled" && on) return false;
    if (!q) return true;
    const hay = [
      env.display_name,
      env.id,
      env.current_context ?? "",
      ...(env.tags ?? []),
      env.source ?? "",
    ]
      .join(" ")
      .toLowerCase();
    return hay.includes(q);
  });
});

const tokenFingerprint = computed(() => {
  if (!tokenPreview.value) return "未生成";
  return `···· ${tokenPreview.value.slice(0, 8)}`;
});

function presetLabel(envId: string): string {
  const b = policy.value?.bindings.find((x) => x.env_id === envId);
  if (!b?.enabled) return "未开放";
  return MCP_PRESET_OPTIONS.find((o) => o.value === b.preset)?.label ?? b.preset;
}

function selectEnv(envId: string) {
  selectedEnvId.value = envId;
  ensureBinding(envId);
}

function setEnvEnabled(on: boolean) {
  const id = selectedEnvId.value;
  if (!id || !policy.value) return;
  const b = ensureBinding(id);
  const wasEnabled = b.enabled;
  b.enabled = on;
  if (on && !wasEnabled) {
    b.preset = "readonly";
    b.capabilities = [];
    b.deny_capabilities = [];
    b.require_approval = true;
    b.allow_secrets = false;
  }
}

function setPreset(preset: McpPreset) {
  const b = selectedBinding.value;
  if (!b) return;
  b.preset = preset;
  if (preset !== "custom") {
    b.capabilities = b.capabilities.filter((c) =>
      (MCP_DANGER_CAPS as readonly string[]).includes(c)
    );
  }
}

function setRequireApproval(on: boolean) {
  const b = selectedBinding.value;
  if (!b) return;
  b.require_approval = on;
}

function setAllowSecrets(on: boolean) {
  const b = selectedBinding.value;
  if (!b) return;
  b.allow_secrets = on;
}

function toggleDangerCap(cap: string, on: boolean) {
  const b = selectedBinding.value;
  if (!b) return;
  const deny = new Set(b.deny_capabilities);
  const extra = new Set(b.capabilities);
  if (on) {
    deny.delete(cap);
    if (b.preset !== "full" && b.preset !== "custom") {
      extra.add(cap);
    }
  } else {
    extra.delete(cap);
    if (b.preset === "full") {
      deny.add(cap);
    }
  }
  b.capabilities = [...extra];
  b.deny_capabilities = [...deny];
}

function hasDangerCap(cap: string): boolean {
  const b = selectedBinding.value;
  if (!b) return false;
  if (b.deny_capabilities.includes(cap)) return false;
  if (b.capabilities.includes(cap)) return true;
  return b.preset === "full";
}

const namespacesText = computed({
  get() {
    return selectedBinding.value?.namespaces.join(", ") ?? "";
  },
  set(text: string) {
    const b = selectedBinding.value;
    if (!b) return;
    b.namespaces = text
      .split(/[,，\s]+/)
      .map((s) => s.trim())
      .filter(Boolean);
  },
});

async function refresh() {
  loading.value = true;
  try {
    const [p, envs, service, token, recent] = await Promise.all([
      mcpGetPolicy(),
      envList(),
      mcpServiceStatus(),
      mcpEnsureToken().catch(() => ""),
      mcpAuditRecent(20).catch(() => [] as McpAuditEntry[]),
    ]);
    policy.value = p;
    environments.value = envs;
    applyServiceStatus(service);
    tokenPreview.value = token ? token.slice(0, 8) : "";
    audit.value = recent.slice().reverse();
    if (selectedEnvId.value && !envs.some((e) => e.id === selectedEnvId.value)) {
      selectedEnvId.value = null;
    }
    const [resolved] = await mcpResolveBinary(binaryPath.value.trim() || undefined).catch(
      () => ["kube-flow", false] as [string, boolean]
    );
    if (resolved) binaryPath.value = resolved;
    await loadClientConfigs();
    await runDiagnose();
  } catch (e) {
    showMsg(extractErrorMessage(e), true);
  } finally {
    loading.value = false;
  }
}

function applyServiceStatus(service: {
  gateway_running: boolean;
  http_running: boolean;
  http_url: string | null;
}) {
  gatewayRunning.value = service.gateway_running;
  httpRunning.value = service.http_running;
  httpUrl.value = service.http_url;
}

async function save() {
  if (!policy.value) return;
  policy.value.bindings = policy.value.bindings.filter(
    (b) => b.enabled || b.capabilities.length || b.deny_capabilities.length || b.namespaces.length
  );
  saving.value = true;
  try {
    await mcpSetPolicy(policy.value);
    applyServiceStatus(await mcpServiceStatus());
    await runDiagnose();
    showMsg("已保存");
  } catch (e) {
    showMsg(extractErrorMessage(e), true);
  } finally {
    saving.value = false;
  }
}

async function rotateToken() {
  rotatingToken.value = true;
  try {
    const t = await mcpRegenerateToken();
    tokenPreview.value = t.slice(0, 8);
    await loadClientConfigs();
    showMsg("已轮换访问令牌，请重新复制客户端配置");
  } catch (e) {
    showMsg(extractErrorMessage(e), true);
  } finally {
    rotatingToken.value = false;
  }
}

async function loadClientConfigs() {
  configsLoading.value = true;
  try {
    const cfg = await mcpExportClientConfigs(binaryPath.value.trim());
    clientConfigs.value = cfg.clients;
    binaryPath.value = cfg.command;
    mcpArgsPreview.value = (cfg.args ?? ["mcp"]).join(" ");
    tokenPreview.value = cfg.token ? cfg.token.slice(0, 8) : "";
    if (cfg.http_url) httpUrl.value = cfg.http_url;
    if (!cfg.clients.some((c) => c.id === clientTab.value) && cfg.clients[0]) {
      clientTab.value = cfg.clients[0].id;
    }
  } catch (e) {
    showMsg(extractErrorMessage(e), true);
  } finally {
    configsLoading.value = false;
  }
}

async function copyText(text: string, okMsg: string) {
  try {
    await navigator.clipboard.writeText(text);
    showMsg(okMsg);
  } catch (e) {
    showMsg(extractErrorMessage(e), true);
  }
}

function formatAuditTime(ts: string): string {
  try {
    const d = new Date(ts);
    if (Number.isNaN(d.getTime())) return ts;
    return d.toLocaleString(undefined, {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  } catch {
    return ts;
  }
}

const mcpReadyLabel = computed(() => {
  if (!diagnose.value) return "未检测";
  return diagnose.value.ready ? "就绪" : "未就绪";
});

const mcpReadyOk = computed(() => Boolean(diagnose.value?.ready));

async function runDiagnose() {
  diagnosing.value = true;
  try {
    const result = await mcpDiagnose(binaryPath.value.trim() || undefined);
    diagnose.value = result;
    gatewayRunning.value = result.gateway_running;
    httpRunning.value = result.http_running;
    httpUrl.value = result.http_url;
    if (result.binary_exists && result.binary_path) {
      binaryPath.value = result.binary_path;
    }
  } catch (e) {
    showMsg(extractErrorMessage(e), true);
  } finally {
    diagnosing.value = false;
  }
}

async function startService() {
  startingService.value = true;
  try {
    const status = await mcpServiceStart();
    applyServiceStatus(status);
    if (policy.value) {
      policy.value.mcp.enabled = true;
      policy.value.mcp.listen = true;
    }
    await loadClientConfigs();
    await runDiagnose();
    showMsg(status.http_url ? `MCP 服务已启动：${status.http_url}` : "MCP 服务已启动");
  } catch (e) {
    showMsg(extractErrorMessage(e), true);
  } finally {
    startingService.value = false;
  }
}

async function stopService() {
  stoppingService.value = true;
  try {
    const status = await mcpServiceStop();
    applyServiceStatus(status);
    if (policy.value) {
      policy.value.mcp.listen = false;
    }
    await runDiagnose();
    showMsg("MCP 服务已停止");
  } catch (e) {
    showMsg(extractErrorMessage(e), true);
  } finally {
    stoppingService.value = false;
  }
}

const activeClient = computed(
  () => clientConfigs.value.find((c) => c.id === clientTab.value) ?? null
);

const defaultHowto = [
  "1. 点击「启动 MCP 服务」（自动开启总开关、Gateway 与本机 HTTP）",
  "2. 在「按环境开放」中至少开放一个环境",
  "3. 在「客户端接入」复制 HTTP 配置到 Cursor / Claude Code / Codex",
  "4. 保持 kube-flow App 运行；写操作仍会在本机弹出确认",
];

watch(
  () => policy.value?.mcp.enabled,
  (enabled) => {
    if (enabled && policy.value) {
      policy.value.mcp.listen = true;
    }
  }
);

onMounted(() => {
  void refresh();
});
</script>

<template>
  <div class="mcp-panel">
    <NAlert
      v-if="message"
      class="mcp-toast"
      :type="messageIsError ? 'error' : 'success'"
      :title="message"
      :bordered="false"
    />

    <!-- 全局 -->
    <NCard size="small" class="mcp-card" :bordered="true" title="全局与状态">
      <div class="mcp-global">
        <div class="mcp-global-row">
          <div class="mcp-stat">
            <span class="mcp-stat-label">MCP 总开关</span>
            <NSwitch v-if="policy" v-model:value="policy.mcp.enabled" :disabled="loading" />
          </div>
          <div class="mcp-stat">
            <span class="mcp-stat-label">服务状态</span>
            <span
              class="mcp-status-dot"
              :class="mcpReadyOk ? 'mcp-status-dot--on' : 'mcp-status-dot--off'"
            />
            <span class="mcp-stat-value">{{ mcpReadyLabel }}</span>
          </div>
          <div class="mcp-stat">
            <span class="mcp-stat-label">HTTP MCP</span>
            <span
              class="mcp-status-dot"
              :class="httpRunning ? 'mcp-status-dot--on' : 'mcp-status-dot--off'"
            />
            <span class="mcp-stat-value">{{ httpRunning ? "运行中" : "未运行" }}</span>
          </div>
          <div class="mcp-stat">
            <span class="mcp-stat-label">Gateway</span>
            <span
              class="mcp-status-dot"
              :class="gatewayRunning ? 'mcp-status-dot--on' : 'mcp-status-dot--off'"
            />
            <span class="mcp-stat-value">{{ gatewayRunning ? "运行中" : "未运行" }}</span>
          </div>
          <div class="mcp-stat">
            <span class="mcp-stat-label">已开放环境</span>
            <span class="mcp-stat-value">{{ enabledCount }}</span>
          </div>
          <div class="mcp-stat">
            <span class="mcp-stat-label">SessionGrant</span>
            <NInputNumber
              v-if="policy"
              v-model:value="policy.mcp.session_grant_minutes"
              :min="1"
              :max="240"
              size="small"
              class="mcp-grant-input"
            />
            <span class="mcp-stat-unit">分钟</span>
          </div>
          <div class="mcp-global-actions">
            <NButton
              size="small"
              secondary
              :loading="diagnosing"
              :disabled="loading"
              @click="runDiagnose"
            >
              检测状态
            </NButton>
            <NButton
              v-if="!httpRunning"
              size="small"
              type="primary"
              :loading="startingService"
              :disabled="loading"
              @click="startService"
            >
              启动 MCP 服务
            </NButton>
            <NButton
              v-else
              size="small"
              secondary
              :loading="stoppingService"
              :disabled="loading"
              @click="stopService"
            >
              停止 MCP 服务
            </NButton>
            <NButton quaternary size="small" :disabled="loading" @click="refresh">刷新</NButton>
            <NButton type="primary" size="small" :loading="saving" :disabled="!policy" @click="save">
              保存策略
            </NButton>
          </div>
        </div>

        <div v-if="httpUrl" class="mcp-url-bar">
          <span class="mcp-field-label">服务地址</span>
          <code class="mcp-url">{{ httpUrl }}</code>
          <NButton
            size="tiny"
            quaternary
            @click="copyText(httpUrl!, '已复制服务地址')"
          >
            复制
          </NButton>
        </div>

        <div v-if="diagnose" class="mcp-health">
          <div
            v-for="item in diagnose.checks"
            :key="item.id"
            class="mcp-health-item"
            :class="item.ok ? 'mcp-health-item--ok' : 'mcp-health-item--bad'"
          >
            <span class="mcp-health-mark">{{ item.ok ? "✓" : "!" }}</span>
            <div class="mcp-health-body">
              <div class="mcp-health-label">{{ item.label }}</div>
              <div class="mcp-health-detail">{{ item.detail }}</div>
            </div>
          </div>
        </div>

        <div class="mcp-howto">
          <div class="mcp-field-label">如何运行 MCP</div>
          <ol class="mcp-howto-list">
            <li v-for="(line, i) in diagnose?.howto ?? defaultHowto" :key="i">{{ line }}</li>
          </ol>
          <p class="mcp-desc">
            <strong>说明：</strong>在设置中启动后，本 App 会在本机提供 HTTP MCP。
            客户端只需连接上方地址；写操作仍经 App Gateway，并在本机确认。
            stdio（<code>{{ binaryPath || "kube-flow" }} mcp</code>）仍可作为备选。
          </p>
        </div>
      </div>
    </NCard>

    <!-- 按环境开放 -->
    <NCard size="small" class="mcp-card" :bordered="true" title="按环境开放">
      <div class="mcp-split">
        <aside class="mcp-split-left">
          <div class="mcp-left-filter">
            <NInput
              v-model:value="envFilterQuery"
              size="small"
              clearable
              placeholder="搜索名称、标签、context…"
            />
            <NSelect
              v-model:value="envFilterScope"
              size="small"
              :options="ENV_SCOPE_OPTIONS"
            />
            <div class="mcp-left-filter-meta">
              {{ filteredEnvironments.length }} / {{ environments.length }}
            </div>
          </div>
          <div class="mcp-env-list">
            <div v-if="!environments.length" class="mcp-empty">
              <NEmpty description="暂无环境" size="small" />
            </div>
            <div v-else-if="!filteredEnvironments.length" class="mcp-empty">
              <NEmpty description="没有匹配的环境" size="small" />
            </div>
            <button
              v-for="env in filteredEnvironments"
              :key="env.id"
              type="button"
              class="mcp-env-item"
              :class="{
                'mcp-env-item--active': env.id === selectedEnvId,
                'mcp-env-item--on': isEnvEnabled(env.id),
              }"
              @click="selectEnv(env.id)"
            >
              <span
                class="mcp-env-dot"
                :class="isEnvEnabled(env.id) ? 'mcp-env-dot--on' : 'mcp-env-dot--off'"
              />
              <span class="mcp-env-item-body">
                <span class="mcp-env-item-title">{{ env.display_name }}</span>
                <span class="mcp-env-item-sub">{{ presetLabel(env.id) }}</span>
              </span>
            </button>
          </div>
        </aside>

        <section class="mcp-split-right">
          <template v-if="selectedEnv && selectedBinding">
            <header class="mcp-right-header">
              <div class="mcp-right-heading">
                <h3 class="mcp-right-title">{{ selectedEnv.display_name }}</h3>
                <code class="mcp-id">{{ selectedEnv.id }}</code>
              </div>
              <label class="mcp-enable-toggle">
                <span>开放 MCP</span>
                <NSwitch
                  :value="selectedBinding.enabled"
                  :disabled="!policy?.mcp.enabled"
                  @update:value="setEnvEnabled"
                />
              </label>
            </header>

            <NAlert
              v-if="!policy?.mcp.enabled"
              type="warning"
              :bordered="false"
              title="请先打开上方 MCP 总开关"
              class="mcp-inline-alert"
            />

            <div class="mcp-right-body" :class="{ 'mcp-right-body--dim': !selectedBinding.enabled }">
              <div class="mcp-field">
                <div class="mcp-field-label">权限档位</div>
                <NSelect
                  :value="selectedBinding.preset"
                  :options="MCP_PRESET_OPTIONS"
                  :disabled="!selectedBinding.enabled"
                  class="mcp-preset-select"
                  @update:value="(v) => setPreset(v as McpPreset)"
                />
                <p class="mcp-field-hint">{{ PRESET_HINT[selectedBinding.preset] }}</p>
              </div>

              <div class="mcp-field">
                <div class="mcp-field-label">Namespaces</div>
                <NInput
                  v-model:value="namespacesText"
                  size="small"
                  :disabled="!selectedBinding.enabled"
                  placeholder="空 = 全部；支持 app-*"
                />
              </div>

              <div class="mcp-checks">
                <NCheckbox
                  :checked="selectedBinding.require_approval"
                  :disabled="!selectedBinding.enabled"
                  @update:checked="setRequireApproval"
                >
                  写操作需确认
                </NCheckbox>
                <NCheckbox
                  :checked="selectedBinding.allow_secrets"
                  :disabled="!selectedBinding.enabled"
                  @update:checked="setAllowSecrets"
                >
                  允许 Secret
                </NCheckbox>
              </div>

              <div class="mcp-field">
                <div class="mcp-field-label">危险能力</div>
                <div class="mcp-danger-grid">
                  <label
                    v-for="cap in MCP_DANGER_CAPS"
                    :key="cap"
                    class="mcp-danger-item"
                    :class="{ 'mcp-danger-item--on': hasDangerCap(cap) }"
                  >
                    <NCheckbox
                      :checked="hasDangerCap(cap)"
                      :disabled="!selectedBinding.enabled"
                      @update:checked="(v) => toggleDangerCap(cap, v)"
                    />
                    <span>
                      <span class="mcp-danger-name">{{ DANGER_CAP_LABELS[cap] ?? cap }}</span>
                      <span class="mcp-danger-code">{{ cap }}</span>
                    </span>
                  </label>
                </div>
                <p class="mcp-field-hint">每次强制确认，不使用 SessionGrant。</p>
              </div>
            </div>
          </template>
          <div v-else class="mcp-empty mcp-empty--center">
            <NEmpty description="请从左侧选择环境后再配置权限" />
          </div>
        </section>
      </div>
    </NCard>

    <!-- 客户端接入 -->
    <NCard size="small" class="mcp-card" :bordered="true">
      <template #header>
        <button type="button" class="mcp-card-toggle" @click="clientsExpanded = !clientsExpanded">
          <span>客户端接入</span>
          <span class="mcp-card-toggle-hint">Cursor · Claude Code · Codex</span>
          <span class="mcp-chevron" :class="{ 'mcp-chevron--open': clientsExpanded }">›</span>
        </button>
      </template>

      <div v-show="clientsExpanded" class="mcp-clients">
        <div class="mcp-token-bar">
          <div class="mcp-token-info">
            <div class="mcp-field-label">访问令牌</div>
            <NTooltip trigger="hover">
              <template #trigger>
                <code class="mcp-token-fp">{{ tokenFingerprint }}</code>
              </template>
              仅显示指纹，完整令牌写入各客户端配置的环境变量中
            </NTooltip>
            <span class="mcp-token-note">客户端连接 Gateway 时校验；轮换后旧配置失效</span>
          </div>
          <NPopconfirm @positive-click="rotateToken">
            <template #trigger>
              <NButton size="small" secondary :loading="rotatingToken">轮换令牌</NButton>
            </template>
            轮换后需重新复制并更新各客户端配置，确定继续？
          </NPopconfirm>
        </div>

        <div class="mcp-path-row">
          <div class="mcp-field-label">接入方式</div>
          <div class="mcp-path-controls">
            <NInput
              :value="httpUrl || '启动 MCP 服务后显示地址'"
              size="small"
              readonly
              placeholder="HTTP 服务地址"
            />
            <NButton size="small" :loading="configsLoading" @click="loadClientConfigs">
              刷新配置
            </NButton>
          </div>
          <p class="mcp-field-hint">
            推荐 HTTP：复制下方配置即可。stdio 备选：
            <code>{{ binaryPath || "…" }} mcp</code>
          </p>
        </div>

        <NTabs v-if="clientConfigs.length" v-model:value="clientTab" type="segment" size="small">
          <NTabPane
            v-for="client in clientConfigs"
            :key="client.id"
            :name="client.id"
            :tab="client.label"
          >
            <div class="mcp-client-pane">
              <div class="mcp-client-meta">
                <span class="mcp-path-badge">写入位置：{{ client.config_path_hint }}</span>
                <NTag size="tiny" :bordered="false">{{ client.format.toUpperCase() }}</NTag>
              </div>

              <section class="mcp-export-block">
                <header class="mcp-export-header">
                  <div class="mcp-export-intro">
                    <h4 class="mcp-export-title">配置文件内容</h4>
                    <p class="mcp-export-desc">
                      复制后粘贴到客户端配置文件（见上方写入位置）。含 HTTP 地址与访问令牌。
                    </p>
                  </div>
                  <NButton
                    size="small"
                    type="primary"
                    secondary
                    class="mcp-export-copy"
                    @click="
                      copyText(
                        client.snippet,
                        `已复制配置文件内容，请粘贴到 ${client.config_path_hint}`
                      )
                    "
                  >
                    复制配置文件
                  </NButton>
                </header>
                <pre class="mcp-code">{{ client.snippet }}</pre>
              </section>

              <section v-if="client.cli_hint" class="mcp-export-block">
                <header class="mcp-export-header">
                  <div class="mcp-export-intro">
                    <h4 class="mcp-export-title">终端安装命令</h4>
                    <p class="mcp-export-desc">
                      在终端执行此命令，由客户端 CLI 自动注册 MCP（无需手改配置文件）。
                    </p>
                  </div>
                  <NButton
                    size="small"
                    secondary
                    class="mcp-export-copy"
                    @click="copyText(client.cli_hint!, '已复制终端安装命令')"
                  >
                    复制安装命令
                  </NButton>
                </header>
                <pre class="mcp-code mcp-code--cli">{{ client.cli_hint }}</pre>
              </section>
            </div>
          </NTabPane>
        </NTabs>
        <NEmpty v-else description="点击「刷新配置」生成各客户端片段" size="small" />

        <p v-if="activeClient" class="mcp-desc">
          合并进对应配置文件后重启客户端。需保持本机 kube-flow App 的 MCP 服务在线。
        </p>
      </div>
    </NCard>

    <!-- 审计 -->
    <NCard size="small" class="mcp-card" :bordered="true">
      <template #header>
        <button type="button" class="mcp-card-toggle" @click="auditExpanded = !auditExpanded">
          <span>最近审计</span>
          <span class="mcp-card-toggle-hint">{{ audit.length }} 条</span>
          <span class="mcp-chevron" :class="{ 'mcp-chevron--open': auditExpanded }">›</span>
        </button>
      </template>
      <div v-show="auditExpanded" class="mcp-audit">
        <div v-if="!audit.length" class="mcp-muted">暂无记录</div>
        <div v-for="(row, i) in audit" :key="i" class="mcp-audit-row">
          <NTag size="tiny" :type="row.ok ? 'success' : 'error'" :bordered="false">
            {{ row.ok ? "成功" : "失败" }}
          </NTag>
          <span class="mcp-audit-time">{{ formatAuditTime(row.ts) }}</span>
          <span class="mcp-audit-tool">{{ row.tool }}</span>
          <span class="mcp-audit-cap">{{ row.capability }}</span>
          <span v-if="row.env_id" class="mcp-audit-env">{{ row.env_id.slice(0, 8) }}</span>
          <span v-if="row.error" class="mcp-audit-err">{{ row.error }}</span>
        </div>
      </div>
    </NCard>
  </div>
</template>

<style scoped>
.mcp-panel {
  display: flex;
  flex-direction: column;
  gap: 14px;
  width: 100%;
  max-width: 1080px;
}

.mcp-card {
  max-width: none;
}

.mcp-toast {
  margin-bottom: 0;
}

.mcp-howto {
  margin-top: 14px;
  padding-top: 12px;
  border-top: 1px solid var(--kf-border, rgba(148, 163, 184, 0.18));
}

.mcp-url-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 12px;
  flex-wrap: wrap;
}

.mcp-url {
  font-size: 12px;
  padding: 4px 8px;
  border-radius: 6px;
  background: var(--kf-surface-2, rgba(148, 163, 184, 0.12));
  color: var(--kf-text, #1f2a37);
}

.mcp-howto-list {
  margin: 6px 0 0;
  padding-left: 1.25rem;
  font-size: 12px;
  line-height: 1.55;
  color: var(--kf-text-secondary, #66768f);
}

.mcp-howto-list li {
  margin-bottom: 4px;
}

.mcp-health {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 8px;
  margin-top: 14px;
}

.mcp-health-item {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid var(--kf-border, rgba(148, 163, 184, 0.22));
  background: var(--kf-surface-strong, #fff);
}

.mcp-health-item--ok {
  border-color: color-mix(in srgb, var(--kf-success, #16a34a) 35%, var(--kf-border));
  background: color-mix(in srgb, var(--kf-success, #16a34a) 7%, var(--kf-surface-strong, #fff));
}

.mcp-health-item--bad {
  border-color: color-mix(in srgb, var(--kf-warning, #d97706) 40%, var(--kf-border));
  background: color-mix(in srgb, var(--kf-warning, #d97706) 8%, var(--kf-surface-strong, #fff));
}

.mcp-health-mark {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 700;
  flex-shrink: 0;
  margin-top: 1px;
}

.mcp-health-item--ok .mcp-health-mark {
  color: var(--kf-success, #16a34a);
  background: color-mix(in srgb, var(--kf-success, #16a34a) 16%, transparent);
}

.mcp-health-item--bad .mcp-health-mark {
  color: var(--kf-warning, #d97706);
  background: color-mix(in srgb, var(--kf-warning, #d97706) 16%, transparent);
}

.mcp-health-label {
  font-size: 13px;
  font-weight: 650;
}

.mcp-health-detail {
  margin-top: 2px;
  font-size: 12px;
  color: var(--kf-text-secondary, #66768f);
  line-height: 1.4;
  word-break: break-word;
}

.mcp-muted {
  font-size: 12px;
  color: var(--kf-text-muted, #8a98ac);
}

.mcp-global-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 16px 20px;
}

.mcp-stat {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-height: 28px;
}

.mcp-stat-label {
  font-size: 12px;
  color: var(--kf-text-secondary, #66768f);
}

.mcp-stat-value {
  font-size: 13px;
  font-weight: 600;
  color: var(--kf-text-primary, #0f172a);
}

.mcp-stat-unit {
  font-size: 12px;
  color: var(--kf-text-muted, #8a98ac);
}

.mcp-grant-input {
  width: 84px;
}

.mcp-global-actions {
  margin-left: auto;
  display: flex;
  gap: 8px;
}

.mcp-status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.mcp-status-dot--on {
  background: var(--kf-success, #16a34a);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--kf-success, #16a34a) 22%, transparent);
}

.mcp-status-dot--off {
  background: var(--kf-text-muted, #8a98ac);
}

.mcp-split {
  display: grid;
  grid-template-columns: minmax(220px, 280px) 1fr;
  min-height: 400px;
  border: 1px solid var(--kf-border, rgba(148, 163, 184, 0.24));
  border-radius: 12px;
  overflow: hidden;
  background: var(--kf-surface-strong, #fff);
}

.mcp-split-left {
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--kf-border, rgba(148, 163, 184, 0.24));
  background: color-mix(in srgb, var(--kf-bg-elevated, #eef3fa) 88%, transparent);
  max-height: 520px;
}

.mcp-left-filter {
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  border-bottom: 1px solid var(--kf-border, rgba(148, 163, 184, 0.18));
}

.mcp-left-filter-meta {
  font-size: 11px;
  color: var(--kf-text-muted, #8a98ac);
}

.mcp-env-list {
  overflow: auto;
  flex: 1;
}

.mcp-env-item {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  width: 100%;
  border: 0;
  border-bottom: 1px solid var(--kf-border, rgba(148, 163, 184, 0.12));
  background: transparent;
  padding: 11px 12px;
  cursor: pointer;
  color: inherit;
  text-align: left;
}

.mcp-env-item:hover {
  background: color-mix(in srgb, var(--kf-primary, #2563eb) 7%, transparent);
}

.mcp-env-item--active {
  background: color-mix(in srgb, var(--kf-primary, #2563eb) 12%, transparent);
  box-shadow: inset 3px 0 0 var(--kf-primary, #2563eb);
}

.mcp-env-dot {
  width: 8px;
  height: 8px;
  margin-top: 5px;
  border-radius: 50%;
  flex-shrink: 0;
}

.mcp-env-dot--on {
  background: var(--kf-success, #16a34a);
}

.mcp-env-dot--off {
  background: color-mix(in srgb, var(--kf-text-muted, #8a98ac) 55%, transparent);
}

.mcp-env-item-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.mcp-env-item-title {
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mcp-env-item-sub {
  font-size: 11px;
  color: var(--kf-text-muted, #8a98ac);
}

.mcp-split-right {
  padding: 16px 18px;
  overflow: auto;
  max-height: 520px;
}

.mcp-right-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--kf-border, rgba(148, 163, 184, 0.18));
}

.mcp-right-title {
  margin: 0 0 4px;
  font-size: 16px;
  font-weight: 650;
}

.mcp-id {
  font-size: 11px;
  color: var(--kf-text-muted, #8a98ac);
  background: color-mix(in srgb, var(--kf-bg-elevated, #eef3fa) 80%, transparent);
  padding: 2px 6px;
  border-radius: 4px;
}

.mcp-enable-toggle {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  white-space: nowrap;
}

.mcp-inline-alert {
  margin-bottom: 12px;
}

.mcp-right-body {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.mcp-right-body--dim {
  opacity: 0.42;
  pointer-events: none;
}

.mcp-field-label {
  font-size: 12px;
  font-weight: 600;
  margin-bottom: 6px;
  color: var(--kf-text-secondary, #66768f);
}

.mcp-preset-select {
  max-width: 220px;
}

.mcp-checks {
  display: flex;
  flex-wrap: wrap;
  gap: 14px 18px;
}

.mcp-danger-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 8px;
}

.mcp-danger-item {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 10px 12px;
  border-radius: 8px;
  border: 1px solid var(--kf-border, rgba(148, 163, 184, 0.22));
  cursor: pointer;
  background: var(--kf-surface-strong, #fff);
}

.mcp-danger-item--on {
  border-color: color-mix(in srgb, var(--kf-danger, #dc2626) 35%, var(--kf-border));
  background: color-mix(in srgb, var(--kf-danger, #dc2626) 6%, var(--kf-surface-strong, #fff));
}

.mcp-danger-name {
  display: block;
  font-size: 13px;
  font-weight: 600;
}

.mcp-danger-code {
  display: block;
  font-size: 11px;
  color: var(--kf-text-muted, #8a98ac);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}

.mcp-card-toggle {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  border: 0;
  background: transparent;
  padding: 0;
  cursor: pointer;
  color: inherit;
  font: inherit;
  text-align: left;
}

.mcp-card-toggle-hint {
  font-size: 12px;
  font-weight: 400;
  color: var(--kf-text-muted, #8a98ac);
}

.mcp-chevron {
  margin-left: auto;
  display: inline-block;
  transform: rotate(90deg);
  transition: transform 0.15s ease;
  color: var(--kf-text-muted, #8a98ac);
  font-size: 18px;
  line-height: 1;
}

.mcp-chevron--open {
  transform: rotate(-90deg);
}

.mcp-clients {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.mcp-token-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 14px;
  border-radius: 10px;
  background: color-mix(in srgb, var(--kf-bg-elevated, #eef3fa) 90%, transparent);
  border: 1px solid var(--kf-border, rgba(148, 163, 184, 0.18));
}

.mcp-token-info {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 8px 12px;
  min-width: 0;
}

.mcp-token-fp {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 13px;
  font-weight: 600;
  letter-spacing: 0.02em;
  padding: 2px 8px;
  border-radius: 6px;
  background: var(--kf-surface-strong, #fff);
  border: 1px solid var(--kf-border, rgba(148, 163, 184, 0.22));
}

.mcp-token-note {
  font-size: 12px;
  color: var(--kf-text-muted, #8a98ac);
}

.mcp-path-row {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.mcp-path-controls {
  display: grid;
  grid-template-columns: 1fr 72px auto;
  gap: 8px;
}

.mcp-args-input {
  max-width: 72px;
}

.mcp-client-pane {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding-top: 12px;
}

.mcp-client-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.mcp-path-badge {
  font-size: 12px;
  color: var(--kf-text-secondary, #66768f);
  background: var(--kf-bg-elevated, #eef3fa);
  border: 1px solid var(--kf-border, rgba(148, 163, 184, 0.22));
  border-radius: 999px;
  padding: 3px 10px;
}

.mcp-export-block {
  border: 1px solid var(--kf-border, rgba(148, 163, 184, 0.24));
  border-radius: 12px;
  overflow: hidden;
  background: var(--kf-surface-strong, #ffffff);
}

.mcp-export-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding: 12px 14px;
  background: var(--kf-bg-elevated, #eef3fa);
  border-bottom: 1px solid var(--kf-border, rgba(148, 163, 184, 0.2));
}

.mcp-export-intro {
  min-width: 0;
  flex: 1;
}

.mcp-export-title {
  margin: 0 0 4px;
  font-size: 13px;
  font-weight: 650;
  color: var(--kf-text-primary, #0f172a);
}

.mcp-export-desc {
  margin: 0;
  font-size: 12px;
  line-height: 1.45;
  color: var(--kf-text-secondary, #66768f);
}

.mcp-export-copy {
  flex-shrink: 0;
  align-self: center;
}

.mcp-code {
  margin: 0;
  padding: 12px 14px;
  max-height: 240px;
  overflow: auto;
  font-size: 12px;
  line-height: 1.55;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  color: var(--kf-text-primary, #0f172a);
  background: var(--kf-surface-strong, #ffffff);
  white-space: pre-wrap;
  word-break: break-word;
}

.mcp-code--cli {
  max-height: 96px;
}

:root[data-kf-chrome="dark"] .mcp-export-block {
  background: var(--kf-surface-strong, #1e293b);
}

:root[data-kf-chrome="dark"] .mcp-export-header {
  background: color-mix(in srgb, var(--kf-bg-elevated, #1e293b) 88%, #000);
}

:root[data-kf-chrome="dark"] .mcp-code {
  background: var(--kf-surface-strong, #1e293b);
  color: var(--kf-text-primary, #f1f5f9);
}

.mcp-audit {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.mcp-audit-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px 10px;
  padding: 8px 10px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--kf-bg-elevated, #eef3fa) 70%, transparent);
  font-size: 12px;
}

.mcp-audit-time {
  color: var(--kf-text-muted, #8a98ac);
  font-variant-numeric: tabular-nums;
}

.mcp-audit-tool {
  font-weight: 600;
}

.mcp-audit-cap,
.mcp-audit-env {
  color: var(--kf-text-secondary, #66768f);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}

.mcp-audit-err {
  color: var(--kf-danger, #dc2626);
  flex: 1 1 100%;
}

.mcp-empty {
  padding: 20px 12px;
}

.mcp-empty--center {
  min-height: 240px;
  display: flex;
  align-items: center;
  justify-content: center;
}

@media (max-width: 760px) {
  .mcp-split {
    grid-template-columns: 1fr;
  }

  .mcp-split-left {
    max-height: 220px;
    border-right: 0;
    border-bottom: 1px solid var(--kf-border, rgba(148, 163, 184, 0.24));
  }

  .mcp-global-actions {
    margin-left: 0;
    width: 100%;
    justify-content: flex-end;
  }

  .mcp-path-controls {
    grid-template-columns: 1fr;
  }

  .mcp-args-input {
    max-width: none;
  }

  .mcp-token-bar {
    flex-direction: column;
    align-items: stretch;
  }

  .mcp-export-header {
    flex-direction: column;
    align-items: stretch;
  }

  .mcp-export-copy {
    align-self: flex-end;
  }
}
</style>
