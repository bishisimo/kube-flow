<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted } from "vue";
import { useEnvStore } from "../stores/env";
import { useConnectionStore } from "../stores/connection";
import { kubeGetTunnelLocalPort } from "../api/kube";
import { envListSshTunnels } from "../api/env";
import type { SshTunnel } from "../api/env";

const props = defineProps<{
  collapsed: boolean;
  onReconnect?: (envId: string) => void;
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

/** 某环境 hover 详情：状态、SSH 的 localPort/remoteHost、断开时的错误信息 */
function hoverLines(e: { id: string; source: string; ssh_tunnel_id?: string | null }): string[] {
  const lines: string[] = [];
  lines.push(`状态: ${statusLabel(e.id)}`);
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
      <span class="icon" aria-hidden="true">{{ collapsed ? "»" : "«" }}</span>
      <span v-if="!collapsed" class="title">已打开环境</span>
    </div>
    <ul v-if="!collapsed && hasOpened" class="list">
      <li
        v-for="e in openedEnvs"
        :key="e.id"
        class="item"
        :class="{ active: currentId === e.id }"
        @click="setCurrent(e.id)"
      >
        <span class="name">{{ e.display_name }}</span>
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
        <button
          v-if="getState(e.id) === 'disconnected' && props.onReconnect"
          type="button"
          class="btn-reconnect-small"
          title="重连"
          @click.stop="props.onReconnect(e.id)"
        >
          重连
        </button>
        <button
          type="button"
          class="close"
          title="关闭"
          @click.stop="closeEnv(e.id)"
        >
          ×
        </button>
      </li>
    </ul>
    <p v-if="!collapsed && !hasOpened" class="empty">暂无打开的环境</p>
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
  width: 200px;
  min-width: 200px;
  border-right: 1px solid var(--border-color, #e0e0e0);
  background: var(--sidebar-bg, #fafafa);
  display: flex;
  flex-direction: column;
  transition: min-width 0.2s, width 0.2s;
}
.env-bar.collapsed {
  width: 40px;
  min-width: 40px;
}
.header {
  padding: 0.75rem;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  cursor: pointer;
  user-select: none;
  border-bottom: 1px solid var(--border-color, #e0e0e0);
}
.header:hover {
  background: rgba(0, 0, 0, 0.04);
}
.icon {
  font-size: 1rem;
  color: #666;
}
.title {
  font-size: 0.875rem;
  font-weight: 500;
}
.list {
  list-style: none;
  margin: 0;
  padding: 0.5rem 0;
  overflow: auto;
}
.item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.5rem 0.75rem;
  cursor: pointer;
  gap: 0.5rem;
}
.item:hover {
  background: rgba(0, 0, 0, 0.04);
}
.item.active {
  background: rgba(57, 108, 216, 0.12);
  color: #396cd8;
}
.item .name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.875rem;
}
.env-item-status-wrap {
  position: relative;
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
}
.status-icon {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
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
  padding: 0.15rem 0.5rem;
  font-size: 0.7rem;
  background: #b45309;
  color: #fff;
  border: none;
  border-radius: 3px;
  cursor: pointer;
}
.btn-reconnect-small:hover {
  background: #92400e;
}
.close {
  flex-shrink: 0;
  width: 20px;
  height: 20px;
  padding: 0;
  border: none;
  background: transparent;
  color: #666;
  cursor: pointer;
  font-size: 1.1rem;
  line-height: 1;
  border-radius: 4px;
}
.close:hover {
  background: rgba(0, 0, 0, 0.08);
  color: #333;
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
