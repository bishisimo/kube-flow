import { ref, type Ref } from "vue";
import type { ResourceKind } from "../../../constants/resourceKinds";
import {
  WORKBENCH_MAX_RECENT_KINDS,
  WORKBENCH_NS_FAVORITES_KEY,
  WORKBENCH_NS_RECENT_KEY_PREFIX,
  WORKBENCH_RECENT_KINDS_KEY,
} from "../constants";
import { createStorage, type Storage } from "../../../utils/storage";

const nsFavoritesStorage = createStorage<string[]>({
  key: WORKBENCH_NS_FAVORITES_KEY,
  version: 1,
  fallback: [],
  migrate: (old) => (Array.isArray(old) ? (old as unknown[]).filter((v): v is string => typeof v === "string" && v.trim().length > 0) : []),
});

const recentKindsStorage = createStorage<string[]>({
  key: WORKBENCH_RECENT_KINDS_KEY,
  version: 1,
  fallback: [],
});

const nsRecentStorageCache = new Map<string, Storage<string[]>>();
function getNsRecentStorage(envId: string): Storage<string[]> {
  if (!nsRecentStorageCache.has(envId)) {
    nsRecentStorageCache.set(
      envId,
      createStorage<string[]>({
        key: `${WORKBENCH_NS_RECENT_KEY_PREFIX}${envId}`,
        version: 1,
        fallback: [],
        migrate: (old) => (Array.isArray(old) ? (old as unknown[]).filter((v): v is string => typeof v === "string" && v.trim().length > 0) : []),
      })
    );
  }
  return nsRecentStorageCache.get(envId)!;
}

export type UseWorkbenchRecentsOptions = {
  /** 当前环境 id；用于按环境读写「最近 namespace」 */
  currentId: Ref<string | null>;
  /** 合法的 ResourceKind id，用于校验从存储恢复的最近资源类型 */
  validKindIds: Set<string>;
};

/**
 * 工作台「收藏 / 最近」：命名空间收藏（全局）、按环境的最近 namespace、跨环境的最近资源类型。
 */
export function useWorkbenchRecents(options: UseWorkbenchRecentsOptions) {
  const favoriteNamespaces = ref<Set<string>>(new Set());
  const recentNamespaces = ref<string[]>([]);
  const recentKinds = ref<ResourceKind[]>([]);

  function loadFavoriteNamespaces(): Set<string> {
    return new Set(nsFavoritesStorage.read());
  }

  function persistFavoriteNamespaces() {
    nsFavoritesStorage.write(Array.from(favoriteNamespaces.value));
  }

  function loadRecentNamespaces(envId: string | null): string[] {
    if (!envId) return [];
    return getNsRecentStorage(envId).read();
  }

  function persistRecentNamespaces(envId: string | null) {
    if (!envId) return;
    getNsRecentStorage(envId).write(recentNamespaces.value.slice(0, 8));
  }

  function loadRecentKinds(): ResourceKind[] {
    return recentKindsStorage.read()
      .filter((v): v is ResourceKind => typeof v === "string" && options.validKindIds.has(v))
      .slice(0, WORKBENCH_MAX_RECENT_KINDS);
  }

  function persistRecentKinds() {
    recentKindsStorage.write(recentKinds.value.slice(0, WORKBENCH_MAX_RECENT_KINDS));
  }

  function touchRecentNamespace(ns: string | null) {
    if (!ns || !ns.trim()) return;
    recentNamespaces.value = [ns, ...recentNamespaces.value.filter((v) => v !== ns)].slice(0, 8);
    persistRecentNamespaces(options.currentId.value);
  }

  function toggleFavoriteNamespace(ns: string) {
    const next = new Set(favoriteNamespaces.value);
    if (next.has(ns)) next.delete(ns);
    else next.add(ns);
    favoriteNamespaces.value = next;
    persistFavoriteNamespaces();
  }

  function touchRecentKind(kind: ResourceKind) {
    recentKinds.value = [kind, ...recentKinds.value.filter((k) => k !== kind)].slice(0, WORKBENCH_MAX_RECENT_KINDS);
    persistRecentKinds();
  }

  /** 挂载时从 localStorage 恢复；需在 currentId 已就绪后调用 */
  function hydrateFromStorage() {
    favoriteNamespaces.value = loadFavoriteNamespaces();
    recentKinds.value = loadRecentKinds();
    recentNamespaces.value = loadRecentNamespaces(options.currentId.value);
  }

  /** 切换环境时加载该环境的最近 namespace 列表 */
  function loadRecentNamespacesForEnv(envId: string | null) {
    recentNamespaces.value = loadRecentNamespaces(envId);
  }

  return {
    favoriteNamespaces,
    recentNamespaces,
    recentKinds,
    touchRecentNamespace,
    toggleFavoriteNamespace,
    touchRecentKind,
    hydrateFromStorage,
    loadRecentNamespacesForEnv,
  };
}
