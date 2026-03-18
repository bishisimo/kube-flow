<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from "vue";
import { useShellStore } from "../stores/shell";
import { useEnvStore } from "../stores/env";
import {
  kubePodExecStart,
  kubePodExecStop,
  kubeRemoveClient,
  kubeGetPodContainers,
  kubeListPodsForWorkload,
  type PodItem,
} from "../api/kube";
import PodShellTerminal from "../components/PodShellTerminal.vue";

const {
  sessions,
  currentSessionId,
  currentSession,
  addSession,
  updateSession,
  removeSession,
  setCurrent,
  pendingOpen,
  clearPendingOpen,
} = useShellStore();

useEnvStore();

const sessionListCollapsed = ref(false);
const podOptions = ref<PodItem[]>([]);
const containerOptions = ref<string[]>([]);
const switcherLoading = ref(false);
const reconnectAttemptMap = ref<Record<string, number>>({});
const reconnectTimerMap = new Map<string, ReturnType<typeof setTimeout>>();
const reconnectingSessionIds = new Set<string>();
const suppressEndStreamIds = new Set<string>();

const RECONNECT_DELAYS_MS = [1000, 2000, 5000, 10000, 15000];
const MAX_RECONNECT_ATTEMPTS = RECONNECT_DELAYS_MS.length;

function extractErrorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function isLikelyConnectionError(message?: string): boolean {
  if (!message) return false;
  const m = message.toLowerCase();
  return [
    "timeout",
    "timed out",
    "connection reset",
    "broken pipe",
    "eof",
    "disconnected",
    "unreachable",
    "transport",
    "ssh",
    "tcp",
    "session not found",
  ].some((k) => m.includes(k));
}

function clearReconnectState(sessionId: string) {
  const next = { ...reconnectAttemptMap.value };
  delete next[sessionId];
  reconnectAttemptMap.value = next;
  const timer = reconnectTimerMap.get(sessionId);
  if (timer) {
    clearTimeout(timer);
    reconnectTimerMap.delete(sessionId);
  }
  reconnectingSessionIds.delete(sessionId);
}

function markStreamSuppressEnd(streamId: string | null) {
  if (streamId) suppressEndStreamIds.add(streamId);
}

async function tryReconnectSession(sessionId: string, resetClient: boolean): Promise<boolean> {
  if (reconnectingSessionIds.has(sessionId)) return false;
  const s = sessions.value.find((x) => x.id === sessionId);
  if (!s) return false;
  reconnectingSessionIds.add(sessionId);
  try {
    if (resetClient) {
      await kubeRemoveClient(s.envId).catch(() => {});
    }
    const streamId = await kubePodExecStart(
      s.envId,
      s.namespace,
      s.podName,
      s.container || null
    );
    updateSession(sessionId, {
      streamId,
      status: "connected",
      error: undefined,
    });
    clearReconnectState(sessionId);
    return true;
  } catch (e) {
    updateSession(sessionId, {
      streamId: null,
      status: "reconnecting",
      error: extractErrorMessage(e),
    });
    return false;
  } finally {
    reconnectingSessionIds.delete(sessionId);
  }
}

function scheduleReconnect(sessionId: string, reason?: string) {
  const s = sessions.value.find((x) => x.id === sessionId);
  if (!s || reconnectTimerMap.has(sessionId)) return;
  const attempt = (reconnectAttemptMap.value[sessionId] ?? 0) + 1;
  reconnectAttemptMap.value = { ...reconnectAttemptMap.value, [sessionId]: attempt };
  if (attempt > MAX_RECONNECT_ATTEMPTS) {
    updateSession(sessionId, {
      streamId: null,
      status: "disconnected",
      error: reason
        ? `${reason}；已重试 ${MAX_RECONNECT_ATTEMPTS} 次，连接仍未恢复`
        : `已重试 ${MAX_RECONNECT_ATTEMPTS} 次，连接仍未恢复`,
    });
    return;
  }
  const delay = RECONNECT_DELAYS_MS[Math.min(attempt - 1, RECONNECT_DELAYS_MS.length - 1)];
  updateSession(sessionId, {
    streamId: null,
    status: "reconnecting",
    error: `连接中断，${Math.round(delay / 1000)} 秒后进行第 ${attempt} 次重连`,
  });
  const timer = setTimeout(async () => {
    reconnectTimerMap.delete(sessionId);
    const resetClient = isLikelyConnectionError(reason);
    const ok = await tryReconnectSession(sessionId, resetClient);
    if (!ok) {
      const latest = sessions.value.find((x) => x.id === sessionId);
      scheduleReconnect(sessionId, latest?.error ?? reason);
    }
  }, delay);
  reconnectTimerMap.set(sessionId, timer);
}

async function openConnection(
  envId: string,
  envName: string,
  namespace: string,
  podName: string,
  container: string,
  workloadKind?: string,
  workloadName?: string
) {
  const id = addSession({
    envId,
    envName,
    namespace,
    podName,
    container,
    workloadKind,
    workloadName,
  });
  try {
    const streamId = await kubePodExecStart(
      envId,
      namespace,
      podName,
      container || null
    );
    updateSession(id, { streamId, status: "connected", error: undefined });
    clearReconnectState(id);
  } catch (e) {
    updateSession(id, {
      status: "error",
      error: extractErrorMessage(e),
    });
  }
}

async function handlePendingOpen() {
  const p = pendingOpen.value;
  if (!p) return;
  clearPendingOpen();

  let podName = p.podName;
  let workloadKind = p.workloadKind;
  let workloadName = p.workloadName;

  if (workloadKind && workloadName) {
    try {
      const pods = await kubeListPodsForWorkload(
        p.envId,
        workloadKind,
        workloadName,
        p.namespace
      );
      const ready = pods.find((x) => x.phase === "Running") ?? pods[0];
      if (!ready) {
        const id = addSession({
          envId: p.envId,
          envName: p.envName,
          namespace: p.namespace,
          podName: workloadName,
          container: "",
          workloadKind,
          workloadName,
        });
        updateSession(id, { status: "error", error: "该 Workload 下暂无 Pod" });
        return;
      }
      podName = ready.name;
    } catch (e) {
      const id = addSession({
        envId: p.envId,
        envName: p.envName,
        namespace: p.namespace,
        podName: workloadName,
        container: "",
        workloadKind,
        workloadName,
      });
      updateSession(id, {
        status: "error",
        error: e instanceof Error ? e.message : String(e),
      });
      return;
    }
  } else if (!podName) {
    return;
  }

  await openConnection(
    p.envId,
    p.envName,
    p.namespace,
    podName,
    p.container ?? "",
    workloadKind,
    workloadName
  );
}

async function loadPodOptions() {
  const s = currentSession.value;
  if (!s?.workloadKind || !s?.workloadName || !s.envId) return;
  switcherLoading.value = true;
  try {
    podOptions.value = await kubeListPodsForWorkload(
      s.envId,
      s.workloadKind,
      s.workloadName,
      s.namespace
    );
  } catch {
    podOptions.value = [];
  } finally {
    switcherLoading.value = false;
  }
}

async function loadContainerOptions() {
  const s = currentSession.value;
  if (!s?.envId || !s?.namespace || !s?.podName) return;
  try {
    containerOptions.value = await kubeGetPodContainers(
      s.envId,
      s.namespace,
      s.podName
    );
  } catch {
    containerOptions.value = [];
  }
}

async function switchPod(newPodName: string) {
  const s = currentSession.value;
  if (!s || s.podName === newPodName) return;
  if (s.streamId) {
    markStreamSuppressEnd(s.streamId);
    await kubePodExecStop(s.streamId);
  }
  clearReconnectState(s.id);
  updateSession(s.id, { streamId: null, status: "connecting", podName: newPodName });
  try {
    const containers = await kubeGetPodContainers(s.envId, s.namespace, newPodName);
    const container = containers[0] ?? "";
    updateSession(s.id, { container });
    const streamId = await kubePodExecStart(
      s.envId,
      s.namespace,
      newPodName,
      container || null
    );
    updateSession(s.id, { streamId, status: "connected", error: undefined });
    containerOptions.value = containers;
  } catch (e) {
    updateSession(s.id, {
      status: "error",
      error: extractErrorMessage(e),
    });
  }
}

async function switchContainer(newContainer: string) {
  const s = currentSession.value;
  if (!s || s.container === newContainer) return;
  if (s.streamId) {
    markStreamSuppressEnd(s.streamId);
    await kubePodExecStop(s.streamId);
  }
  clearReconnectState(s.id);
  updateSession(s.id, { streamId: null, status: "connecting", container: newContainer });
  try {
    const streamId = await kubePodExecStart(
      s.envId,
      s.namespace,
      s.podName,
      newContainer || null
    );
    updateSession(s.id, { streamId, status: "connected", error: undefined });
  } catch (e) {
    updateSession(s.id, {
      status: "error",
      error: extractErrorMessage(e),
    });
  }
}

function closeSession(id: string) {
  const s = sessions.value.find((x) => x.id === id);
  if (s?.streamId) {
    markStreamSuppressEnd(s.streamId);
    kubePodExecStop(s.streamId).catch(() => {});
  }
  clearReconnectState(id);
  removeSession(id);
}

function onTerminalEnd(sessionId: string, payload: { streamId: string; error?: string }) {
  if (suppressEndStreamIds.has(payload.streamId)) {
    suppressEndStreamIds.delete(payload.streamId);
    return;
  }
  const s = sessions.value.find((x) => x.id === sessionId);
  if (!s) return;
  if (s.streamId && s.streamId !== payload.streamId) return;
  updateSession(sessionId, {
    streamId: null,
    status: "reconnecting",
    error: payload.error ?? "Shell 连接已断开",
  });
  scheduleReconnect(sessionId, payload.error);
}

async function reconnectSessionNow(sessionId: string) {
  clearReconnectState(sessionId);
  updateSession(sessionId, {
    streamId: null,
    status: "reconnecting",
    error: "正在重连…",
  });
  const ok = await tryReconnectSession(sessionId, true);
  if (!ok) {
    const latest = sessions.value.find((x) => x.id === sessionId);
    scheduleReconnect(sessionId, latest?.error ?? "重连失败");
  }
}

watch(pendingOpen, (p) => {
  if (p) handlePendingOpen();
});

watch(
  () => currentSession.value,
  async (s) => {
    if (!s) {
      podOptions.value = [];
      containerOptions.value = [];
      return;
    }
    if (s.workloadKind && s.workloadName) {
      await loadPodOptions();
    } else {
      podOptions.value = [];
    }
    if (s.podName) {
      await loadContainerOptions();
    } else {
      containerOptions.value = [];
    }
  },
  { immediate: true }
);

onMounted(() => {
  if (pendingOpen.value) handlePendingOpen();
});

onUnmounted(() => {
  for (const timer of reconnectTimerMap.values()) clearTimeout(timer);
  reconnectTimerMap.clear();
  reconnectingSessionIds.clear();
  suppressEndStreamIds.clear();
});
</script>

<template>
  <div class="pod-shell-view">
    <aside class="session-bar" :class="{ collapsed: sessionListCollapsed }">
      <div class="header" @click="sessionListCollapsed = !sessionListCollapsed">
        <span class="icon" aria-hidden="true">
          {{ sessionListCollapsed ? "»" : "«" }}
        </span>
        <span v-if="!sessionListCollapsed" class="title">Shell 连接</span>
      </div>
      <ul v-if="!sessionListCollapsed" class="list">
        <li
          v-for="s in sessions"
          :key="s.id"
          class="item"
          :class="{ active: currentSessionId === s.id }"
          @click="setCurrent(s.id)"
        >
          <span class="label" :title="`${s.envName} / ${s.namespace} / ${s.podName} / ${s.container}`">
            {{ s.podName }}{{ s.container ? ` (${s.container})` : "" }}
          </span>
          <span
            v-if="s.status === 'connecting' || s.status === 'reconnecting'"
            class="status-badge connecting"
          >
            {{ s.status === "reconnecting" ? "重连中" : "连接中" }}
          </span>
          <span
            v-else-if="s.status === 'disconnected'"
            class="status-badge error"
            :title="s.error"
          >
            已断开
          </span>
          <span
            v-else-if="s.status === 'error'"
            class="status-badge error"
            :title="s.error"
          >
            错误
          </span>
          <button
            type="button"
            class="close"
            title="关闭"
            @click.stop="closeSession(s.id)"
          >
            ×
          </button>
        </li>
      </ul>
      <p v-if="!sessionListCollapsed && !sessions.length" class="empty">
        从工作台选择 Pod 或 Deployment/StatefulSet 打开 Shell
      </p>
    </aside>

    <main class="shell-main">
      <div
        v-if="
          currentSession &&
          (currentSession.streamId || currentSession.status === 'connecting' || currentSession.status === 'reconnecting')
        "
        class="shell-terminal-wrap"
      >
        <div class="switcher-bar">
          <div v-if="currentSession.workloadKind && podOptions.length > 1" class="switcher-row">
            <label class="switcher-label">Pod</label>
            <select
              :value="currentSession.podName"
              class="switcher-select"
              :disabled="switcherLoading || currentSession.status === 'connecting' || currentSession.status === 'reconnecting'"
              @change="switchPod(($event.target as HTMLSelectElement).value)"
            >
              <option v-for="po in podOptions" :key="po.name" :value="po.name">
                {{ po.name }}
              </option>
            </select>
          </div>
          <div v-if="containerOptions.length > 1" class="switcher-row">
            <label class="switcher-label">容器</label>
            <select
              :value="currentSession.container"
              class="switcher-select"
              :disabled="currentSession.status === 'connecting' || currentSession.status === 'reconnecting'"
              @change="switchContainer(($event.target as HTMLSelectElement).value)"
            >
              <option v-for="c in containerOptions" :key="c" :value="c">
                {{ c }}
              </option>
            </select>
          </div>
        </div>
        <PodShellTerminal
          v-if="currentSession.streamId"
          :stream-id="currentSession.streamId"
          @end="onTerminalEnd(currentSession.id, $event)"
        />
        <div v-else class="empty-state connecting">
          <p v-if="currentSession.status === 'reconnecting'">
            正在重连 {{ currentSession.podName }}…
          </p>
          <p v-else>
            正在连接 {{ currentSession.podName }}…
          </p>
          <p v-if="currentSession.error" class="reconnect-hint">{{ currentSession.error }}</p>
        </div>
      </div>
      <template v-else-if="currentSession && (currentSession.status === 'connecting' || currentSession.status === 'reconnecting')">
        <div class="empty-state">
          <p v-if="currentSession.status === 'reconnecting'">正在重连 {{ currentSession.podName }}…</p>
          <p v-else>正在连接 {{ currentSession.podName }}…</p>
          <p v-if="currentSession.error" class="reconnect-hint">{{ currentSession.error }}</p>
        </div>
      </template>
      <template v-else-if="currentSession && (currentSession.status === 'error' || currentSession.status === 'disconnected')">
        <div class="empty-state error">
          <p>连接失败：{{ currentSession.error }}</p>
          <div class="error-actions">
            <button type="button" class="btn-reconnect" @click="reconnectSessionNow(currentSession.id)">
              立即重连
            </button>
            <button type="button" class="btn-reconnect" @click="closeSession(currentSession.id)">
              关闭
            </button>
          </div>
        </div>
      </template>
      <template v-else>
        <div class="empty-state">
          <p>从工作台选择 Pod 或 Deployment/StatefulSet 打开 Shell</p>
        </div>
      </template>
    </main>

  </div>
</template>

<style scoped>
.pod-shell-view {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.session-bar {
  width: 220px;
  flex-shrink: 0;
  background: #fff;
  border-right: 1px solid #e2e8f0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}
.session-bar.collapsed {
  width: 40px;
}
.session-bar .header {
  padding: 0.5rem 0.75rem;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  border-bottom: 1px solid #e2e8f0;
  flex-shrink: 0;
}
.session-bar .icon {
  font-size: 0.875rem;
  color: #64748b;
}
.session-bar .title {
  font-size: 0.8125rem;
  font-weight: 500;
  color: #334155;
}
.session-bar .list {
  list-style: none;
  margin: 0;
  padding: 0.25rem 0;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
}
.session-bar .item {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.4rem 0.75rem;
  font-size: 0.8125rem;
  cursor: pointer;
}
.session-bar .item:hover {
  background: #f8fafc;
}
.session-bar .item.active {
  background: rgba(37, 99, 235, 0.1);
  color: #2563eb;
}
.session-bar .label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.session-bar .status-badge {
  font-size: 0.6875rem;
  padding: 0.1rem 0.35rem;
  border-radius: 4px;
}
.session-bar .status-badge.connecting {
  background: #fef3c7;
  color: #b45309;
}
.session-bar .status-badge.error {
  background: #fef2f2;
  color: #dc2626;
}
.session-bar .close {
  padding: 0 0.25rem;
  border: none;
  background: none;
  font-size: 1rem;
  color: #94a3b8;
  cursor: pointer;
  line-height: 1;
}
.session-bar .close:hover {
  color: #dc2626;
}
.session-bar .empty {
  padding: 0.75rem 1rem;
  font-size: 0.8125rem;
  color: #94a3b8;
  margin: 0;
}

.shell-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
  background: #1e293b;
}

.shell-terminal-wrap {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

.switcher-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.75rem 1rem;
  padding: 0.35rem 0.75rem;
  background: rgba(0, 0, 0, 0.2);
  flex-shrink: 0;
}

.switcher-row {
  display: flex;
  align-items: center;
  gap: 0.35rem;
}

.switcher-label {
  font-size: 0.75rem;
  color: #94a3b8;
}

.switcher-select {
  padding: 0.25rem 0.5rem;
  border: 1px solid #475569;
  border-radius: 4px;
  background: #1e293b;
  color: #e2e8f0;
  font-size: 0.8125rem;
  min-width: 120px;
}

.switcher-select:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  color: #94a3b8;
  font-size: 0.875rem;
}
.empty-state.error {
  color: #fca5a5;
}
.empty-state.connecting {
  color: #94a3b8;
}
.empty-state .reconnect-hint {
  margin: 0;
  font-size: 0.8125rem;
  opacity: 0.9;
}
.empty-state .error-actions {
  display: flex;
  gap: 0.5rem;
}
.empty-state .btn-reconnect {
  padding: 0.35rem 0.75rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  background: #fff;
  font-size: 0.8125rem;
  cursor: pointer;
}
</style>
