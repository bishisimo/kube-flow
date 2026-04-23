<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { NButton, NEmpty, NScrollbar, NSelect, NSpace, NTag } from "naive-ui";
import { kfSpace } from "../kf";

defineOptions({ name: "PodShellView" });
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
import { extractErrorMessage } from "../utils/errorMessage";
import { isConnectionError } from "../stores/connection";
import { useStrongholdAuthStore } from "../stores/strongholdAuth";
import { useAppSettingsStore } from "../stores/appSettings";
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
const { terminalInstanceCacheLimit, ensureAppSettingsLoaded } = useAppSettingsStore();

const sessionRailCollapsed = ref(false);
const podOptions = ref<PodItem[]>([]);
const containerOptions = ref<string[]>([]);
const switcherLoading = ref(false);
const hostEntryEnvId = ref("");
const reconnectAttemptMap = ref<Record<string, number>>({});
const reconnectTimerMap = new Map<string, ReturnType<typeof setTimeout>>();
const reconnectingSessionIds = new Set<string>();
const suppressEndStreamIds = new Set<string>();
const terminalActivationOrder = ref<string[]>([]);

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
const hostEntrySelectOptions = computed(() =>
  hostEntryOptions.value.map((e) => ({ label: e.display_name, value: e.id }))
);
const podSelectOptions = computed(() =>
  podOptions.value.map((p) => ({ label: p.name, value: p.name }))
);
const containerSelectOptions = computed(() =>
  containerOptions.value.map((c) => ({ label: c, value: c }))
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

const hasMountedTerminalSession = computed(() => sessions.value.some((session) => Boolean(session.streamId)));
const visibleTerminalSessions = computed(() => {
  const limit = Math.max(1, terminalInstanceCacheLimit.value || 6);
  const connected = sessions.value.filter((session) => Boolean(session.streamId));
  if (!connected.length) return [];
  const order = terminalActivationOrder.value;
  const rank = new Map<string, number>();
  order.forEach((id, index) => rank.set(id, index));
  const sorted = [...connected].sort((a, b) => {
    const aRank = rank.get(a.id) ?? Number.MAX_SAFE_INTEGER;
    const bRank = rank.get(b.id) ?? Number.MAX_SAFE_INTEGER;
    return aRank - bRank;
  });
  return sorted.slice(0, limit);
});

function sessionBadge(session: (typeof sessions.value)[number]): string {
  return session.kind === "host" ? "主机" : "Pod";
}

function sessionLabel(session: (typeof sessions.value)[number]): string {
  if (session.kind === "host") return session.hostLabel || `${session.envName} 主机`;
  return `${session.podName || "-"}${session.container ? ` (${session.container})` : ""}`;
}

async function handleStrongholdLocked(message: string, onConfirmed: () => void): Promise<boolean> {
  return strongholdAuth.checkAndHandle(message, onConfirmed, {
    title: "解锁终端凭证",
    description: "当前终端会话需要访问已保存凭证，请先输入 Stronghold 主密码解锁。",
  });
}


function isNonRetryableTerminalError(message?: string): boolean {
  if (!message) return false;
  const m = message.toLowerCase();
  return [
    "未能解析容器 pid",
    "尚未运行",
    "缺少有效 containerid",
    "未找到容器",
    "unsupported",
    "节点终端策略",
    "至少需要一个步骤",
    "缺少 host",
    "缺少 user",
    "permission denied",
    "operation not permitted",
    "no such file or directory",
    "not found",
    "invalid",
    "shell exited with status 1",
    "shell exited with status 126",
    "shell exited with status 127",
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

function touchTerminalSession(sessionId: string | null) {
  if (!sessionId) return;
  const next = terminalActivationOrder.value.filter((id) => id !== sessionId);
  next.unshift(sessionId);
  terminalActivationOrder.value = next;
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
  if (isNonRetryableTerminalError(reason)) {
    updateSession(sessionId, {
      streamId: null,
      status: "error",
      error: reason ?? "终端启动失败",
    });
    clearReconnectState(sessionId);
    return;
  }
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
    const resetClient = isConnectionError(reason ?? "");
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
  if (isNonRetryableTerminalError(payload.error)) {
    updateSession(sessionId, {
      streamId: null,
      status: "error",
      error: payload.error ?? "终端启动失败",
    });
    clearReconnectState(sessionId);
    return;
  }
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
    touchTerminalSession(session?.id ?? null);
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
  () => sessions.value.map((session) => `${session.id}:${session.streamId ?? ""}`).join("|"),
  () => {
    const activeIds = new Set(sessions.value.filter((session) => Boolean(session.streamId)).map((session) => session.id));
    terminalActivationOrder.value = terminalActivationOrder.value.filter((id) => activeIds.has(id));
    if (currentSession.value?.streamId) {
      touchTerminalSession(currentSession.value.id);
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
  void ensureAppSettingsLoaded();
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
      <NButton quaternary class="rail-toggle" @click="sessionRailCollapsed = !sessionRailCollapsed">
        <span>{{ sessionRailCollapsed ? "»" : "«" }}</span>
        <span v-if="!sessionRailCollapsed">会话</span>
      </NButton>
      <div v-if="!sessionRailCollapsed" class="session-rail-body">
        <div class="quick-open-card">
          <div class="quick-open-title">打开指定环境终端</div>
          <div class="quick-open-desc">终端中心不依赖已打开环境，直接选择目标环境即可进入主机终端。</div>
          <NSpace v-bind="kfSpace.quickOpen" class="quick-open-actions">
            <NSelect
              v-model:value="hostEntryEnvId"
              :options="hostEntrySelectOptions"
              placeholder="选择环境"
              filterable
              class="quick-open-select-naive"
            />
            <NButton
              type="primary"
              class="quick-open-btn-naive"
              :disabled="!hostEntryEnvId"
              @click="openHostShellForEnv(hostEntryEnvId)"
            >打开终端</NButton>
          </NSpace>
        </div>

        <NScrollbar v-if="groupedSessions.length" class="session-scroll" trigger="hover">
          <div class="session-groups">
            <section v-for="group in groupedSessions" :key="group.envId" class="session-group">
              <div class="session-group-title">{{ group.envName }}</div>
              <div
                v-for="session in group.items"
                :key="session.id"
                class="session-item"
                :class="{ active: currentSessionId === session.id }"
              >
                <NButton quaternary class="session-item-main-button" @click="setCurrent(session.id)">
                  <NTag
                    size="small"
                    round
                    :bordered="false"
                    :class="session.kind === 'host' ? 'badge-host' : 'badge-podsh'"
                  >{{ sessionBadge(session) }}</NTag>
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
                </NButton>
                <NButton text class="session-item-close" @click.stop="closeSession(session.id)">×</NButton>
              </div>
            </section>
          </div>
        </NScrollbar>
        <NEmpty
          v-else
          class="session-empty"
          description="还没有会话。你可以从工作台打开 Pod，也可以直接在这里打开指定环境终端。"
        />
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
          <NSpace v-bind="kfSpace.contextBar" class="terminal-context-row">
            <NSpace v-bind="kfSpace.terminalContextMain" class="terminal-context-main">
              <NTag
                size="small"
                round
                :bordered="false"
                :class="currentSession.kind === 'host' ? 'ctx-pill-host' : 'ctx-pill-podsh'"
              >{{ currentSession.kind === "host" ? "主机 Shell" : "Pod Shell" }}</NTag>
              <NTag
                v-if="currentSessionContextName"
                size="small"
                round
                :bordered="false"
                class="context-name-naive"
                :class="currentSession.kind"
              >{{ currentSessionContextName }}</NTag>
              <div v-if="currentSession.kind === 'pod' && currentSession.workloadKind && podOptions.length > 1" class="switcher-row">
                <span class="switcher-label">Pod</span>
                <NSelect
                  :value="currentSession.podName"
                  :options="podSelectOptions"
                  :disabled="switcherLoading || currentSession.status === 'connecting' || currentSession.status === 'reconnecting'"
                  size="small"
                  class="switcher-naive"
                  @update:value="(v) => v && switchPod(String(v))"
                />
              </div>
              <div v-if="currentSession.kind === 'pod' && containerOptions.length > 1" class="switcher-row">
                <span class="switcher-label">容器</span>
                <NSelect
                  :value="currentSession.container"
                  :options="containerSelectOptions"
                  :disabled="currentSession.status === 'connecting' || currentSession.status === 'reconnecting'"
                  size="small"
                  class="switcher-naive"
                  @update:value="(v) => v && switchContainer(String(v))"
                />
              </div>
            </NSpace>
            <NButton size="small" @click="reconnectSessionNow(currentSession.id)">重新连接</NButton>
          </NSpace>
        </div>

        <div v-if="hasMountedTerminalSession" class="terminal-stack">
          <PodShellTerminal
            v-for="session in visibleTerminalSessions"
            v-show="session.id === currentSession.id && session.streamId"
            :key="`${session.id}:${session.streamId}`"
            :stream-id="session.streamId"
            :mode="session.kind"
            :active="session.id === currentSession.id"
            @end="onTerminalEnd(session.id, $event)"
          />
        </div>

        <div v-if="!currentSession.streamId" class="terminal-loading">
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
        <NSpace v-bind="kfSpace.centeredActions" class="empty-actions">
          <NButton type="primary" @click="reconnectSessionNow(currentSession.id)">立即重连</NButton>
          <NButton secondary @click="closeSession(currentSession.id)">关闭会话</NButton>
        </NSpace>
      </section>

      <section v-else class="terminal-empty">
        <div class="empty-hero">
          <div class="empty-kicker">终端工作区</div>
          <h3>同时管理 Pod 与主机会话</h3>
          <p>从工作台、资源详情，或者直接在这里选择环境打开终端，会话都会统一收纳在这里。</p>
        </div>
        <div class="empty-grid">
          <NButton
            v-if="hostEntryEnvId"
            quaternary
            class="empty-card"
            @click="openHostShellForEnv(hostEntryEnvId)"
          >
            <span class="empty-card-title">打开指定环境终端</span>
            <span class="empty-card-desc">
              {{ hostEntryOptions.find((env) => env.id === hostEntryEnvId)?.display_name || "选择环境" }}
            </span>
          </NButton>
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
    radial-gradient(circle at top left, color-mix(in srgb, var(--kf-primary) 12%, transparent), transparent 28%),
    linear-gradient(180deg, var(--wb-panel-soft) 0%, var(--kf-bg-elevated) 100%);
}

.session-rail {
  width: 320px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-right: 1px solid var(--kf-border);
  background: var(--kf-surface);
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
  border-bottom: 1px solid var(--kf-border);
  background: transparent;
  color: var(--kf-text-primary);
  font-size: 0.875rem;
  font-weight: 700;
  cursor: pointer;
}

.session-rail-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 1rem;
}
.session-scroll {
  flex: 1;
  min-height: 0;
  margin-top: 1rem;
}
.quick-open-select-naive {
  flex: 1;
  min-width: 0;
}
.session-item-main-button {
  flex: 1;
  min-width: 0;
  height: auto !important;
}
.session-item-main-button :deep(.n-button__content) {
  display: flex;
  align-items: center;
  gap: 0.7rem;
  width: 100%;
  min-width: 0;
  justify-content: flex-start;
}
.badge-host {
  min-width: 44px;
  text-align: center;
  background: color-mix(in srgb, var(--kf-warning) 16%, transparent) !important;
  color: var(--kf-warning) !important;
}
.badge-podsh {
  min-width: 44px;
  text-align: center;
  background: color-mix(in srgb, var(--kf-success) 16%, transparent) !important;
  color: var(--kf-success) !important;
}
.ctx-pill-host {
  background: color-mix(in srgb, var(--kf-warning) 20%, transparent) !important;
  color: var(--kf-embed-t-warn-glow) !important;
  font-weight: 700;
}
.ctx-pill-podsh {
  background: color-mix(in srgb, var(--kf-success) 20%, transparent) !important;
  color: var(--kf-embed-t-ok-glow) !important;
  font-weight: 700;
}
.context-name-naive.host {
  border: 1px solid color-mix(in srgb, var(--kf-warning) 30%, var(--kf-embed-t-canvas)) !important;
  background: color-mix(in srgb, var(--kf-warning) 14%, var(--kf-embed-t-canvas)) !important;
  color: var(--kf-embed-t-warn-ink) !important;
}
.context-name-naive.pod {
  border: 1px solid color-mix(in srgb, var(--kf-success) 30%, var(--kf-embed-t-canvas)) !important;
  background: color-mix(in srgb, var(--kf-success) 14%, var(--kf-embed-t-canvas)) !important;
  color: var(--kf-embed-t-ok-ink) !important;
}
.switcher-naive {
  min-width: 120px;
  max-width: min(42vw, 280px);
}
.empty-card.n-button {
  height: auto;
  align-items: flex-start;
}

.quick-open-card {
  padding: 1rem;
  border-radius: 18px;
  background: linear-gradient(135deg, var(--kf-embed-t-canvas) 0%, var(--wb-chip-text) 100%);
  color: var(--kf-embed-t-foreground);
  box-shadow: var(--kf-shadow-md);
}

.quick-open-title {
  font-size: 0.95rem;
  font-weight: 700;
}

.quick-open-desc {
  margin-top: 0.35rem;
  font-size: 0.8125rem;
  line-height: 1.5;
  color: color-mix(in srgb, var(--kf-embed-t-blue-ink) 88%, transparent);
}

.quick-open-actions {
  margin-top: 0.85rem;
  width: 100%;
}
.quick-open-actions :deep(.n-space-item:first-child) {
  flex: 1;
  min-width: 0;
}

.quick-open-select,
.switcher-select {
  min-width: 0;
  padding: 0.55rem 0.7rem;
  border-radius: 12px;
  border: 1px solid var(--kf-border-strong);
  font-size: 0.8125rem;
}

.quick-open-select {
  flex: 1;
  background: color-mix(in srgb, var(--kf-mix-surface) 96%, transparent);
  color: var(--kf-text-primary);
}

.quick-open-btn,
.header-action {
  padding: 0.6rem 0.9rem;
  border: none;
  border-radius: 12px;
  background: var(--kf-embed-t-canvas);
  color: var(--kf-embed-t-foreground);
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
  background: color-mix(in srgb, var(--kf-text-primary) 6%, var(--kf-mix-surface));
  color: var(--kf-text-secondary);
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
  color: var(--kf-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.08em;
}

.session-item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.7rem;
  padding: 0.8rem 0.85rem;
  border: 1px solid var(--kf-border);
  border-radius: 16px;
  background: var(--kf-surface-strong);
  text-align: left;
}

.session-item + .session-item {
  margin-top: 0.45rem;
}

.session-item:hover {
  transform: translateY(-1px);
  box-shadow: var(--kf-shadow-sm);
}

.session-item.active {
  border-color: color-mix(in srgb, var(--kf-primary) 40%, var(--kf-border));
  box-shadow: 0 14px 30px color-mix(in srgb, var(--kf-primary) 12%, transparent);
  background: linear-gradient(135deg, var(--kf-surface-strong) 0%, var(--kf-primary-soft) 100%);
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
  background: color-mix(in srgb, var(--kf-warning) 16%, transparent);
  color: var(--kf-warning);
}

.session-item-badge.pod {
  background: color-mix(in srgb, var(--kf-success) 16%, transparent);
  color: var(--kf-success);
}

.session-item-main {
  flex: 1;
  min-width: 0;
}

.session-item-name {
  display: block;
  font-size: 0.84rem;
  font-weight: 600;
  color: var(--kf-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.session-item-meta {
  display: block;
  margin-top: 0.18rem;
  font-size: 0.75rem;
  color: var(--kf-text-secondary);
}

.session-item-close {
  border: none;
  background: transparent;
  color: var(--kf-text-muted);
  font-size: 1rem;
  cursor: pointer;
}

.session-item-close:hover {
  color: var(--kf-danger);
}

.session-empty {
  margin-top: 1rem;
  padding: 1rem;
  border-radius: 16px;
  background: var(--kf-bg-soft);
  color: var(--kf-text-secondary);
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
  border-bottom: 1px solid var(--kf-border);
  background: color-mix(in srgb, var(--kf-surface-strong) 75%, transparent);
  backdrop-filter: blur(14px);
}

.terminal-title {
  margin: 0.25rem 0 0;
  font-size: 1.4rem;
  line-height: 1.15;
  color: var(--kf-text-primary);
}

.terminal-subtitle {
  margin: 0.3rem 0 0;
  font-size: 0.88rem;
  color: var(--kf-text-secondary);
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
  padding: 0.75rem 0.9rem;
  border-radius: 18px 18px 0 0;
  background: linear-gradient(135deg, var(--kf-embed-t-canvas) 0%, var(--kf-embed-t-slate) 100%);
}
.terminal-context-row {
  width: 100%;
  min-width: 0;
}
.terminal-context-main {
  min-width: 0;
}

.context-pill {
  padding: 0.28rem 0.6rem;
  border-radius: 999px;
  font-size: 0.72rem;
  font-weight: 700;
}

.context-pill.host {
  background: color-mix(in srgb, var(--kf-warning) 20%, transparent);
  color: var(--kf-embed-t-warn-glow);
}

.context-pill.pod {
  background: color-mix(in srgb, var(--kf-success) 20%, transparent);
  color: var(--kf-embed-t-ok-glow);
}

.context-name-pill {
  max-width: min(42vw, 420px);
  padding: 0.42rem 0.8rem;
  border-radius: 14px;
  border: 1px solid var(--kf-border);
  background: color-mix(in srgb, var(--kf-mix-surface) 8%, var(--kf-embed-t-canvas));
  color: var(--kf-embed-t-foreground);
  font-size: 0.8rem;
  font-weight: 600;
  line-height: 1.2;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  box-shadow: inset 0 1px 0 color-mix(in srgb, var(--kf-mix-surface) 5%, var(--kf-embed-t-canvas));
}

.context-name-pill.host {
  border-color: color-mix(in srgb, var(--kf-warning) 30%, var(--kf-embed-t-canvas));
  background: color-mix(in srgb, var(--kf-warning) 14%, var(--kf-embed-t-canvas));
  color: var(--kf-embed-t-warn-ink);
}

.context-name-pill.pod {
  border-color: color-mix(in srgb, var(--kf-success) 30%, var(--kf-embed-t-canvas));
  background: color-mix(in srgb, var(--kf-success) 14%, var(--kf-embed-t-canvas));
  color: var(--kf-embed-t-ok-ink);
}

.switcher-row {
  display: flex;
  align-items: center;
  gap: 0.45rem;
}

.switcher-label {
  font-size: 0.75rem;
  color: var(--kf-embed-t-foreground-subtle);
}

.switcher-select {
  background: color-mix(in srgb, var(--kf-embed-t-canvas) 72%, var(--kf-embed-t-slate));
  color: var(--kf-embed-t-foreground);
  border-color: var(--kf-border-strong);
}

.context-action {
  flex-shrink: 0;
  padding: 0.5rem 0.85rem;
  border: 1px solid color-mix(in srgb, var(--wb-chip-text) 45%, var(--kf-embed-t-canvas));
  border-radius: 12px;
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--kf-primary) 30%, var(--kf-embed-t-canvas)),
    color-mix(in srgb, var(--kf-primary) 20%, var(--kf-embed-t-canvas))
  );
  color: var(--kf-embed-t-blue-ink);
  font-size: 0.8rem;
  font-weight: 600;
  cursor: pointer;
  box-shadow: inset 0 1px 0 color-mix(in srgb, var(--kf-embed-t-blue-ink) 12%, transparent);
  transition: background 0.15s, border-color 0.15s, color 0.15s, transform 0.15s;
}

.context-action:hover {
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--kf-primary) 42%, var(--kf-embed-t-canvas)),
    color-mix(in srgb, var(--kf-primary) 32%, var(--kf-embed-t-canvas))
  );
  border-color: color-mix(in srgb, var(--wb-chip-text) 60%, var(--kf-embed-t-canvas));
  color: var(--kf-embed-t-foreground);
  transform: translateY(-1px);
}

.terminal-stage :deep(.pod-shell-terminal) {
  border-radius: 0 0 20px 20px;
  background: var(--kf-embed-t-canvas);
}

.terminal-stack {
  flex: 1;
  min-height: 0;
  display: flex;
}

.terminal-stack :deep(.pod-shell-terminal) {
  flex: 1;
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
  color: var(--kf-text-secondary);
}

.terminal-loading {
  margin: 0 1.1rem 1.1rem;
  border-radius: 0 0 20px 20px;
  background: var(--kf-embed-t-canvas);
  color: var(--kf-embed-t-foreground-subtle);
}

.terminal-loading-hint,
.empty-desc {
  font-size: 0.82rem;
  color: inherit;
  opacity: 0.88;
}

.terminal-empty.error {
  color: var(--kf-danger);
}

.empty-actions {
  width: 100%;
}

.empty-hero {
  text-align: center;
}

.empty-kicker {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--kf-primary);
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.empty-hero h3 {
  margin: 0.35rem 0 0;
  font-size: 1.55rem;
  color: var(--kf-text-primary);
}

.empty-hero p {
  margin: 0.45rem 0 0;
  color: var(--kf-text-secondary);
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
  border: 1px solid color-mix(in srgb, var(--kf-primary) 22%, var(--kf-border));
  border-radius: 18px;
  background: linear-gradient(135deg, var(--kf-surface-strong) 0%, var(--kf-primary-soft) 100%);
  text-align: left;
  cursor: pointer;
}

.empty-card.secondary {
  border-color: var(--kf-border);
  background: var(--kf-surface-strong);
}

.empty-card.info {
  cursor: default;
  border-color: var(--kf-border);
  background: var(--kf-surface);
}

.empty-card-title {
  font-size: 0.9rem;
  font-weight: 700;
  color: var(--kf-text-primary);
}

.empty-card-desc {
  font-size: 0.8125rem;
  color: var(--kf-text-secondary);
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
    border-bottom: 1px solid var(--kf-border);
  }

  .terminal-header {
    flex-direction: column;
  }

  .terminal-context-row {
    align-items: flex-start;
  }

  .context-name-pill {
    max-width: 100%;
  }
}
</style>
