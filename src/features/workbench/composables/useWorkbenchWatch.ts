import { ref } from "vue";
import type { ResourceKind } from "../../../constants/resourceKinds";
import { uid } from "../../../utils/uid";

export type WorkbenchWatchView = {
  kind: ResourceKind;
  namespace: string | null;
  labelSelector: string | null;
};

/**
 * 工作台资源列表 Watch：开关、各环境 token 与当前订阅视图，用于与 Tauri `resource-watch-update` 对齐。
 */
export function useWorkbenchWatch() {
  const watchEnabled = ref(true);
  const activeWatchTokens = ref<Record<string, string>>({});
  const activeWatchViews = ref<Record<string, WorkbenchWatchView>>({});

  function createWatchToken(prefix: string): string {
    return uid(prefix);
  }

  function setWatchToken(envId: string, token: string) {
    activeWatchTokens.value = { ...activeWatchTokens.value, [envId]: token };
  }

  function setWatchView(
    envId: string,
    kind: ResourceKind,
    namespace: string | null,
    labelSelector: string | null
  ) {
    activeWatchViews.value = {
      ...activeWatchViews.value,
      [envId]: { kind, namespace, labelSelector },
    };
  }

  function clearWatchToken(envId: string) {
    const nextTokens = { ...activeWatchTokens.value };
    delete nextTokens[envId];
    activeWatchTokens.value = nextTokens;
    const nextViews = { ...activeWatchViews.value };
    delete nextViews[envId];
    activeWatchViews.value = nextViews;
  }

  return {
    watchEnabled,
    activeWatchTokens,
    activeWatchViews,
    createWatchToken,
    setWatchToken,
    setWatchView,
    clearWatchToken,
  };
}
