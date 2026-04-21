<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted } from "vue";
import { NButton, NCard, NScrollbar, NTag } from "naive-ui";
import { useEnvStore } from "../stores/env";
import { useConnectionStore } from "../stores/connection";
import { kubeGetTunnelLocalPort } from "../api/kube";
import { envListSshTunnels } from "../api/env";
import type { SshTunnel } from "../api/env";
import { effectiveContext } from "../api/env";

const props = defineProps<{
  collapsed: boolean;
  onReconnect?: (envId: string) => void;
  onOpenTerminal?: (envId: string) => void;
}>();
const emit = defineEmits<{ (e: "toggle"): void }>();

const {
  openedEnvs,
  currentId,
  setCurrent,
  closeEnv,
} = useEnvStore();
const { getState, getError } = useConnectionStore();

const hasOpened = computed(() => openedEnvs.value.length > 0);

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

/** 当前 hover 的状态图标对应的 env id，用于浮层展示 */
const hoveredEnvId = ref<string | null>(null);
/** 触发浮层的元素位置，用于 fixed 定位 */
const triggerRect = ref<{ left: number; top: number; height: number } | null>(null);

function onStatusEnter(ev: MouseEvent, envId: string) {
  const el = ev.currentTarget as HTMLElement;
  const rect = el.getBoundingClientRect();
  hoveredEnvId.value = envId;
  triggerRect.value = { left: rect.right, top: rect.top, height: rect.height };
}

function onStatusLeave() {
  hoveredEnvId.value = null;
  triggerRect.value = null;
}

const hoveredEnv = computed(() =>
  hoveredEnvId.value ? openedEnvs.value.find((e) => e.id === hoveredEnvId.value) ?? null : null
);

watch([openedEnvs, currentId], scheduleRefresh);
onMounted(scheduleRefresh);
onUnmounted(() => {
  if (pollTimer) clearInterval(pollTimer);
});

function toggle() {
  emit("toggle");
}
</script>

<template>
  <aside class="env-bar" :class="{ collapsed: collapsed }">
    <div class="header" @click="toggle">
      <NButton text class="collapse-btn" tabindex="-1">
        <span class="icon" aria-hidden="true">{{ collapsed ? "»" : "«" }}</span>
      </NButton>
      <span class="title" :class="{ 'title-hidden': collapsed }">已打开环境</span>
    </div>
    <div class="env-content" :class="{ 'env-content-hidden': collapsed }">
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
                  <span
                    class="env-item-status-wrap"
                    @mouseenter="onStatusEnter($event, e.id)"
                    @mouseleave="onStatusLeave"
                  >
                    <span
                      class="status-icon"
                      :class="{
                        'status-disconnected': getState(e.id) === 'disconnected',
                        'status-connecting': getState(e.id) === 'connecting',
                        'status-ready': getState(e.id) === 'connected' || getState(e.id) === 'error',
                      }"
                      :aria-label="statusLabel(e.id)"
                    />
                  </span>
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
  <Teleport to="body">
    <div
      v-if="hoveredEnv && triggerRect"
      class="env-status-popover"
      role="tooltip"
      :style="{
        left: `${triggerRect.left + 6}px`,
        top: `${triggerRect.top}px`,
      }"
    >
      <div class="env-status-popover-content">
        {{ hoveredEnv ? hoverLines(hoveredEnv).join('\n') : '' }}
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.env-bar {
  width: 236px;
  min-width: 236px;
  border-right: 1px solid var(--border-color, #e0e0e0);
  background: var(--sidebar-bg, #fafafa);
  display: flex;
  flex-direction: column;
  transition: min-width 0.22s ease, width 0.22s ease;
  will-change: width;
  overflow: hidden;
}
.env-bar.collapsed {
  width: 40px;
  min-width: 40px;
}
.header {
  padding: 0.58rem 0.6rem;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  cursor: pointer;
  user-select: none;
  border-bottom: 1px solid var(--kf-border, #e0e0e0);
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.94), rgba(248, 250, 252, 0.88));
}
.header:hover {
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.98), rgba(241, 245, 249, 0.94));
}
.icon {
  font-size: 1rem;
  color: var(--kf-text-secondary, #66768f);
}
.title {
  font-size: 0.875rem;
  font-weight: 650;
  color: var(--kf-text-primary, #0f172a);
  white-space: nowrap;
  transition: opacity 0.14s ease, transform 0.14s ease;
}
.title-hidden {
  opacity: 0;
  transform: translateX(-4px);
}
.collapse-btn {
  padding: 0.16rem;
}
.env-content {
  flex: 1;
  min-height: 0;
  transition: opacity 0.14s ease, transform 0.14s ease;
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
  transition: border-color 0.16s ease, background-color 0.16s ease, box-shadow 0.16s ease;
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
}
.status-icon {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  flex-shrink: 0;
  box-shadow: 0 0 0 2px #ffffff;
}
.status-disconnected {
  background: #dc2626;
}
.status-connecting {
  background: #0369a1;
  animation: status-pulse 1s ease-in-out infinite;
}
.status-ready {
  background: #16a34a;
}
@keyframes status-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}
.btn-reconnect-small {
  flex-shrink: 0;
  padding: 0.28rem 0.7rem;
  font-size: 0.72rem;
  background: #fff7ed;
  color: #b45309;
  border: 1px solid #fdba74;
  border-radius: 999px;
  cursor: pointer;
}
.btn-reconnect-small:hover {
  background: #ffedd5;
  border-color: #fb923c;
}
.item-action-btn {
  --n-height: 22px;
  --n-padding: 0 9px;
  --n-font-size: 11px;
  --n-border-radius: 999px;
  border-radius: 999px;
}
.btn-reconnect-small {
  --n-height: 22px;
  --n-padding: 0 9px;
  --n-font-size: 11px;
  --n-border-radius: 999px;
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
.empty {
  padding: 0.75rem;
  margin: 0;
  font-size: 0.875rem;
  color: #666;
}
</style>

<style>
/* 浮层挂到 body，需全局样式，可覆盖右侧工作区 */
.env-status-popover {
  position: fixed;
  z-index: 10000;
  padding: 0;
  pointer-events: none;
}
.env-status-popover-content {
  padding: 0.5rem 0.75rem;
  font-size: 0.8125rem;
  line-height: 1.4;
  white-space: pre-line;
  max-width: 280px;
  background: var(--tooltip-bg, #1f2937);
  color: var(--tooltip-fg, #f9fafb);
  border-radius: 6px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
}
</style>
