import type { ComputedRef, Ref } from "vue";
import type { ResourceKind } from "../../../constants/resourceKinds";
import type { Environment } from "../../../api/env";
import { defaultNamespace } from "../../../api/env";
import {
  kubeListCrdInstances,
  kubeListNamespaces,
  type DynamicCrdInstanceItem,
  type NamespaceItem,
  type ResolvedAliasTarget,
} from "../../../api/kube";
import { isConnectionError } from "../../../stores/connection";
import { getWorkbenchResourceDescriptor } from "../resourceDescriptors";
import { WORKBENCH_ALL_NAMESPACES_SENTINEL } from "../constants";
import { extractErrorMessage } from "../../../utils/errorMessage";
import { handleAuthRetry, type StrongholdAuthLike, type SshAuthLike } from "../utils/handleAuthRetry";

export type UseWorkbenchLoadListOptions = {
  currentId: Ref<string | null>;
  currentEnv: ComputedRef<Environment | null>;
  selectedNamespace: Ref<string | null>;
  selectedKind: Ref<ResourceKind>;
  selectedCustomTarget: Ref<ResolvedAliasTarget | null>;
  labelSelector: Ref<string>;
  listError: Ref<string | null>;
  listLoading: Ref<boolean>;
  envSwitching: Ref<boolean>;
  latestListRequestId: Ref<number>;
  viewSessionId: Ref<number>;
  isStaleView: (envId: string, sessionId: number, requestId?: number) => boolean;
  getState: (envId: string) => string;
  setConnecting: (envId: string) => void;
  setConnected: (envId: string) => void;
  setDisconnected: (envId: string, msg: string) => void;
  touchEnv: (id: string) => Promise<void>;
  loadEnvironments: () => Promise<void>;
  namespaceCache: Map<string, NamespaceItem[]>;
  clearResourceCollections: () => void;
  namespaceOptions: Ref<NamespaceItem[]>;
  dynamicCrdItems: Ref<DynamicCrdInstanceItem[]>;
  setResourceItems: (kind: ResourceKind, items: unknown[]) => void;
  cacheCurrentView: (
    envId: string,
    kind: ResourceKind,
    namespace: string | null,
    labelSelector: string | null,
    items: unknown[]
  ) => void;
  applyCachedView: (
    envId: string,
    kind: ResourceKind,
    namespace: string | null,
    labelSelector: string | null
  ) => boolean;
  strongholdAuth: StrongholdAuthLike;
  sshAuth: SshAuthLike;
};

/**
 * 工作台资源列表拉取：内置/CRD、缓存命中、连接态与认证重试，与 Watch 路径并列。
 */
export function useWorkbenchLoadList(options: UseWorkbenchLoadListOptions) {
  async function loadList() {
    const id = options.currentId.value;
    if (!id) {
      options.clearResourceCollections();
      return;
    }
    const requestId = ++options.latestListRequestId.value;
    const sessionId = options.viewSessionId.value;
    const ns = options.selectedNamespace.value ?? WORKBENCH_ALL_NAMESPACES_SENTINEL;
    const labelSel = options.labelSelector.value.trim() || null;
    const crdTarget = options.selectedCustomTarget.value;
    options.listError.value = null;
    if (options.getState(id) !== "connected") options.setConnecting(id);

    if (crdTarget) {
      options.listLoading.value = options.dynamicCrdItems.value.length === 0;
      try {
        await options.touchEnv(id);
        if (options.isStaleView(id, sessionId, requestId)) return;
        await options.loadEnvironments();
        if (options.isStaleView(id, sessionId, requestId)) return;
        const nextNamespaces = await kubeListNamespaces(id, labelSel);
        if (options.isStaleView(id, sessionId, requestId)) return;
        options.namespaceCache.set(id, [...nextNamespaces]);
        const defaultNs = options.currentEnv.value
          ? defaultNamespace(options.currentEnv.value) ?? "default"
          : "default";
        const nsForCrd = crdTarget.namespaced
          ? ns === WORKBENCH_ALL_NAMESPACES_SENTINEL
            ? "__all__"
            : (options.selectedNamespace.value ?? defaultNs)
          : null;
        const items = await kubeListCrdInstances(id, {
          apiVersion: crdTarget.api_version,
          kind: crdTarget.kind,
          namespace: nsForCrd,
          labelSelector: labelSel,
        });
        if (options.isStaleView(id, sessionId, requestId)) return;
        options.clearResourceCollections();
        options.namespaceOptions.value = nextNamespaces;
        options.dynamicCrdItems.value = items;
        options.envSwitching.value = false;
        options.listLoading.value = false;
        options.setConnected(id);
      } catch (e) {
        if (options.isStaleView(id, sessionId, requestId)) return;
        const msg = extractErrorMessage(e);
        const authHandled = await handleAuthRetry({
          message: msg,
          envId: id,
          retry: () => {
            void loadList();
          },
          strongholdAuth: options.strongholdAuth,
          sshAuth: options.sshAuth,
          setConnecting: options.setConnecting,
        });
        if (authHandled) {
          return;
        }
        options.listError.value = msg;
        if (id && isConnectionError(msg)) {
          options.setDisconnected(id, msg);
        }
        options.clearResourceCollections();
        options.envSwitching.value = false;
      } finally {
        if (options.isStaleView(id, sessionId, requestId)) return;
        options.listLoading.value = false;
      }
      return;
    }

    const descriptor = getWorkbenchResourceDescriptor(options.selectedKind.value);
    const namespaceKey =
      options.selectedKind.value === "namespaces" || descriptor.capabilities.clusterScoped ? null : ns;
    const hasCache = options.applyCachedView(id, options.selectedKind.value, namespaceKey, labelSel);
    options.listLoading.value = !hasCache;
    try {
      await options.touchEnv(id);
      if (options.isStaleView(id, sessionId, requestId)) return;
      await options.loadEnvironments();
      if (options.isStaleView(id, sessionId, requestId)) return;
      const nextNamespaces = await kubeListNamespaces(id, labelSel);
      if (options.isStaleView(id, sessionId, requestId)) return;
      options.namespaceCache.set(id, [...nextNamespaces]);
      const applyResult = () => {
        options.clearResourceCollections();
        options.namespaceOptions.value = nextNamespaces;
        options.envSwitching.value = false;
        options.listLoading.value = false;
      };
      const targetNamespace = options.selectedKind.value === "namespaces" || descriptor.capabilities.clusterScoped ? null : ns;
      const items =
        options.selectedKind.value === "namespaces"
          ? nextNamespaces
          : await descriptor.fetchList(id, targetNamespace, labelSel);
      if (options.isStaleView(id, sessionId, requestId)) return;
      applyResult();
      options.setResourceItems(options.selectedKind.value, items);
      options.cacheCurrentView(id, options.selectedKind.value, targetNamespace, labelSel, items);
      if (options.isStaleView(id, sessionId, requestId)) return;
      options.setConnected(id);
    } catch (e) {
      if (options.isStaleView(id, sessionId, requestId)) return;
      const msg = extractErrorMessage(e);
      const authHandled = await handleAuthRetry({
        message: msg,
        envId: id,
        retry: () => {
          void loadList();
        },
        strongholdAuth: options.strongholdAuth,
        sshAuth: options.sshAuth,
        setConnecting: options.setConnecting,
      });
      if (authHandled) {
        return;
      }
      options.listError.value = msg;
      if (id && isConnectionError(msg)) {
        options.setDisconnected(id, msg);
      }
      options.clearResourceCollections();
      options.envSwitching.value = false;
    } finally {
      if (options.isStaleView(id, sessionId, requestId)) return;
      options.listLoading.value = false;
    }
  }

  return { loadList };
}
