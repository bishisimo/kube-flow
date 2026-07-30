<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { NAlert, NButton, NEmpty, NScrollbar, NSelect, NSpace, NTag, NTooltip } from "naive-ui";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { kfSpace } from "../kf";

defineOptions({ name: "PodShellView" });
import { useShellStore } from "../stores/shell";
import { useEnvStore } from "../stores/env";
import {
  kubeGetPodContainers,
  kubeListPodsForWorkload,
  kubePodExecStart,
  kubePodExecStop,
  kubePodFileDownload,
  kubePodFileUpload,
  kubeRemoveClient,
  type PodItem,
} from "../api/kube";
import { hostFileDownload, hostFileUpload, hostShellStart, hostShellStdin, hostShellStop } from "../api/terminal";
import {
  fileTransferCancel,
  runFileTransfer,
  type FileTransferProgress,
} from "../api/fileTransfer";
import { extractErrorMessage } from "../utils/errorMessage";
import { bytesToBase64, estimateTerminalSize } from "../utils/terminalBytes";
import { isConnectionError, useConnectionStore } from "../stores/connection";
import { useStrongholdAuthStore } from "../stores/strongholdAuth";
import { useAppSettingsStore } from "../stores/appSettings";
import PodShellTerminal from "../components/PodShellTerminal.vue";
import FileTransferDialog, {
  type FileTransferDirection,
} from "../components/FileTransferDialog.vue";
import { buildCompactRailItems } from "../utils/compactRail";

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
const { environments, currentId } = useEnvStore();
const { envConnectionState, envConnectionError, setDisconnected } = useConnectionStore();
const strongholdAuth = useStrongholdAuthStore();
const { terminalInstanceCacheLimit, ensureAppSettingsLoaded } = useAppSettingsStore();

const sessionRailCollapsed = ref(false);
const podOptions = ref<PodItem[]>([]);
const containerOptions = ref<string[]>([]);
const switcherLoading = ref(false);
const hostEntryEnvId = ref<string | null>(null);
const reconnectingSessionIds = new Set<string>();
const suppressEndStreamIds = new Set<string>();
const terminalActivationOrder = ref<string[]>([]);
const fileTransferBusy = ref(false);
const fileTransferStatus = ref<{ type: "success" | "error"; text: string } | null>(null);
const fileTransferDialogVisible = ref(false);
const fileTransferDirection = ref<FileTransferDirection>("upload");
const fileTransferInitialLocalPath = ref<string | null>(null);
const fileTransferProgress = ref<FileTransferProgress | null>(null);
const fileTransferError = ref<string | null>(null);
const activeTransferId = ref<string | null>(null);
const dropArmed = ref(false);
let unlistenDragDrop: (() => void) | null = null;

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

/** 当前会话对应环境在工作台连接层的状态（隧道 / API 等），与终端流 end 事件互补展示 */
const envConnectionAlert = computed(() => {
  const envId = currentSession.value?.envId;
  if (!envId) return null;
  void envConnectionState.value[envId];
  void envConnectionError.value[envId];
  const state = envConnectionState.value[envId] ?? "connected";
  const err = envConnectionError.value[envId]?.trim();
  if (state === "connected") return null;
  if (state === "connecting") {
    return {
      type: "info" as const,
      title: "环境正在连接",
      body: "集群或隧道建立中，此环境下终端可能暂时不可用。",
    };
  }
  if (state === "disconnected") {
    return {
      type: "warning" as const,
      title: "环境连接已断开",
      body: err
        ? err
        : "与工作台或集群的会话已中断。请在工作台对当前环境重连，或排障后再使用「重新连接」。",
    };
  }
  if (state === "error") {
    return { type: "error" as const, title: "环境连接异常", body: err || "请在工作台查看错误并尝试重连该环境。" };
  }
  return null;
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
const compactSessionItems = computed(() =>
  buildCompactRailItems(
    sessions.value.map((session) => ({
      id: session.id,
      label:
        session.kind === "host"
          ? session.envName
          : session.podName || session.workloadName || "Pod",
      context: session.kind === "host" ? session.hostLabel || session.envName : session.container || session.envName,
      fallback: session.kind === "host" ? "Host" : "Pod",
    }))
  )
);

function sessionBadge(session: (typeof sessions.value)[number]): string {
  return session.kind === "host" ? "主机" : "Pod";
}

function sessionLabel(session: (typeof sessions.value)[number]): string {
  if (session.kind === "host") return session.hostLabel || `${session.envName} 主机`;
  return `${session.podName || "-"}${session.container ? ` (${session.container})` : ""}`;
}

function sessionStatusLabel(session: (typeof sessions.value)[number]): string {
  if (session.status === "reconnecting") return "重连中";
  if (session.status === "connecting") return "连接中";
  if (session.status === "connected") return "已连接";
  if (session.status === "error") return "错误";
  return "已断开";
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

function isMissingShellError(message?: string): boolean {
  if (!message) return false;
  const m = message.toLowerCase();
  return (
    m.includes("容器内未发现可用 shell") ||
    m.includes("executable file not found") ||
    m.includes("not found in $path") ||
    m.includes("no such file or directory")
  );
}

function buildTerminalUnavailableMessage(
  session: (typeof sessions.value)[number],
  reason?: string
): string {
  if (isMissingShellError(reason)) {
    return "目标容器未提供可用 shell（/bin/sh）。请改用调试容器或节点终端。";
  }
  if (reason && isConnectionError(reason)) {
    return session.kind === "host"
      ? "主机连接已中断，请排查网络/隧道后手动重连。"
      : "集群连接已中断，请排查网络或 kube 连接后手动重连。";
  }
  if (reason) return reason;
  return session.kind === "host" ? "主机 Shell 连接已断开，请手动重连。" : "Shell 连接已断开，请手动重连。";
}

function clearReconnectState(sessionId: string) {
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

const fileTransferTargetLabel = computed(() => {
  const session = currentSession.value;
  if (!session) return "";
  if (session.kind === "host") {
    return `${session.envName} · ${session.hostLabel || "主机"}`;
  }
  const container = session.container ? ` / ${session.container}` : "";
  return `${session.envName} · ${session.namespace}/${session.podName}${container}`;
});

const fileTransferDefaultRemotePath = computed(() => {
  const session = currentSession.value;
  if (!session) return "";
  if (fileTransferDirection.value === "upload") {
    const name = fileTransferInitialLocalPath.value?.split(/[\\/]/).pop();
    if (session.kind === "pod") return name ? `/tmp/${name}` : "/tmp/";
    return name || "";
  }
  return "";
});

function openFileTransferDialog(direction: FileTransferDirection, initialLocalPath?: string | null) {
  const session = currentSession.value;
  if (!session?.streamId || fileTransferBusy.value) return;
  fileTransferDirection.value = direction;
  fileTransferInitialLocalPath.value = initialLocalPath ?? null;
  fileTransferProgress.value = null;
  fileTransferError.value = null;
  fileTransferDialogVisible.value = true;
}

function closeFileTransferDialog() {
  if (fileTransferBusy.value) return;
  fileTransferDialogVisible.value = false;
  fileTransferInitialLocalPath.value = null;
  fileTransferProgress.value = null;
  fileTransferError.value = null;
}

async function startFileTransfer(payload: {
  localPath: string;
  remotePath: string;
  overwrite: boolean;
}) {
  const session = currentSession.value;
  if (!session || fileTransferBusy.value) return;
  fileTransferBusy.value = true;
  fileTransferStatus.value = null;
  fileTransferError.value = null;
  fileTransferProgress.value = {
    transferId: "",
    transferredBytes: 0,
    totalBytes: null,
  };
  activeTransferId.value = null;
  try {
    const transferId = await runFileTransfer(
      async () => {
        if (session.kind === "host") {
          if (fileTransferDirection.value === "upload") {
            return hostFileUpload(session.envId, payload.localPath, payload.remotePath, payload.overwrite);
          }
          return hostFileDownload(session.envId, payload.remotePath, payload.localPath, payload.overwrite);
        }
        if (fileTransferDirection.value === "upload") {
          return kubePodFileUpload(
            session.envId,
            session.namespace || "default",
            session.podName || "",
            session.container || null,
            payload.localPath,
            payload.remotePath,
            payload.overwrite
          );
        }
        return kubePodFileDownload(
          session.envId,
          session.namespace || "default",
          session.podName || "",
          session.container || null,
          payload.remotePath,
          payload.localPath,
          payload.overwrite
        );
      },
      {
        onStarted: (id) => {
          activeTransferId.value = id;
          // 立刻展示进度条，避免等首个事件到来前空白
          if (!fileTransferProgress.value) {
            fileTransferProgress.value = {
              transferId: id,
              transferredBytes: 0,
              totalBytes: null,
            };
          }
        },
        onProgress: (progress) => {
          fileTransferProgress.value = progress;
        },
      }
    );
    activeTransferId.value = transferId;
    const doneText = fileTransferDirection.value === "upload" ? "上传完成" : "下载完成";
    fileTransferStatus.value = { type: "success", text: doneText };
    fileTransferDialogVisible.value = false;
    fileTransferInitialLocalPath.value = null;
  } catch (e) {
    const msg = extractErrorMessage(e);
    fileTransferError.value = msg;
    fileTransferStatus.value = { type: "error", text: msg };
  } finally {
    fileTransferBusy.value = false;
    activeTransferId.value = null;
    fileTransferProgress.value = null;
  }
}

async function cancelActiveFileTransfer() {
  const id = activeTransferId.value;
  if (!id) return;
  try {
    await fileTransferCancel(id);
  } catch {
    // 结束事件会带回取消结果
  }
}

function uploadFileForCurrentSession() {
  openFileTransferDialog("upload");
}

function downloadFileForCurrentSession() {
  openFileTransferDialog("download");
}

/** 启动 shell 前按当前终端舞台估算 PTY 尺寸。 */
function shellLaunchSize(): { cols: number; rows: number } {
  const el = document.querySelector(".terminal-stage") as HTMLElement | null;
  return estimateTerminalSize(el);
}

async function startHostSessionStream(sessionId: string): Promise<boolean> {
  const session = sessions.value.find((item) => item.id === sessionId);
  if (!session) return false;
  try {
    const { cols, rows } = shellLaunchSize();
    const streamId = await hostShellStart(
      session.envId,
      session.nodeTerminalLaunch ?? null,
      cols,
      rows
    );
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
      status: "error",
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
    const { cols, rows } = shellLaunchSize();
    const streamId = await kubePodExecStart(
      session.envId,
      session.namespace || "default",
      session.podName || "",
      session.container || null,
      cols,
      rows
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
      status: "error",
      error: msg,
    });
    return false;
  } finally {
    reconnectingSessionIds.delete(sessionId);
  }
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
    const { cols, rows } = shellLaunchSize();
    const streamId = await kubePodExecStart(envId, namespace, podName, container || null, cols, rows);
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
    const dataB64 = bytesToBase64(new TextEncoder().encode(text));
    hostShellStdin(streamId, dataB64).catch(() => {});
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
    const { cols, rows } = shellLaunchSize();
    const streamId = await hostShellStart(envId, nodeTerminalLaunch ?? null, cols, rows);
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
    updateSession(id, { status: "disconnected", error: msg });
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
    const { cols, rows } = shellLaunchSize();
    const streamId = await kubePodExecStart(
      session.envId,
      session.namespace,
      newPodName,
      container || null,
      cols,
      rows
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
    const { cols, rows } = shellLaunchSize();
    const streamId = await kubePodExecStart(
      session.envId,
      session.namespace,
      session.podName,
      newContainer || null,
      cols,
      rows
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
  if (payload.error && isConnectionError(payload.error)) {
    setDisconnected(session.envId, payload.error);
  }
  const fallbackError = buildTerminalUnavailableMessage(session, payload.error);
  updateSession(sessionId, {
    streamId: null,
    status: payload.error ? "error" : "disconnected",
    error: fallbackError,
  });
}

async function reconnectSessionNow(sessionId: string) {
  clearReconnectState(sessionId);
  updateSession(sessionId, { streamId: null, status: "reconnecting", error: "正在重连…" });
  const ok = await tryReconnectSession(sessionId, true);
  if (!ok) {
    const latest = sessions.value.find((item) => item.id === sessionId);
    const session = latest ?? sessions.value.find((item) => item.id === sessionId);
    updateSession(sessionId, {
      streamId: null,
      status: "error",
      error: session ? buildTerminalUnavailableMessage(session, latest?.error) : "重连失败，请稍后重试。",
    });
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

/**
 * 与「当前工作台选中的环境」对齐：有 currentId 且仍存在于环境列表时默认选它，避免曾用首项当默认值；
 * 无工作台当前环境时不默认任一项，由「请选择」再选，避免点错环境。
 */
watch(
  () => [hostEntryOptions.value.map((item) => item.id).join(","), currentId.value ?? ""] as const,
  () => {
    const ids = new Set(hostEntryOptions.value.map((item) => item.id));
    const cur = currentId.value;
    if (cur && ids.has(cur)) {
      hostEntryEnvId.value = cur;
      return;
    }
    hostEntryEnvId.value = null;
  },
  { immediate: true }
);

onMounted(() => {
  void ensureAppSettingsLoaded();
  if (pendingOpen.value) void handlePendingOpen();
  void getCurrentWebview()
    .onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") {
        dropArmed.value = Boolean(currentSession.value?.streamId) && !fileTransferBusy.value;
        return;
      }
      if (event.payload.type === "leave") {
        dropArmed.value = false;
        return;
      }
      if (event.payload.type !== "drop") return;
      dropArmed.value = false;
      const path = event.payload.paths?.[0];
      if (!path || !currentSession.value?.streamId || fileTransferBusy.value) return;
      openFileTransferDialog("upload", path);
    })
    .then((unlisten) => {
      unlistenDragDrop = unlisten;
    })
    .catch(() => {
      unlistenDragDrop = null;
    });
});

onUnmounted(() => {
  reconnectingSessionIds.clear();
  suppressEndStreamIds.clear();
  unlistenDragDrop?.();
  unlistenDragDrop = null;
});
</script>

<template>
  <div class="terminal-center">
    <aside class="session-rail" :class="{ collapsed: sessionRailCollapsed }">
      <NButton quaternary class="rail-toggle" @click="sessionRailCollapsed = !sessionRailCollapsed">
        <span>{{ sessionRailCollapsed ? "»" : "«" }}</span>
        <span v-if="!sessionRailCollapsed">会话</span>
      </NButton>
      <div v-if="sessionRailCollapsed" class="session-rail-compact">
        <NScrollbar class="session-scroll" trigger="hover">
          <div v-if="sessions.length" class="compact-session-list">
            <NTooltip v-for="session in sessions" :key="session.id" placement="right" :show-arrow="false">
              <template #trigger>
                <button
                  type="button"
                  class="compact-session-item"
                  :class="[
                    currentSessionId === session.id ? 'active' : '',
                    session.kind === 'host' ? 'host' : 'pod',
                  ]"
                  @click="setCurrent(session.id)"
                >
                  <span class="compact-session-label">
                    {{ compactSessionItems[session.id]?.shortLabel ?? sessionLabel(session) }}
                  </span>
                </button>
              </template>
              <div class="compact-session-tip">
                <div>{{ session.envName }}</div>
                <div>{{ sessionLabel(session) }}</div>
              </div>
            </NTooltip>
          </div>
          <div v-else class="compact-session-empty">暂无</div>
        </NScrollbar>
      </div>
      <div v-if="!sessionRailCollapsed" class="session-rail-body">
        <div class="quick-open-card">
          <div class="quick-open-title">打开指定环境终端</div>
          <div class="quick-open-desc">终端中心不依赖已打开环境，直接选择目标环境即可进入主机终端。</div>
          <NSpace v-bind="kfSpace.quickOpen" class="quick-open-actions">
            <NSelect
              v-model:value="hostEntryEnvId"
              :options="hostEntrySelectOptions"
              clearable
              placeholder="请选择环境"
              filterable
              class="quick-open-select-naive"
            />
            <NButton
              type="primary"
              class="quick-open-btn-naive"
              :disabled="!hostEntryEnvId"
              @click="hostEntryEnvId && openHostShellForEnv(hostEntryEnvId)"
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
                    <span class="session-item-meta">{{ sessionStatusLabel(session) }}</span>
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

      <NAlert
        v-if="envConnectionAlert"
        :type="envConnectionAlert.type"
        :title="envConnectionAlert.title"
        class="terminal-env-alert"
        :bordered="true"
      >
        {{ envConnectionAlert.body }}
      </NAlert>

      <section
        v-if="currentSession && (currentSession.streamId || currentSession.status === 'connecting' || currentSession.status === 'reconnecting')"
        class="terminal-stage"
        :class="{ 'is-drop-target': dropArmed }"
      >
        <div v-if="dropArmed" class="terminal-drop-hint">松开以上传文件到当前会话</div>
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
            <NSpace v-bind="kfSpace.buttonGroup" class="terminal-file-actions">
              <NTag
                v-if="fileTransferStatus"
                size="small"
                round
                :bordered="false"
                :type="fileTransferStatus.type"
                class="file-transfer-status"
              >{{ fileTransferStatus.text }}</NTag>
              <NButton
                size="small"
                :loading="fileTransferBusy"
                :disabled="!currentSession.streamId || fileTransferBusy"
                @click="uploadFileForCurrentSession"
              >上传</NButton>
              <NButton
                size="small"
                :loading="fileTransferBusy"
                :disabled="!currentSession.streamId || fileTransferBusy"
                @click="downloadFileForCurrentSession"
              >下载</NButton>
              <NButton size="small" @click="reconnectSessionNow(currentSession.id)">重新连接</NButton>
            </NSpace>
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
            v-if="hostEntryOptions.length"
            quaternary
            class="empty-card"
            :disabled="!hostEntryEnvId"
            @click="hostEntryEnvId && openHostShellForEnv(hostEntryEnvId)"
          >
            <span class="empty-card-title">打开指定环境终端</span>
            <span class="empty-card-desc">
              {{
                hostEntryEnvId
                  ? hostEntryOptions.find((env) => env.id === hostEntryEnvId)?.display_name
                  : "请选择环境"
              }}
            </span>
          </NButton>
          <div class="empty-card info">
            <span class="empty-card-title">从工作台进入 Pod</span>
            <span class="empty-card-desc">右键 Pod / Deployment / StatefulSet / DaemonSet 即可打开。</span>
          </div>
        </div>
      </section>
    </main>

    <FileTransferDialog
      :visible="fileTransferDialogVisible"
      :direction="fileTransferDirection"
      :target-label="fileTransferTargetLabel"
      :default-remote-path="fileTransferDefaultRemotePath"
      :initial-local-path="fileTransferInitialLocalPath"
      :transferring="fileTransferBusy"
      :progress="fileTransferProgress"
      :error="fileTransferError"
      @close="closeFileTransferDialog"
      @start="startFileTransfer"
      @cancel-transfer="cancelActiveFileTransfer"
    />
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
.terminal-file-actions {
  flex-shrink: 0;
}
.file-transfer-status {
  max-width: 260px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.session-rail-compact {
  flex: 1;
  min-height: 0;
}
.session-scroll {
  flex: 1;
  min-height: 0;
  margin-top: 1rem;
}
.compact-session-list {
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
  padding: 0.75rem 0.35rem;
}
.compact-session-item {
  position: relative;
  width: 100%;
  min-height: 38px;
  border: 1px solid var(--kf-border);
  border-radius: 10px;
  background: var(--kf-surface-strong);
  color: var(--kf-text-primary);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0.35rem 0.2rem;
  cursor: pointer;
}
.compact-session-item.host {
  color: var(--kf-warning);
}
.compact-session-item.pod {
  color: var(--kf-success);
}
.compact-session-item.active {
  border-color: color-mix(in srgb, var(--kf-primary) 40%, var(--kf-border));
  background: linear-gradient(135deg, var(--kf-surface-strong) 0%, var(--kf-primary-soft) 100%);
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--kf-primary) 10%, transparent);
}
.compact-session-item.active::before {
  content: "";
  position: absolute;
  left: -1px;
  top: 8px;
  bottom: 8px;
  width: 3px;
  border-radius: 999px;
  background: var(--kf-primary);
}
.compact-session-label {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.72rem;
  font-weight: 700;
}
.compact-session-tip {
  display: flex;
  flex-direction: column;
  gap: 0.18rem;
  font-size: 0.78rem;
  line-height: 1.45;
}
.compact-session-empty {
  padding: 0.75rem 0.25rem;
  text-align: center;
  font-size: 0.75rem;
  color: var(--kf-text-secondary);
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

.terminal-env-alert {
  flex-shrink: 0;
  margin: 0 1.25rem 0.75rem;
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
  position: relative;
}

.terminal-stage.is-drop-target {
  outline: 2px dashed color-mix(in srgb, var(--kf-primary) 55%, transparent);
  outline-offset: -6px;
  background: color-mix(in srgb, var(--kf-primary) 8%, transparent);
}

.terminal-drop-hint {
  position: absolute;
  inset: 0;
  z-index: 5;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--kf-primary);
  background: color-mix(in srgb, var(--kf-bg-elevated) 72%, transparent);
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

/* 深色内嵌条上的默认 Button 易沿用主文色（与 embed canvas 撞色）；显式用终端前景色 */
.terminal-context-bar :deep(.n-button) {
  color: var(--kf-embed-t-foreground);
}
.terminal-context-bar :deep(.n-button:not(:disabled):hover) {
  color: var(--kf-embed-t-foreground);
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
}

.terminal-empty:not(.error) {
  color: var(--kf-text-secondary);
}

.terminal-loading {
  margin: 0 1.1rem 1.1rem;
  border-radius: 0 0 20px 20px;
  background: var(--kf-embed-t-canvas);
  /* 与 body 主文色同 hex 的 canvas 上，必须用语义上的浅色，避免子节点未继承时与背景同色 */
  color: var(--kf-embed-t-foreground);
}

.terminal-loading > p:first-of-type {
  margin: 0;
  text-align: center;
  color: var(--kf-embed-t-foreground);
  font-size: 0.95rem;
  font-weight: 600;
}

.terminal-loading .terminal-loading-hint {
  font-size: 0.82rem;
  line-height: 1.55;
  color: var(--kf-embed-t-foreground-subtle);
  max-width: min(40rem, 100%);
  text-align: center;
}

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
