import { onMounted, onUnmounted, watch, type ComputedRef, type Ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import {
  kubeStartWatch,
  kubeStopWatch,
  type NamespaceItem,
  type ResolvedAliasTarget,
} from "../../../api/kube";
import { isConnectionError } from "../../../stores/connection";
import type { ResourceKind } from "../../../constants/resourceKinds";
import { getWorkbenchResourceDescriptor } from "../resourceDescriptors";
import { WORKBENCH_ALL_NAMESPACES_SENTINEL } from "../constants";
import { extractErrorMessage } from "../utils/extractErrorMessage";
import { handleAuthRetry, type StrongholdAuthLike, type SshAuthLike } from "../utils/handleAuthRetry";
import type { WorkbenchWatchView } from "./useWorkbenchWatch";

export type UseWorkbenchResourceWatchOptions = {
  currentId: Ref<string | null>;
  selectedNamespace: Ref<string | null>;
  selectedKind: Ref<ResourceKind>;
  selectedCustomTarget: Ref<ResolvedAliasTarget | null>;
  labelSelector: Ref<string>;
  watchEnabled: Ref<boolean>;
  listLoading: Ref<boolean>;
  listError: Ref<string | null>;
  envSwitching: Ref<boolean>;
  activeWatchTokens: Ref<Record<string, string>>;
  activeWatchViews: Ref<Record<string, WorkbenchWatchView>>;
  createWatchToken: (prefix: string) => string;
  setWatchToken: (envId: string, token: string) => void;
  setWatchView: (
    envId: string,
    kind: ResourceKind,
    namespace: string | null,
    labelSelector: string | null
  ) => void;
  clearWatchToken: (envId: string) => void;
  namespaceCache: Map<string, NamespaceItem[]>;
  namespaceOptions: Ref<NamespaceItem[]>;
  clearResourceCollections: () => void;
  setResourceItems: (kind: ResourceKind, items: unknown[]) => void;
  cacheCurrentView: (
    envId: string,
    kind: ResourceKind,
    namespace: string | null,
    labelSel: string | null,
    items: unknown[]
  ) => void;
  applyCachedView: (
    envId: string,
    kind: ResourceKind,
    namespace: string | null,
    labelSel: string | null
  ) => boolean;
  openedEnvs: ComputedRef<readonly { id: string }[]>;
  loadList: () => void | Promise<void>;
  refreshNamespaceOptions: () => void | Promise<void>;
  viewSessionId: Ref<number>;
  isStaleView: (envId: string, sessionId: number, requestId?: number) => boolean;
  strongholdAuth: StrongholdAuthLike;
  sshAuth: SshAuthLike;
  setConnecting: (envId: string) => void;
  setDisconnected: (envId: string, msg: string) => void;
};

/**
 * 工作台列表 Watch：订阅 Tauri `resource-watch-update`、应用/清理 token 与视图，并在视图切换时拉起或回退到 loadList。
 */
export function useWorkbenchResourceWatch(o: UseWorkbenchResourceWatchOptions) {
  let unlistenWatch: (() => void) | null = null;

  function applyWatch() {
    const id = o.currentId.value;
    if (!id) return;
    if (o.selectedCustomTarget.value) {
      kubeStopWatch(id).catch(() => {});
      void o.loadList();
      return;
    }
    const sessionId = o.viewSessionId.value;
    const watchToken = o.createWatchToken("watch");
    const descriptor = getWorkbenchResourceDescriptor(o.selectedKind.value);
    const ns =
      o.selectedKind.value === "namespaces" ||
      descriptor.capabilities.clusterScoped
        ? null
        : (o.selectedNamespace.value ?? WORKBENCH_ALL_NAMESPACES_SENTINEL);
    const labelSel = o.labelSelector.value.trim() || null;
    const hasCache = o.applyCachedView(id, o.selectedKind.value, ns, labelSel);
    o.setWatchToken(id, watchToken);
    o.setWatchView(id, o.selectedKind.value, ns, labelSel);
    o.listLoading.value = !hasCache;
    o.listError.value = null;
    o.envSwitching.value = false;
    if (!o.namespaceCache.has(id)) void o.refreshNamespaceOptions();
    kubeStopWatch(id).catch(() => {});
    if (o.watchEnabled.value && descriptor.capabilities.supportsWatch) {
      kubeStartWatch(id, o.selectedKind.value, ns, labelSel, watchToken).catch(async (e) => {
        if (o.isStaleView(id, sessionId)) return;
        const msg = extractErrorMessage(e);
        const authHandled = await handleAuthRetry({
          message: msg,
          envId: id,
          retry: () => {
            void applyWatch();
          },
          strongholdAuth: o.strongholdAuth,
          sshAuth: o.sshAuth,
          setConnecting: o.setConnecting,
        });
        if (authHandled) {
          return;
        }
        o.clearWatchToken(id);
        o.listError.value = msg;
        if (isConnectionError(msg)) o.setDisconnected(id, msg);
      });
    }
  }

  onMounted(async () => {
    unlistenWatch = await listen<{
      envId?: string;
      watchToken?: string;
      kind?: string;
      items?: unknown[];
      error?: string;
    }>("resource-watch-update", (ev) => {
      const payload = ev.payload;
      if (!payload) return;
      if (o.selectedCustomTarget.value) return;
      const envId = payload.envId;
      if (!envId) return;
      if (payload.watchToken !== o.activeWatchTokens.value[envId]) return;
      if (payload?.error) {
        if (envId === o.currentId.value) {
          o.listError.value = payload.error;
          if (isConnectionError(payload.error)) o.setDisconnected(envId, payload.error);
        }
        return;
      }
      const kind = payload?.kind;
      const items = payload?.items ?? [];
      if (!kind) return;
      const watchView = o.activeWatchViews.value[envId];
      if (!watchView) return;
      const resourceKind = watchView.kind;
      const namespace = watchView.namespace;
      const labelSel = watchView.labelSelector;
      if (resourceKind === "namespaces") {
        o.namespaceCache.set(envId, [...(items as NamespaceItem[])]);
      }
      o.cacheCurrentView(envId, resourceKind, namespace, labelSel, items);
      if (
        envId !== o.currentId.value ||
        resourceKind !== o.selectedKind.value ||
        namespace !==
          (o.selectedKind.value === "namespaces" ||
          getWorkbenchResourceDescriptor(o.selectedKind.value).capabilities.clusterScoped
            ? null
            : (o.selectedNamespace.value ?? WORKBENCH_ALL_NAMESPACES_SENTINEL)) ||
        labelSel !== (o.labelSelector.value.trim() || null)
      )
        return;
      o.listError.value = null;
      o.listLoading.value = false;
      o.envSwitching.value = false;
      o.clearResourceCollections();
      const cachedNamespaces = o.namespaceCache.get(envId);
      if (cachedNamespaces) {
        o.namespaceOptions.value = [...cachedNamespaces];
      }
      o.setResourceItems(resourceKind, items);
    });
  });

  onUnmounted(() => {
    unlistenWatch?.();
    unlistenWatch = null;
    for (const env of o.openedEnvs.value) {
      kubeStopWatch(env.id).catch(() => {});
      o.clearWatchToken(env.id);
    }
  });

  watch(
    [o.currentId, o.selectedNamespace, o.selectedKind, o.selectedCustomTarget],
    () => {
      const id = o.currentId.value;
      if (o.selectedCustomTarget.value) {
        if (id) kubeStopWatch(id).catch(() => {});
        void o.loadList();
        return;
      }
      if (o.watchEnabled.value && getWorkbenchResourceDescriptor(o.selectedKind.value).capabilities.supportsWatch) {
        applyWatch();
      } else {
        if (id) kubeStopWatch(id).catch(() => {});
        void o.loadList();
      }
    },
    { immediate: true }
  );

  watch(o.watchEnabled, () => {
    const id = o.currentId.value;
    if (!id) return;
    if (o.selectedCustomTarget.value) {
      kubeStopWatch(id).catch(() => {});
      void o.loadList();
      return;
    }
    if (o.watchEnabled.value) {
      if (getWorkbenchResourceDescriptor(o.selectedKind.value).capabilities.supportsWatch) {
        applyWatch();
      } else {
        void o.loadList();
      }
    } else {
      for (const env of o.openedEnvs.value) {
        kubeStopWatch(env.id).catch(() => {});
        o.clearWatchToken(env.id);
      }
      void o.loadList();
    }
  });

  return { applyWatch };
}
