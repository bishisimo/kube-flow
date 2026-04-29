import { ref } from "vue";
import type { ResolvedAliasTarget } from "../../../api/kube";
import type { ResourceKind } from "../../../constants/resourceKinds";
import { WORKBENCH_FAVORITE_KINDS_KEY } from "../constants";
import { createStorage } from "../../../utils/storage";
import { extensionStableKey } from "../builtinGvk";

export type FavoriteKindEntry =
  | { kind: "builtin"; id: ResourceKind }
  | { kind: "extension"; target: ResolvedAliasTarget };

type StoredRow =
  | { t: "b"; id: ResourceKind }
  | {
      t: "e";
      group: string;
      version: string;
      api_version: string;
      kind: string;
      plural: string;
      namespaced: boolean;
      short_names?: string[];
      singular?: string | null;
    };

function toStored(e: FavoriteKindEntry): StoredRow {
  if (e.kind === "builtin") return { t: "b", id: e.id };
  const x = e.target;
  return {
    t: "e",
    group: x.group,
    version: x.version,
    api_version: x.api_version,
    kind: x.kind,
    plural: x.plural,
    namespaced: x.namespaced,
    short_names: x.short_names,
    singular: x.singular ?? null,
  };
}

function fromStored(r: StoredRow, validKindIds: Set<string>): FavoriteKindEntry | null {
  if (r.t === "b") {
    if (!validKindIds.has(r.id)) return null;
    return { kind: "builtin", id: r.id };
  }
  return {
    kind: "extension",
    target: {
      group: r.group,
      version: r.version,
      api_version: r.api_version,
      kind: r.kind,
      plural: r.plural,
      namespaced: r.namespaced,
      short_names: r.short_names ?? [],
      singular: r.singular ?? null,
    },
  };
}

const favoriteKindsStorage = createStorage<StoredRow[]>({
  key: WORKBENCH_FAVORITE_KINDS_KEY,
  version: 1,
  fallback: [],
  migrate: (old) =>
    Array.isArray(old) ? (old as unknown[]).filter((x): x is StoredRow => x != null && typeof x === "object") : [],
});

export type UseWorkbenchFavoriteKindsOptions = {
  validKindIds: Set<string>;
};

/**
 * 工作台资源类型收藏：顺序即展示顺序，持久化到 localStorage。
 */
export function useWorkbenchFavoriteKinds(options: UseWorkbenchFavoriteKindsOptions) {
  const favoriteKindEntries = ref<FavoriteKindEntry[]>([]);

  function persist() {
    favoriteKindsStorage.write(favoriteKindEntries.value.map(toStored));
  }

  function hydrateFavoriteKinds() {
    const raw = favoriteKindsStorage.read();
    const out: FavoriteKindEntry[] = [];
    for (const row of raw) {
      const e = fromStored(row, options.validKindIds);
      if (e) out.push(e);
    }
    favoriteKindEntries.value = out;
  }

  function isFavoriteBuiltin(id: ResourceKind): boolean {
    return favoriteKindEntries.value.some((e) => e.kind === "builtin" && e.id === id);
  }

  function isFavoriteExtension(t: ResolvedAliasTarget): boolean {
    const k = extensionStableKey(t);
    return favoriteKindEntries.value.some((e) => e.kind === "extension" && extensionStableKey(e.target) === k);
  }

  function toggleFavoriteBuiltin(id: ResourceKind) {
    const next = favoriteKindEntries.value.filter(
      (e) => !(e.kind === "builtin" && e.id === id)
    );
    if (next.length === favoriteKindEntries.value.length) {
      next.unshift({ kind: "builtin", id });
    }
    favoriteKindEntries.value = next;
    persist();
  }

  function toggleFavoriteExtension(target: ResolvedAliasTarget) {
    const k = extensionStableKey(target);
    const next = favoriteKindEntries.value.filter(
      (e) => !(e.kind === "extension" && extensionStableKey(e.target) === k)
    );
    if (next.length === favoriteKindEntries.value.length) {
      next.unshift({ kind: "extension", target: { ...target } });
    }
    favoriteKindEntries.value = next;
    persist();
  }

  return {
    favoriteKindEntries,
    hydrateFavoriteKinds,
    isFavoriteBuiltin,
    isFavoriteExtension,
    toggleFavoriteBuiltin,
    toggleFavoriteExtension,
  };
}

/** 供命令面板同步读取收藏条目（与 composable 同源存储）。 */
export function readFavoriteKindEntries(validKindIds: Set<string>): FavoriteKindEntry[] {
  const raw = favoriteKindsStorage.read();
  const out: FavoriteKindEntry[] = [];
  for (const row of raw) {
    const e = fromStored(row, validKindIds);
    if (e) out.push(e);
  }
  return out;
}
