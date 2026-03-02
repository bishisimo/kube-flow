/**
 * Pod Shell 会话状态：维护已打开的 Pod Shell 连接列表，与工作台的已打开环境平级。
 */
import { ref, computed } from "vue";

export interface ShellSession {
  id: string;
  envId: string;
  envName: string;
  namespace: string;
  podName: string;
  container: string;
  streamId: string | null;
  status: "connecting" | "connected" | "error" | "closed";
  error?: string;
  /** 从 Workload 打开时，用于 Pod 切换器 */
  workloadKind?: string;
  workloadName?: string;
}

const sessions = ref<ShellSession[]>([]);
const currentSessionId = ref<string | null>(null);

/** 从工作台跳转时待打开的信息，Shell 界面读取后清空。支持 Pod 直连或 Workload（Deploy/STS） */
export const pendingOpen = ref<{
  envId: string;
  envName: string;
  namespace: string;
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
    patch: Partial<Pick<ShellSession, "streamId" | "status" | "error" | "podName" | "container">>
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
