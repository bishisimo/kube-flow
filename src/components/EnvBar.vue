<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted } from "vue";
import { useEnvStore } from "../stores/env";
import { useConnectionStore } from "../stores/connection";
import { kubeGetTunnelLocalPort } from "../api/kube";

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
const { getState, getProgress } = useConnectionStore();

const hasOpened = computed(() => openedEnvs.value.length > 0);

/** SSH 隧道环境的 env_id -> 本地映射端口 */
const tunnelPorts = ref<Record<string, number>>({});

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
  refreshTunnelPorts();
  const hasSshWithoutPort = openedEnvs.value.some(
    (e) => e.source === "ssh_tunnel" && !tunnelPorts.value[e.id]
  );
  if (hasSshWithoutPort) startTunnelPortPolling();
}

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
        <span v-if="getState(e.id) === 'disconnected'" class="status-badge disconnected" title="已断开">断开</span>
        <span
          v-else-if="getState(e.id) === 'connecting'"
          class="status-badge connecting"
          :title="getProgress(e.id)?.stage_label ?? '连接中'"
        >
          连接中
        </span>
        <span v-else-if="e.source === 'ssh_tunnel' && tunnelPorts[e.id]" class="tunnel-port" :title="`SSH 隧道映射: https://127.0.0.1:${tunnelPorts[e.id]}`">
          :{{ tunnelPorts[e.id] }}
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
.tunnel-port {
  flex-shrink: 0;
  font-size: 0.75rem;
  color: #666;
  font-variant-numeric: tabular-nums;
}
.status-badge {
  flex-shrink: 0;
  font-size: 0.7rem;
  padding: 0.1rem 0.35rem;
  border-radius: 3px;
  max-width: 4.5em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.status-badge.disconnected {
  background: #fef2f2;
  color: #dc2626;
}
.status-badge.connecting {
  background: #f0f9ff;
  color: #0369a1;
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
