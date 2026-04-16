import { computed, ref } from "vue";
import { uid } from "../utils/uid";

export interface LogCenterSession {
  id: string;
  kind: "pod" | "workload";
  envId: string;
  envName: string;
  namespace: string;
  podName?: string;
  workloadKind?: string;
  workloadName?: string;
}

export const pendingLogOpen = ref<
  | {
      kind: "pod" | "workload";
      envId: string;
      envName: string;
      namespace: string;
      podName?: string;
      workloadKind?: string;
      workloadName?: string;
    }
  | null
>(null);

export const switchToLogCenterRequested = ref(0);

const sessions = ref<LogCenterSession[]>([]);
const currentSessionId = ref<string | null>(null);


export function useLogCenterStore() {
  const currentSession = computed(() =>
    currentSessionId.value
      ? sessions.value.find((item) => item.id === currentSessionId.value) ?? null
      : null
  );

  function findExistingSession(input: Omit<LogCenterSession, "id">): LogCenterSession | null {
    return (
      sessions.value.find((item) => {
        if (
          item.kind !== input.kind ||
          item.envId !== input.envId ||
          item.namespace !== input.namespace
        ) {
          return false;
        }
        if (item.kind === "pod") {
          return item.podName === input.podName;
        }
        return (
          item.workloadKind === input.workloadKind && item.workloadName === input.workloadName
        );
      }) ?? null
    );
  }

  function openOrFocusSession(input: Omit<LogCenterSession, "id">): string {
    const existing = findExistingSession(input);
    if (existing) {
      currentSessionId.value = existing.id;
      return existing.id;
    }
    const id = uid("log");
    sessions.value.unshift({ ...input, id });
    currentSessionId.value = id;
    return id;
  }

  function closeSession(id: string) {
    sessions.value = sessions.value.filter((item) => item.id !== id);
    if (currentSessionId.value === id) {
      currentSessionId.value = sessions.value[0]?.id ?? null;
    }
  }

  function setCurrentSession(id: string) {
    if (sessions.value.some((item) => item.id === id)) {
      currentSessionId.value = id;
    }
  }

  function clearPendingOpen() {
    pendingLogOpen.value = null;
  }

  function requestSwitchToLogCenter() {
    switchToLogCenterRequested.value += 1;
  }

  return {
    sessions,
    currentSessionId,
    currentSession,
    pendingLogOpen,
    clearPendingOpen,
    openOrFocusSession,
    closeSession,
    setCurrentSession,
    switchToLogCenterRequested,
    requestSwitchToLogCenter,
  };
}
