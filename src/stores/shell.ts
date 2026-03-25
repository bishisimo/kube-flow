/**
 * 终端中心会话状态：统一维护 Pod / 主机会话。
 */
import { ref, computed } from "vue";
import type { HostShellBootstrap } from "../api/terminal";

export interface ShellSession {
  id: string;
  kind: "pod" | "host";
  envId: string;
  envName: string;
  namespace?: string;
  podName?: string;
  container?: string;
  streamId: string | null;
  status: "connecting" | "connected" | "reconnecting" | "disconnected" | "error" | "closed";
  error?: string;
  hostLabel?: string;
  bootstrapCommands?: string[];
  nodeTerminalLaunch?: HostShellBootstrap | null;
  /** 从 Workload 打开时，用于 Pod 切换器 */
  workloadKind?: string;
  workloadName?: string;
}

const sessions = ref<ShellSession[]>([]);
const currentSessionId = ref<string | null>(null);

/** 从工作台跳转时待打开的信息，终端中心读取后清空。 */
export const pendingOpen = ref<{
  kind: "pod" | "host";
  envId: string;
  envName: string;
  namespace?: string;
  hostLabel?: string;
  bootstrapCommands?: string[];
  nodeTerminalLaunch?: HostShellBootstrap | null;
  /** 直连 Pod 时必填 */
  podName?: string;
  container?: string;
  /** 从 Workload 打开时必填 */
  workloadKind?: string;
  workloadName?: string;
} | null>(null);

/** 请求切换到 Shell 界面（由工作台触发，Shell 父组件监听） */
export const switchToShellRequested = ref(0);

export function useShellStore() {
  const currentSession = computed(() =>
    currentSessionId.value
      ? sessions.value.find((s) => s.id === currentSessionId.value) ?? null
      : null
  );

  function addSession(session: Omit<ShellSession, "id" | "streamId" | "status">): string {
    const id = `shell-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
    sessions.value.push({
      ...session,
      id,
      streamId: null,
      status: "connecting",
    });
    currentSessionId.value = id;
    return id;
  }

  function updateSession(
    id: string,
    patch: Partial<
      Pick<ShellSession, "streamId" | "status" | "error" | "podName" | "container" | "hostLabel">
    >
  ) {
    const s = sessions.value.find((x) => x.id === id);
    if (s) Object.assign(s, patch);
  }

  function removeSession(id: string) {
    sessions.value = sessions.value.filter((s) => s.id !== id);
    if (currentSessionId.value === id) {
      currentSessionId.value = sessions.value[0]?.id ?? null;
    }
  }

  function setCurrent(id: string | null) {
    if (id === null || sessions.value.some((s) => s.id === id)) {
      currentSessionId.value = id;
    }
  }

  function clearPendingOpen() {
    pendingOpen.value = null;
  }

  function requestSwitchToShell() {
    switchToShellRequested.value += 1;
  }

  return {
    sessions,
    currentSessionId,
    currentSession,
    addSession,
    updateSession,
    removeSession,
    setCurrent,
    pendingOpen,
    clearPendingOpen,
    switchToShellRequested,
    requestSwitchToShell,
  };
}
