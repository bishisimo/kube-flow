<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted } from "vue";
import { NBadge, NButton, NCard, NScrollbar, NTag, NTooltip } from "naive-ui";
import { useEnvStore } from "../stores/env";
import { useConnectionStore } from "../stores/connection";
import { kubeGetTunnelLocalPort } from "../api/kube";
import { envListSshTunnels } from "../api/env";
import type { SshTunnel } from "../api/env";
import { effectiveContext } from "../api/env";
import { buildCompactRailItems } from "../utils/compactRail";

const props = defineProps<{
  /** 透传自 NLayoutSider，用于内部文案随折叠态隐藏；折叠动画由 sider 驱动 */
  collapsed: boolean;
  onReconnect?: (envId: string) => void;
  onOpenTerminal?: (envId: string) => void;
}>();
const emit = defineEmits<{
  (e: "toggle-collapsed"): void;
}>();

const {
  openedEnvs,
  currentId,
  setCurrent,
  closeEnv,
} = useEnvStore();
const { getState, getError } = useConnectionStore();

const hasOpened = computed(() => openedEnvs.value.length > 0);
const compactEnvItems = computed(() =>
  buildCompactRailItems(
    openedEnvs.value.map((env) => ({
      id: env.id,
      label: env.display_name,
      context: effectiveContext(env as never) ?? "",
      fallback: "Env",
    }))
  )
);

/** SSH 隧道 id -> 隧道配置（含 ssh_host） */
const tunnelsById = ref<Record<string, SshTunnel>>({});

/** SSH 隧道环境的 env_id -> 本地映射端口 */
const tunnelPorts = ref<Record<string, number>>({});

async function loadTunnels() {
  const list = await envListSshTunnels();
  const map: Record<string, SshTunnel> = {};
  for (const t of list) {
    map[t.id] = t;
  }
  tunnelsById.value = map;
}

async function refreshTunnelPorts() {
  const sshEnvs = openedEnvs.value.filter((e) => e.source === "ssh_tunnel");
  for (const e of sshEnvs) {
    const port = await kubeGetTunnelLocalPort(e.id);
    if (port != null) {
      tunnelPorts.value = { ...tunnelPorts.value, [e.id]: port };
    }
  }
}

let pollTimer: ReturnType<typeof setInterval> | null = null;

function startTunnelPortPolling() {
  if (pollTimer) clearInterval(pollTimer);
  let count = 0;
  const maxAttempts = 5;
  pollTimer = setInterval(async () => {
    await refreshTunnelPorts();
    count++;
    const allHavePorts = openedEnvs.value
      .filter((e) => e.source === "ssh_tunnel")
      .every((e) => tunnelPorts.value[e.id]);
    if (allHavePorts || count >= maxAttempts) {
      if (pollTimer) clearInterval(pollTimer);
      pollTimer = null;
    }
  }, 2000);
}

function scheduleRefresh() {
  loadTunnels();
  refreshTunnelPorts();
  const hasSshWithoutPort = openedEnvs.value.some(
    (e) => e.source === "ssh_tunnel" && !tunnelPorts.value[e.id]
  );
  if (hasSshWithoutPort) startTunnelPortPolling();
}

/** 某环境的连接状态用于展示：断开 | 连接中 | 就绪 */
function statusLabel(envId: string): string {
  const s = getState(envId);
  if (s === "disconnected") return "断开";
  if (s === "connecting") return "连接中";
  return "就绪";
}

function sourceLabel(source: string): string {
  return source === "ssh_tunnel" ? "SSH" : "本地";
}

function contextLabel(env: { current_context?: string | null; contexts: Array<{ context_name: string }> }): string {
  return effectiveContext(env as never) ?? "未设置 Context";
}

function remoteKubeconfigLabel(env: { ssh_tunnel_id?: string | null }): string {
  if (!env.ssh_tunnel_id) return "未设置远程 kubeconfig";
  return tunnelsById.value[env.ssh_tunnel_id]?.remote_kubeconfig_path ?? "未设置远程 kubeconfig";
}

/** 某环境 hover 详情：状态、SSH 的 localPort/remoteHost、断开时的错误信息 */
function hoverLines(e: { id: string; source: string; ssh_tunnel_id?: string | null }): string[] {
  const lines: string[] = [];
  lines.push(`状态: ${statusLabel(e.id)}`);
  lines.push(`来源: ${sourceLabel(e.source)}`);
  if (e.source === "ssh_tunnel") {
    const port = tunnelPorts.value[e.id];
    const tunnel = e.ssh_tunnel_id ? tunnelsById.value[e.ssh_tunnel_id] : null;
    if (port != null) lines.push(`localPort: ${port}`);
    if (tunnel?.ssh_host) lines.push(`remoteHost: ${tunnel.ssh_host}`);
  }
  const err = getError(e.id);
  if (err) lines.push(`错误: ${err}`);
  return lines;
}

/** 状态点配色：connected/error 绿色；connecting 蓝色（带 processing 动画）；disconnected 红色。 */
function statusBadgeType(envId: string): "success" | "info" | "error" {
  const s = getState(envId);
  if (s === "disconnected") return "error";
  if (s === "connecting") return "info";
  return "success";
}

function statusIsConnecting(envId: string): boolean {
  return getState(envId) === "connecting";
}

watch([openedEnvs, currentId], scheduleRefresh);
onMounted(scheduleRefresh);
onUnmounted(() => {
  if (pollTimer) clearInterval(pollTimer);
});
</script>

<template>
  <aside class="env-bar">
    <div class="header">
      <NButton quaternary class="rail-toggle" :class="{ collapsed }" @click="emit('toggle-collapsed')">
        <span>{{ collapsed ? "»" : "«" }}</span>
        <span v-if="!collapsed">已打开环境</span>
      </NButton>
    </div>
    <div v-if="collapsed" class="env-compact-content">
      <NScrollbar class="list-scroll" trigger="none">
        <div v-if="hasOpened" class="compact-list">
          <NTooltip v-for="e in openedEnvs" :key="e.id" placement="right" :show-arrow="false">
            <template #trigger>
              <button
                type="button"
                class="compact-item"
                :class="{ active: currentId === e.id }"
                @click="setCurrent(e.id)"
              >
                <span class="compact-item-label">{{ compactEnvItems[e.id]?.shortLabel ?? e.display_name }}</span>
                <span class="compact-item-dot">
                  <NBadge
                    dot
                    :type="statusBadgeType(e.id)"
                    :processing="statusIsConnecting(e.id)"
                    :aria-label="statusLabel(e.id)"
                  />
                </span>
              </button>
            </template>
            <div class="env-status-tip">
              <div>{{ e.display_name }}</div>
              <div v-for="line in hoverLines(e)" :key="line">{{ line }}</div>
            </div>
          </NTooltip>
        </div>
        <p v-else class="empty empty-compact">暂无</p>
      </NScrollbar>
    </div>
    <div v-else class="env-content" :class="{ 'env-content-hidden': collapsed }">
      <NScrollbar class="list-scroll" trigger="none" x-scrollable>
        <ul v-if="hasOpened" class="list">
          <li
            v-for="e in openedEnvs"
            :key="e.id"
            class="item-shell"
            :class="{ active: currentId === e.id }"
          >
            <NCard
              size="small"
              class="item"
              :class="{ active: currentId === e.id }"
              :bordered="false"
              @click="setCurrent(e.id)"
            >
              <div class="item-topline">
                <div class="item-title-wrap">
                  <span class="name" :class="{ 'name-current': currentId === e.id }" :title="e.display_name">
                    {{ e.display_name }}
                  </span>
                </div>
                <div class="item-top-actions">
                  <NTag size="small" round :bordered="false" class="meta-chip soft">{{ sourceLabel(e.source) }}</NTag>
                  <NTooltip placement="right" :show-arrow="false">
                    <template #trigger>
                      <span class="env-item-status-wrap">
                        <NBadge
                          dot
                          :type="statusBadgeType(e.id)"
                          :processing="statusIsConnecting(e.id)"
                          :aria-label="statusLabel(e.id)"
                        />
                      </span>
                    </template>
                    <div class="env-status-tip">
                      <div v-for="line in hoverLines(e)" :key="line">{{ line }}</div>
                    </div>
                  </NTooltip>
                  <NButton text size="tiny" class="close" title="关闭" @click.stop="closeEnv(e.id)">×</NButton>
                </div>
              </div>
              <div
                class="item-context"
                :title="e.source === 'ssh_tunnel' ? remoteKubeconfigLabel(e) : contextLabel(e)"
              >
                {{ e.source === "ssh_tunnel" ? remoteKubeconfigLabel(e) : contextLabel(e) }}
              </div>
              <div class="item-actions-row">
                <NButton
                  v-if="props.onOpenTerminal"
                  size="tiny"
                  tertiary
                  type="primary"
                  class="item-action-btn"
                  title="打开终端"
                  @click.stop="props.onOpenTerminal(e.id)"
                >
                  终端
                </NButton>
                <NButton
                  v-if="getState(e.id) === 'disconnected' && props.onReconnect"
                  size="tiny"
                  secondary
                  type="warning"
                  class="btn-reconnect-small"
                  title="重连"
                  @click.stop="props.onReconnect(e.id)"
                >
                  重连
                </NButton>
              </div>
            </NCard>
          </li>
        </ul>
        <p v-else class="empty">暂无打开的环境</p>
      </NScrollbar>
    </div>
  </aside>
</template>

<style scoped>
.env-bar {
  --wb-env-btn-height-s: 24px;
  --wb-env-pill-radius: 999px;
  width: 100%;
  height: 100%;
  background: var(--sidebar-bg, #fafafa);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.header {
  padding: 0;
  display: flex;
  align-items: center;
  user-select: none;
  border-bottom: 1px solid var(--kf-border, #e0e0e0);
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.94), rgba(248, 250, 252, 0.88));
  min-height: 42px;
  flex-shrink: 0;
}
.rail-toggle {
  width: 100%;
  min-height: 42px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.65rem;
  padding: 0.58rem 0.75rem;
  color: var(--kf-text-primary, #0f172a);
  font-size: 0.875rem;
  font-weight: 700;
}
.rail-toggle:deep(.n-button__content) {
  width: 100%;
  justify-content: center;
  gap: 0.65rem;
}
.rail-toggle.collapsed {
  justify-content: center;
  padding: 0;
}
.env-content {
  flex: 1;
  min-height: 0;
  transition: opacity 0.14s ease, transform 0.14s ease;
}
.env-compact-content {
  flex: 1;
  min-height: 0;
}
.env-content-hidden {
  opacity: 0;
  transform: translateX(-5px);
  pointer-events: none;
}
.list-scroll {
  height: 100%;
}
.list {
  list-style: none;
  margin: 0;
  padding: 0.62rem;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 0.52rem;
}
.compact-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.62rem 0.38rem;
}
.compact-item {
  position: relative;
  width: 100%;
  min-height: 38px;
  padding: 0.42rem 0.32rem 0.32rem;
  border: 1px solid var(--kf-border, #d9e2ec);
  border-radius: 10px;
  background: #ffffff;
  color: var(--kf-text-primary, #0f172a);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}
.compact-item:hover {
  border-color: var(--kf-border-strong, #cbd5e1);
  background: var(--kf-bg-soft, #f8fafc);
}
.compact-item.active {
  border-color: var(--kf-primary, #2563eb);
  background:
    linear-gradient(135deg, rgba(37, 99, 235, 0.14), rgba(14, 165, 233, 0.06)),
    #eff6ff;
  box-shadow: 0 0 0 1px rgba(37, 99, 235, 0.08);
}
.compact-item.active::before {
  content: "";
  position: absolute;
  left: -1px;
  top: 8px;
  bottom: 8px;
  width: 3px;
  border-radius: 999px;
  background: var(--kf-primary, #2563eb);
}
.compact-item-label {
  max-width: calc(100% - 4px);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.72rem;
  line-height: 1.05;
  font-weight: 800;
  letter-spacing: -0.01em;
}
.compact-item-dot {
  position: absolute;
  top: 4px;
  right: 4px;
  display: inline-flex;
}
.item-shell {
  list-style: none;
}
.item {
  display: flex;
  flex-direction: column;
  padding: 0;
  cursor: pointer;
  border-radius: 12px;
  border: 1px solid var(--kf-border, #d9e2ec);
  background: #ffffff;
  box-shadow: 0 1px 2px rgba(15, 23, 42, 0.04);
  transition: border-color 0.18s ease, background-color 0.18s ease, box-shadow 0.18s ease, transform 0.18s ease;
}
.item:deep(.n-card__content) {
  display: flex;
  flex-direction: column;
  gap: 0.42rem;
  padding: 0.62rem 0.68rem;
  background: transparent;
}
.item:deep(.n-card__border),
.item:deep(.n-card-header__main),
.item:deep(.n-card-header) {
  border: none;
}
.item:deep(.n-card) {
  border-radius: 12px;
}
.item:hover {
  border-color: var(--kf-border-strong, #cbd5e1);
  background: var(--kf-bg-soft, #f8fafc);
  box-shadow: 0 3px 10px rgba(15, 23, 42, 0.06);
  transform: translateY(-1px);
}
.item.active {
  border-color: var(--kf-primary, #2563eb);
  background:
    linear-gradient(135deg, rgba(37, 99, 235, 0.14), rgba(14, 165, 233, 0.06)),
    #eff6ff;
  box-shadow:
    0 0 0 1px rgba(37, 99, 235, 0.1),
    0 4px 14px rgba(37, 99, 235, 0.12);
}
.item:focus-within {
  box-shadow:
    0 0 0 2px rgba(37, 99, 235, 0.18),
    0 4px 14px rgba(37, 99, 235, 0.1);
}
.item.active .name {
  color: #1d4ed8;
}
.item-topline {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.65rem;
  min-width: 0;
}
.item-title-wrap {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  min-width: 0;
  flex: 1;
}
.item-top-actions {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  flex-shrink: 0;
}
.item .name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-size: 0.89rem;
  font-weight: 700;
  color: #0f172a;
  line-height: 1.35;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  white-space: normal;
  word-break: break-word;
}
.item-context {
  font-size: 0.75rem;
  color: var(--kf-text-secondary, #64748b);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.item-actions-row {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  flex-wrap: wrap;
}
.name-current {
  color: var(--kf-primary, #2563eb);
}
.meta-chip.soft {
  color: #334155;
  background: #edf2ff;
}
.env-item-status-wrap {
  position: relative;
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  padding: 0 2px;
  cursor: help;
}
.env-status-tip {
  font-size: 0.78rem;
  line-height: 1.5;
  white-space: pre-line;
  max-width: 280px;
}
.item-action-btn {
  --n-height: var(--wb-env-btn-height-s);
  --n-padding: 0 9px;
  --n-font-size: 11px;
  --n-border-radius: var(--wb-env-pill-radius);
  border-radius: var(--wb-env-pill-radius);
}
.btn-reconnect-small {
  --n-height: var(--wb-env-btn-height-s);
  --n-padding: 0 9px;
  --n-font-size: 11px;
  --n-border-radius: var(--wb-env-pill-radius);
}
.item-actions-row :deep(.n-button):focus-visible {
  outline: none;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.18);
}
.close {
  flex-shrink: 0;
  width: 20px;
  height: 20px;
  padding: 0;
  color: var(--kf-text-secondary, #66768f);
  font-size: 1rem;
  line-height: 1;
  border-radius: 6px;
}
.close:hover {
  background: color-mix(in srgb, var(--kf-bg-soft, #f3f6fb) 86%, transparent);
  color: var(--kf-text-primary, #0f172a);
}
.close:focus-visible {
  outline: none;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.18);
}
.empty {
  padding: 0.75rem;
  margin: 0;
  font-size: 0.875rem;
  color: #666;
}
.empty-compact {
  padding: 0.75rem 0.25rem;
  text-align: center;
  font-size: 0.75rem;
}
</style>
