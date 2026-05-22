import { ref } from "vue";

/** 从快照中心跳转到工作台并打开资源详情。 */
export const pendingWorkbenchResource = ref<{
  envId: string;
  kind: string;
  name: string;
  namespace: string | null;
  initialTab?: string | null;
} | null>(null);

export const switchToSnapshotCenterRequested = ref(0);
export const switchToMainRequested = ref(0);

export function useSnapshotCenterStore() {
  function clearPendingWorkbenchResource() {
    pendingWorkbenchResource.value = null;
  }

  function openResourceInWorkbench(input: {
    envId: string;
    kind: string;
    name: string;
    namespace: string | null;
    initialTab?: string | null;
  }) {
    pendingWorkbenchResource.value = input;
    switchToMainRequested.value += 1;
  }

  function requestSwitchToSnapshotCenter() {
    switchToSnapshotCenterRequested.value += 1;
  }

  function requestSwitchToMain() {
    switchToMainRequested.value += 1;
  }

  return {
    pendingWorkbenchResource,
    switchToSnapshotCenterRequested,
    switchToMainRequested,
    clearPendingWorkbenchResource,
    openResourceInWorkbench,
    requestSwitchToSnapshotCenter,
    requestSwitchToMain,
  };
}
