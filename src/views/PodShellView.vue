<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useShellStore } from "../stores/shell";
import { useEnvStore } from "../stores/env";
import {
  kubeGetPodContainers,
  kubeListPodsForWorkload,
  kubePodExecStart,
  kubePodExecStop,
  kubeRemoveClient,
  type PodItem,
} from "../api/kube";
import { hostShellStart, hostShellStdin, hostShellStop } from "../api/terminal";
import { useStrongholdAuthStore } from "../stores/strongholdAuth";
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
const { environments } = useEnvStore();
const strongholdAuth = useStrongholdAuthStore();

const sessionRailCollapsed = ref(false);
const podOptions = ref<PodItem[]>([]);
const containerOptions = ref<string[]>([]);
const switcherLoading = ref(false);
const hostEntryEnvId = ref("");
const reconnectAttemptMap = ref<Record<string, number>>({});
const reconnectTimerMap = new Map<string, ReturnType<typeof setTimeout>>();
const reconnectingSessionIds = new Set<string>();
const suppressEndStreamIds = new Set<string>();

const RECONNECT_DELAYS_MS = [1000, 2000, 5000, 10000, 15000];
const MAX_RECONNECT_ATTEMPTS = RECONNECT_DELAYS_MS.length;

const groupedSessions = computed(() => {
  const groups = new Map<string, { envId: string; envName: string; items: typeof sessions.value }>();
  for (const session of sessions.value) {
    const group =
      groups.get(session.envId) ??
      { envId: session.envId, envName: session.envName, items: [] as typeof sessions.value };
    group.items.push(session);
    groups.set(session.envId, group);
  }
  return Array.from(groups.values()).sort((a, b) => a.envName.localeCompare(b.envName));
});

const hostEntryOptions = computed(() =>
  [...environments.value].sort((a, b) => a.display_name.localeCompare(b.display_name))
);

const currentSessionSubtitle = computed(() => {
  const session = currentSession.value;
  if (!session) return "集中管理 Pod Shell 与主机 Shell。";
  if (session.kind === "host") return "";
  const ns = session.namespace || "default";
  const container = session.container ? ` / ${session.container}` : "";
  return `${session.envName} / ${ns}${container}`;
});

const currentSessionContextName = computed(() => {
  const session = currentSession.value;
  if (!session) return "";
  if (session.kind === "host") return session.hostLabel || `${session.envName} 主机`;
  return session.podName || "Pod";
});

function sessionBadge(session: (typeof sessions.value)[number]): string {
  return session.kind === "host" ? "主机" : "Pod";
}

function sessionLabel(session: (typeof sessions.value)[number]): string {
  if (session.kind === "host") return session.hostLabel || `${session.envName} 主机`;
  return `${session.podName || "-"}${session.container ? ` (${session.container})` : ""}`;
}

function extractErrorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

async function handleStrongholdLocked(message: string, onConfirmed: () => void): Promise<boolean> {
  return strongholdAuth.checkAndHandle(message, onConfirmed, {
    title: "解锁终端凭证",
    description: "当前终端会话需要访问已保存凭证，请先输入 Stronghold 主密码解锁。",
  });
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

async function startHostSessionStream(sessionId: string): Promise<boolean> {
  const session = sessions.value.find((item) => item.id === sessionId);
  if (!session) return false;
  try {
    const streamId = await hostShellStart(session.envId, session.nodeTerminalLaunch ?? null);
    updateSession(sessionId, {
      streamId,
      status: "connected",
      error: undefined,
      hostLabel: session.hostLabel || `${session.envName} 主机`,
    });
    if (!session.nodeTerminalLaunch) {
      scheduleHostBootstrap(streamId, session.bootstrapCommands);
    }
    clearReconnectState(sessionId);
    return true;
  } catch (e) {
    const msg = extractErrorMessage(e);
    const isStrongholdRequired = await handleStrongholdLocked(msg, () => {
      void startHostSessionStream(sessionId);
    });
    if (isStrongholdRequired) {
      updateSession(sessionId, {
        streamId: null,
        status: "disconnected",
        error: "需要先解锁 Stronghold，解锁后可点击“重新连接”继续。",
      });
      return false;
    }
    updateSession(sessionId, {
      streamId: null,
      status: "reconnecting",
      error: msg,
    });
    return false;
  }
}

async function tryReconnectSession(sessionId: string, resetClient: boolean): Promise<boolean> {
  if (reconnectingSessionIds.has(sessionId)) return false;
  const session = sessions.value.find((item) => item.id === sessionId);
  if (!session) return false;
  reconnectingSessionIds.add(sessionId);
  try {
    if (session.kind === "host") {
      return await startHostSessionStream(sessionId);
    }
    if (resetClient) {
      await kubeRemoveClient(session.envId).catch(() => {});
    }
    const streamId = await kubePodExecStart(
      session.envId,
      session.namespace || "default",
      session.podName || "",
      session.container || null
    );
    updateSession(sessionId, {
      streamId,
      status: "connected",
      error: undefined,
    });
    clearReconnectState(sessionId);
    return true;
  } catch (e) {
    const msg = extractErrorMessage(e);
    const isStrongholdRequired = await handleStrongholdLocked(msg, () => {
      void tryReconnectSession(sessionId, resetClient);
    });
    if (isStrongholdRequired) {
      updateSession(sessionId, {
        streamId: null,
        status: "disconnected",
        error: "需要先解锁 Stronghold，解锁后可点击“重新连接”继续。",
      });
      return false;
    }
    updateSession(sessionId, {
      streamId: null,
      status: "reconnecting",
      error: msg,
    });
    return false;
  } finally {
    reconnectingSessionIds.delete(sessionId);
  }
}

function scheduleReconnect(sessionId: string, reason?: string) {
  const session = sessions.value.find((item) => item.id === sessionId);
  if (!session || reconnectTimerMap.has(sessionId)) return;
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
    error:
      session.kind === "host"
        ? `主机连接中断，${Math.round(delay / 1000)} 秒后进行第 ${attempt} 次重连`
        : `连接中断，${Math.round(delay / 1000)} 秒后进行第 ${attempt} 次重连`,
  });
  const timer = setTimeout(async () => {
    reconnectTimerMap.delete(sessionId);
    const resetClient = isLikelyConnectionError(reason);
    const ok = await tryReconnectSession(sessionId, resetClient);
    if (!ok) {
      const latest = sessions.value.find((item) => item.id === sessionId);
      scheduleReconnect(sessionId, latest?.error ?? reason);
    }
  }, delay);
  reconnectTimerMap.set(sessionId, timer);
}

async function openPodConnection(
  envId: string,
  envName: string,
  namespace: string,
  podName: string,
  container: string,
  workloadKind?: string,
  workloadName?: string,
  existingSessionId?: string
) {
  const id =
    existingSessionId ??
    addSession({
      kind: "pod",
      envId,
      envName,
      namespace,
      podName,
      container,
      workloadKind,
      workloadName,
    });
  if (existingSessionId) {
    updateSession(id, {
      streamId: null,
      status: "connecting",
      error: undefined,
      podName,
      container,
    });
  }
  try {
    const streamId = await kubePodExecStart(envId, namespace, podName, container || null);
    updateSession(id, { streamId, status: "connected", error: undefined });
    clearReconnectState(id);
  } catch (e) {
    const msg = extractErrorMessage(e);
    const isStrongholdRequired = await handleStrongholdLocked(msg, () => {
      void openPodConnection(
        envId,
        envName,
        namespace,
        podName,
        container,
        workloadKind,
        workloadName,
        id
      );
    });
    if (isStrongholdRequired) {
      updateSession(id, {
        status: "disconnected",
        error: "需要先解锁 Stronghold，解锁后可点击“重新连接”继续。",
      });
      return;
    }
    updateSession(id, { status: "error", error: msg });
  }
}

function scheduleHostBootstrap(streamId: string, commands?: string[]) {
  const normalized = (commands ?? []).map((item) => item.trim()).filter(Boolean);
  if (!normalized.length) return;
  window.setTimeout(() => {
    const text = `${normalized.join("\n")}\n`;
    const bytes = Array.from(new TextEncoder().encode(text));
    hostShellStdin(streamId, bytes).catch(() => {});
  }, 320);
}

async function openHostConnectionWithBootstrap(
  envId: string,
  envName: string,
  hostLabel?: string,
  bootstrapCommands?: string[],
  nodeTerminalLaunch?: import("../api/terminal").HostShellBootstrap | null,
  existingSessionId?: string
) {
  const nextHostLabel = hostLabel || `${envName} 主机`;
  const id =
    existingSessionId ??
    addSession({
      kind: "host",
      envId,
      envName,
      hostLabel: nextHostLabel,
      bootstrapCommands,
      nodeTerminalLaunch: nodeTerminalLaunch ?? null,
    });
  if (existingSessionId) {
    updateSession(id, {
      streamId: null,
      status: "connecting",
      error: undefined,
      hostLabel: nextHostLabel,
    });
  }
  try {
    const streamId = await hostShellStart(envId, nodeTerminalLaunch ?? null);
    updateSession(id, { streamId, status: "connected", error: undefined });
    if (!nodeTerminalLaunch) {
      scheduleHostBootstrap(streamId, bootstrapCommands);
    }
    clearReconnectState(id);
  } catch (e) {
    const msg = extractErrorMessage(e);
    const isStrongholdRequired = await handleStrongholdLocked(msg, () => {
      void openHostConnectionWithBootstrap(
        envId,
        envName,
        hostLabel,
        bootstrapCommands,
        nodeTerminalLaunch,
        id
      );
    });
    if (isStrongholdRequired) {
      updateSession(id, {
        status: "disconnected",
        error: "需要先解锁 Stronghold，解锁后可点击“重新连接”继续。",
      });
      return;
    }
    updateSession(id, { status: "error", error: msg });
  }
}

async function handlePendingOpen() {
  const pending = pendingOpen.value;
  if (!pending) return;
  clearPendingOpen();

  if (pending.kind === "host") {
    await openHostConnectionWithBootstrap(
      pending.envId,
      pending.envName,
      pending.hostLabel,
      pending.bootstrapCommands,
      pending.nodeTerminalLaunch ?? null
    );
    return;
  }

  let podName = pending.podName;
  let workloadKind = pending.workloadKind;
  let workloadName = pending.workloadName;

  if (workloadKind && workloadName) {
    try {
      const pods = await kubeListPodsForWorkload(
        pending.envId,
        workloadKind,
        workloadName,
        pending.namespace || "default"
      );
      const ready = pods.find((item) => item.phase === "Running") ?? pods[0];
      if (!ready) {
        const id = addSession({
          kind: "pod",
          envId: pending.envId,
          envName: pending.envName,
          namespace: pending.namespace || "default",
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
        kind: "pod",
        envId: pending.envId,
        envName: pending.envName,
        namespace: pending.namespace || "default",
        podName: workloadName,
        container: "",
        workloadKind,
        workloadName,
      });
      updateSession(id, { status: "error", error: extractErrorMessage(e) });
      return;
    }
  } else if (!podName) {
    return;
  }

  await openPodConnection(
    pending.envId,
    pending.envName,
    pending.namespace || "default",
    podName,
    pending.container ?? "",
    workloadKind,
    workloadName
  );
}

async function loadPodOptions() {
  const session = currentSession.value;
  if (!session || session.kind !== "pod" || !session.workloadKind || !session.workloadName) return;
  switcherLoading.value = true;
  try {
    podOptions.value = await kubeListPodsForWorkload(
      session.envId,
      session.workloadKind,
      session.workloadName,
      session.namespace || "default"
    );
  } catch {
    podOptions.value = [];
  } finally {
    switcherLoading.value = false;
  }
}

async function loadContainerOptions() {
  const session = currentSession.value;
  if (!session || session.kind !== "pod" || !session.namespace || !session.podName) return;
  try {
    containerOptions.value = await kubeGetPodContainers(
      session.envId,
      session.namespace,
      session.podName
    );
  } catch {
    containerOptions.value = [];
  }
}

async function switchPod(newPodName: string) {
  const session = currentSession.value;
  if (!session || session.kind !== "pod" || session.podName === newPodName || !session.namespace) return;
  if (session.streamId) {
    markStreamSuppressEnd(session.streamId);
    await kubePodExecStop(session.streamId);
  }
  clearReconnectState(session.id);
  updateSession(session.id, { streamId: null, status: "connecting", podName: newPodName });
  try {
    const containers = await kubeGetPodContainers(session.envId, session.namespace, newPodName);
    const container = containers[0] ?? "";
    updateSession(session.id, { container });
    const streamId = await kubePodExecStart(
      session.envId,
      session.namespace,
      newPodName,
      container || null
    );
    updateSession(session.id, { streamId, status: "connected", error: undefined });
    containerOptions.value = containers;
  } catch (e) {
    const msg = extractErrorMessage(e);
    const isStrongholdRequired = await handleStrongholdLocked(msg, () => {
      void switchPod(newPodName);
    });
    if (isStrongholdRequired) {
      updateSession(session.id, {
        status: "disconnected",
        error: "需要先解锁 Stronghold，解锁后可点击“重新连接”继续。",
      });
      return;
    }
    updateSession(session.id, { status: "error", error: msg });
  }
}

async function switchContainer(newContainer: string) {
  const session = currentSession.value;
  if (!session || session.kind !== "pod" || session.container === newContainer || !session.namespace || !session.podName) return;
  if (session.streamId) {
    markStreamSuppressEnd(session.streamId);
    await kubePodExecStop(session.streamId);
  }
  clearReconnectState(session.id);
  updateSession(session.id, { streamId: null, status: "connecting", container: newContainer });
  try {
    const streamId = await kubePodExecStart(
      session.envId,
      session.namespace,
      session.podName,
      newContainer || null
    );
    updateSession(session.id, { streamId, status: "connected", error: undefined });
  } catch (e) {
    const msg = extractErrorMessage(e);
    const isStrongholdRequired = await handleStrongholdLocked(msg, () => {
      void switchContainer(newContainer);
    });
    if (isStrongholdRequired) {
      updateSession(session.id, {
        status: "disconnected",
        error: "需要先解锁 Stronghold，解锁后可点击“重新连接”继续。",
      });
      return;
    }
    updateSession(session.id, { status: "error", error: msg });
  }
}

function closeSession(id: string) {
  const session = sessions.value.find((item) => item.id === id);
  if (session?.streamId) {
    markStreamSuppressEnd(session.streamId);
    if (session.kind === "host") {
      hostShellStop(session.streamId).catch(() => {});
    } else {
      kubePodExecStop(session.streamId).catch(() => {});
    }
  }
  clearReconnectState(id);
  removeSession(id);
}

function onTerminalEnd(sessionId: string, payload: { streamId: string; error?: string }) {
  if (suppressEndStreamIds.has(payload.streamId)) {
    suppressEndStreamIds.delete(payload.streamId);
    return;
  }
  const session = sessions.value.find((item) => item.id === sessionId);
  if (!session) return;
  if (session.streamId && session.streamId !== payload.streamId) return;
  updateSession(sessionId, {
    streamId: null,
    status: "reconnecting",
    error: payload.error ?? (session.kind === "host" ? "主机 Shell 连接已断开" : "Shell 连接已断开"),
  });
  scheduleReconnect(sessionId, payload.error);
}

async function reconnectSessionNow(sessionId: string) {
  clearReconnectState(sessionId);
  updateSession(sessionId, { streamId: null, status: "reconnecting", error: "正在重连…" });
  const ok = await tryReconnectSession(sessionId, true);
  if (!ok) {
    const latest = sessions.value.find((item) => item.id === sessionId);
    scheduleReconnect(sessionId, latest?.error ?? "重连失败");
  }
}

async function openHostShellForEnv(envId: string) {
  const env = environments.value.find((item) => item.id === envId);
  if (!env) return;
  await openHostConnectionWithBootstrap(env.id, env.display_name, `${env.display_name} 主机`);
}

watch(pendingOpen, (pending) => {
  if (pending) void handlePendingOpen();
});

watch(
  () => currentSession.value,
  async (session) => {
    if (!session) {
      podOptions.value = [];
      containerOptions.value = [];
      return;
    }
    if (session.kind === "pod" && session.workloadKind && session.workloadName) {
      await loadPodOptions();
    } else {
      podOptions.value = [];
    }
    if (session.kind === "pod" && session.podName) {
      await loadContainerOptions();
    } else {
      containerOptions.value = [];
    }
  },
  { immediate: true }
);

watch(
  () => hostEntryOptions.value.map((item) => item.id).join(","),
  () => {
    if (!hostEntryEnvId.value || !hostEntryOptions.value.some((item) => item.id === hostEntryEnvId.value)) {
      hostEntryEnvId.value = hostEntryOptions.value[0]?.id ?? "";
    }
  },
  { immediate: true }
);

onMounted(() => {
  if (pendingOpen.value) void handlePendingOpen();
});

onUnmounted(() => {
  for (const timer of reconnectTimerMap.values()) clearTimeout(timer);
  reconnectTimerMap.clear();
  reconnectingSessionIds.clear();
  suppressEndStreamIds.clear();
});
</script>

<template>
  <div class="terminal-center">
    <aside class="session-rail" :class="{ collapsed: sessionRailCollapsed }">
      <button type="button" class="rail-toggle" @click="sessionRailCollapsed = !sessionRailCollapsed">
        <span>{{ sessionRailCollapsed ? "»" : "«" }}</span>
        <span v-if="!sessionRailCollapsed">会话</span>
      </button>
      <div v-if="!sessionRailCollapsed" class="session-rail-body">
        <div class="quick-open-card">
          <div class="quick-open-title">打开指定环境终端</div>
          <div class="quick-open-desc">终端中心不依赖已打开环境，直接选择目标环境即可进入主机终端。</div>
          <div class="quick-open-actions">
            <select v-model="hostEntryEnvId" class="quick-open-select">
              <option value="" disabled>选择环境</option>
              <option v-for="env in hostEntryOptions" :key="env.id" :value="env.id">
                {{ env.display_name }}
              </option>
            </select>
            <button type="button" class="quick-open-btn" :disabled="!hostEntryEnvId" @click="openHostShellForEnv(hostEntryEnvId)">
              打开终端
            </button>
          </div>
        </div>

        <div v-if="groupedSessions.length" class="session-groups">
          <section v-for="group in groupedSessions" :key="group.envId" class="session-group">
            <div class="session-group-title">{{ group.envName }}</div>
            <div
              v-for="session in group.items"
              :key="session.id"
              class="session-item"
              :class="{ active: currentSessionId === session.id }"
            >
              <button type="button" class="session-item-main-button" @click="setCurrent(session.id)">
                <span class="session-item-badge" :class="session.kind">{{ sessionBadge(session) }}</span>
                <span class="session-item-main">
                  <span class="session-item-name" :title="sessionLabel(session)">{{ sessionLabel(session) }}</span>
                  <span class="session-item-meta">
                    {{
                      session.status === "reconnecting"
                        ? "重连中"
                        : session.status === "connecting"
                          ? "连接中"
                          : session.status === "connected"
                            ? "已连接"
                            : session.status === "error"
                              ? "错误"
                              : "已断开"
                    }}
                  </span>
                </span>
              </button>
              <button type="button" class="session-item-close" @click.stop="closeSession(session.id)">×</button>
            </div>
          </section>
        </div>
        <div v-else class="session-empty">
          还没有会话。你可以从工作台打开 Pod，也可以直接在这里打开指定环境终端。
        </div>
      </div>
    </aside>

    <main class="terminal-main">
      <header class="terminal-header">
        <div v-if="currentSessionSubtitle" class="terminal-header-main">
          <p class="terminal-subtitle">{{ currentSessionSubtitle }}</p>
        </div>
      </header>

      <section
        v-if="currentSession && (currentSession.streamId || currentSession.status === 'connecting' || currentSession.status === 'reconnecting')"
        class="terminal-stage"
      >
        <div class="terminal-context-bar">
          <div class="context-pill" :class="currentSession.kind">
            {{ currentSession.kind === "host" ? "主机 Shell" : "Pod Shell" }}
          </div>
          <div v-if="currentSessionContextName" class="context-name-pill" :class="currentSession.kind">
            {{ currentSessionContextName }}
          </div>
          <div v-if="currentSession.kind === 'pod' && currentSession.workloadKind && podOptions.length > 1" class="switcher-row">
            <label class="switcher-label">Pod</label>
            <select
              :value="currentSession.podName"
              class="switcher-select"
              :disabled="switcherLoading || currentSession.status === 'connecting' || currentSession.status === 'reconnecting'"
              @change="switchPod(($event.target as HTMLSelectElement).value)"
            >
              <option v-for="pod in podOptions" :key="pod.name" :value="pod.name">
                {{ pod.name }}
              </option>
            </select>
          </div>
          <div v-if="currentSession.kind === 'pod' && containerOptions.length > 1" class="switcher-row">
            <label class="switcher-label">容器</label>
            <select
              :value="currentSession.container"
              class="switcher-select"
              :disabled="currentSession.status === 'connecting' || currentSession.status === 'reconnecting'"
              @change="switchContainer(($event.target as HTMLSelectElement).value)"
            >
              <option v-for="container in containerOptions" :key="container" :value="container">
                {{ container }}
              </option>
            </select>
          </div>
          <div class="context-bar-spacer"></div>
          <button
            type="button"
            class="context-action"
            @click="reconnectSessionNow(currentSession.id)"
          >
            重新连接
          </button>
        </div>

        <PodShellTerminal
          v-if="currentSession.streamId"
          :stream-id="currentSession.streamId"
          :mode="currentSession.kind"
          @end="onTerminalEnd(currentSession.id, $event)"
        />

        <div v-else class="terminal-loading">
          <p>
            {{ currentSession.status === "reconnecting" ? "正在恢复终端连接…" : "正在建立终端连接…" }}
          </p>
          <p v-if="currentSession.error" class="terminal-loading-hint">{{ currentSession.error }}</p>
        </div>
      </section>

      <section
        v-else-if="currentSession && (currentSession.status === 'error' || currentSession.status === 'disconnected')"
        class="terminal-empty error"
      >
        <div class="empty-title">当前会话暂不可用</div>
        <div class="empty-desc">{{ currentSession.error }}</div>
        <div class="empty-actions">
          <button type="button" class="header-action" @click="reconnectSessionNow(currentSession.id)">立即重连</button>
          <button type="button" class="header-action secondary" @click="closeSession(currentSession.id)">关闭会话</button>
        </div>
      </section>

      <section v-else class="terminal-empty">
        <div class="empty-hero">
          <div class="empty-kicker">终端工作区</div>
          <h3>同时管理 Pod 与主机会话</h3>
          <p>从工作台、资源详情，或者直接在这里选择环境打开终端，会话都会统一收纳在这里。</p>
        </div>
        <div class="empty-grid">
          <button
            v-if="hostEntryEnvId"
            type="button"
            class="empty-card"
            @click="openHostShellForEnv(hostEntryEnvId)"
          >
            <span class="empty-card-title">打开指定环境终端</span>
            <span class="empty-card-desc">
              {{ hostEntryOptions.find((env) => env.id === hostEntryEnvId)?.display_name || "选择环境" }}
            </span>
          </button>
          <div class="empty-card info">
            <span class="empty-card-title">从工作台进入 Pod</span>
            <span class="empty-card-desc">右键 Pod / Deployment / StatefulSet / DaemonSet 即可打开。</span>
          </div>
        </div>
      </section>
    </main>
  </div>
</template>

<style scoped>
.terminal-center {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  background:
    radial-gradient(circle at top left, rgba(37, 99, 235, 0.12), transparent 28%),
    linear-gradient(180deg, #f8fbff 0%, #eef4fb 100%);
}

.session-rail {
  width: 320px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-right: 1px solid rgba(148, 163, 184, 0.22);
  background: rgba(255, 255, 255, 0.92);
  backdrop-filter: blur(18px);
}

.session-rail.collapsed {
  width: 52px;
}

.rail-toggle {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  padding: 0.85rem 1rem;
  border: none;
  border-bottom: 1px solid rgba(226, 232, 240, 0.9);
  background: transparent;
  color: #1e293b;
  font-size: 0.875rem;
  font-weight: 700;
  cursor: pointer;
}

.session-rail-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 1rem;
}

.quick-open-card {
  padding: 1rem;
  border-radius: 18px;
  background: linear-gradient(135deg, #0f172a 0%, #1d4ed8 100%);
  color: #eff6ff;
  box-shadow: 0 18px 40px rgba(15, 23, 42, 0.18);
}

.quick-open-title {
  font-size: 0.95rem;
  font-weight: 700;
}

.quick-open-desc {
  margin-top: 0.35rem;
  font-size: 0.8125rem;
  line-height: 1.5;
  color: rgba(219, 234, 254, 0.88);
}

.quick-open-actions {
  margin-top: 0.85rem;
  display: flex;
  gap: 0.5rem;
}

.quick-open-select,
.switcher-select {
  min-width: 0;
  padding: 0.55rem 0.7rem;
  border-radius: 12px;
  border: 1px solid rgba(148, 163, 184, 0.32);
  font-size: 0.8125rem;
}

.quick-open-select {
  flex: 1;
  background: rgba(255, 255, 255, 0.96);
  color: #0f172a;
}

.quick-open-btn,
.header-action {
  padding: 0.6rem 0.9rem;
  border: none;
  border-radius: 12px;
  background: #0f172a;
  color: #fff;
  font-size: 0.8125rem;
  font-weight: 600;
  cursor: pointer;
}

.quick-open-btn:disabled,
.header-action:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.header-action.secondary {
  background: rgba(15, 23, 42, 0.08);
  color: #334155;
}

.session-groups {
  margin-top: 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

.session-group-title {
  margin-bottom: 0.4rem;
  font-size: 0.75rem;
  font-weight: 700;
  color: #64748b;
  text-transform: uppercase;
  letter-spacing: 0.08em;
}

.session-item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.7rem;
  padding: 0.8rem 0.85rem;
  border: 1px solid rgba(226, 232, 240, 0.95);
  border-radius: 16px;
  background: #fff;
  text-align: left;
}

.session-item + .session-item {
  margin-top: 0.45rem;
}

.session-item:hover {
  transform: translateY(-1px);
  box-shadow: 0 10px 22px rgba(15, 23, 42, 0.08);
}

.session-item.active {
  border-color: rgba(37, 99, 235, 0.35);
  box-shadow: 0 14px 30px rgba(37, 99, 235, 0.12);
  background: linear-gradient(135deg, #ffffff 0%, #eff6ff 100%);
}

.session-item-main-button {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 0.7rem;
  padding: 0;
  border: none;
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.session-item-badge {
  flex-shrink: 0;
  min-width: 44px;
  padding: 0.2rem 0.45rem;
  border-radius: 999px;
  font-size: 0.6875rem;
  font-weight: 700;
  text-align: center;
}

.session-item-badge.host {
  background: rgba(245, 158, 11, 0.14);
  color: #b45309;
}

.session-item-badge.pod {
  background: rgba(34, 197, 94, 0.14);
  color: #15803d;
}

.session-item-main {
  flex: 1;
  min-width: 0;
}

.session-item-name {
  display: block;
  font-size: 0.84rem;
  font-weight: 600;
  color: #0f172a;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.session-item-meta {
  display: block;
  margin-top: 0.18rem;
  font-size: 0.75rem;
  color: #64748b;
}

.session-item-close {
  border: none;
  background: transparent;
  color: #94a3b8;
  font-size: 1rem;
  cursor: pointer;
}

.session-item-close:hover {
  color: #dc2626;
}

.session-empty {
  margin-top: 1rem;
  padding: 1rem;
  border-radius: 16px;
  background: rgba(248, 250, 252, 0.92);
  color: #64748b;
  font-size: 0.8125rem;
  line-height: 1.6;
}

.terminal-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.terminal-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
  padding: 1.1rem 1.25rem 1rem;
  border-bottom: 1px solid rgba(226, 232, 240, 0.9);
  background: rgba(255, 255, 255, 0.75);
  backdrop-filter: blur(14px);
}

.terminal-title {
  margin: 0.25rem 0 0;
  font-size: 1.4rem;
  line-height: 1.15;
  color: #0f172a;
}

.terminal-subtitle {
  margin: 0.3rem 0 0;
  font-size: 0.88rem;
  color: #64748b;
}

.terminal-header-actions {
  display: flex;
  gap: 0.6rem;
  flex-wrap: wrap;
}

.terminal-stage {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  padding: 1rem 1.1rem 1.1rem;
}

.terminal-context-bar {
  display: flex;
  align-items: center;
  gap: 0.8rem;
  padding: 0.75rem 0.9rem;
  border-radius: 18px 18px 0 0;
  background: linear-gradient(135deg, #0f172a 0%, #1e293b 100%);
}

.context-bar-spacer {
  flex: 1;
  min-width: 0;
}

.context-pill {
  padding: 0.28rem 0.6rem;
  border-radius: 999px;
  font-size: 0.72rem;
  font-weight: 700;
}

.context-pill.host {
  background: rgba(245, 158, 11, 0.18);
  color: #fde68a;
}

.context-pill.pod {
  background: rgba(34, 197, 94, 0.18);
  color: #bbf7d0;
}

.context-name-pill {
  max-width: min(42vw, 420px);
  padding: 0.42rem 0.8rem;
  border-radius: 14px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  background: rgba(255, 255, 255, 0.08);
  color: #f8fafc;
  font-size: 0.8rem;
  font-weight: 600;
  line-height: 1.2;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.05);
}

.context-name-pill.host {
  border-color: rgba(245, 158, 11, 0.24);
  background: rgba(245, 158, 11, 0.12);
  color: #fef3c7;
}

.context-name-pill.pod {
  border-color: rgba(34, 197, 94, 0.24);
  background: rgba(34, 197, 94, 0.12);
  color: #dcfce7;
}

.switcher-row {
  display: flex;
  align-items: center;
  gap: 0.45rem;
}

.switcher-label {
  font-size: 0.75rem;
  color: #cbd5e1;
}

.switcher-select {
  background: rgba(15, 23, 42, 0.72);
  color: #f8fafc;
  border-color: rgba(148, 163, 184, 0.32);
}

.context-action {
  flex-shrink: 0;
  padding: 0.5rem 0.85rem;
  border: 1px solid rgba(96, 165, 250, 0.42);
  border-radius: 12px;
  background: linear-gradient(135deg, rgba(37, 99, 235, 0.28), rgba(59, 130, 246, 0.18));
  color: #dbeafe;
  font-size: 0.8rem;
  font-weight: 600;
  cursor: pointer;
  box-shadow: inset 0 1px 0 rgba(191, 219, 254, 0.12);
  transition: background 0.15s, border-color 0.15s, color 0.15s, transform 0.15s;
}

.context-action:hover {
  background: linear-gradient(135deg, rgba(59, 130, 246, 0.4), rgba(37, 99, 235, 0.3));
  border-color: rgba(147, 197, 253, 0.58);
  color: #fff;
  transform: translateY(-1px);
}

.terminal-stage :deep(.pod-shell-terminal) {
  border-radius: 0 0 20px 20px;
  background: #0f172a;
}

.terminal-loading,
.terminal-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.65rem;
  padding: 2rem;
  color: #475569;
}

.terminal-loading {
  margin: 0 1.1rem 1.1rem;
  border-radius: 0 0 20px 20px;
  background: #0f172a;
  color: #cbd5e1;
}

.terminal-loading-hint,
.empty-desc {
  font-size: 0.82rem;
  color: inherit;
  opacity: 0.88;
}

.terminal-empty.error {
  color: #b91c1c;
}

.empty-actions {
  display: flex;
  gap: 0.6rem;
}

.empty-hero {
  text-align: center;
}

.empty-kicker {
  font-size: 0.72rem;
  font-weight: 700;
  color: #2563eb;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.empty-hero h3 {
  margin: 0.35rem 0 0;
  font-size: 1.55rem;
  color: #0f172a;
}

.empty-hero p {
  margin: 0.45rem 0 0;
  color: #64748b;
}

.empty-grid {
  margin-top: 1.4rem;
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 0.9rem;
  width: min(900px, 100%);
}

.empty-card {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
  padding: 1rem 1.05rem;
  border: 1px solid rgba(191, 219, 254, 0.9);
  border-radius: 18px;
  background: linear-gradient(135deg, #ffffff 0%, #eff6ff 100%);
  text-align: left;
  cursor: pointer;
}

.empty-card.secondary {
  border-color: rgba(226, 232, 240, 0.95);
  background: #fff;
}

.empty-card.info {
  cursor: default;
  border-color: rgba(226, 232, 240, 0.95);
  background: rgba(255, 255, 255, 0.92);
}

.empty-card-title {
  font-size: 0.9rem;
  font-weight: 700;
  color: #0f172a;
}

.empty-card-desc {
  font-size: 0.8125rem;
  color: #64748b;
  line-height: 1.55;
}

@media (max-width: 960px) {
  .terminal-center {
    flex-direction: column;
  }

  .session-rail,
  .session-rail.collapsed {
    width: 100%;
    border-right: none;
    border-bottom: 1px solid rgba(148, 163, 184, 0.22);
  }

  .terminal-header {
    flex-direction: column;
  }

  .terminal-context-bar {
    flex-wrap: wrap;
  }

  .context-name-pill {
    max-width: 100%;
  }

  .context-bar-spacer {
    display: none;
  }
}
</style>
