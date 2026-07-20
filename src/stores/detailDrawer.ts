/**
 * 资源详情抽屉跨环境多页缓存：最多保留 5 个最近页面，切环境不销毁。
 */
import { computed, ref } from "vue";
import { uid } from "../utils/uid";

export type DetailTab =
  | "yaml"
  | "edit"
  | "describe"
  | "logs"
  | "topology"
  | "snapshots"
  | "taints";

export type DetailEditIntent = { mode: "kv" } | { mode: "structured" } | null;

export interface DetailPageResource {
  kind: string;
  name: string;
  namespace: string | null;
  /** 动态 API 资源（如 CRD）：走专用 get/describe。 */
  dynamic?: { api_version: string; namespaced: boolean };
}

export interface DetailPage {
  id: string;
  key: string;
  envId: string;
  envName: string;
  resource: DetailPageResource;
  activeTab: DetailTab;
  editIntent: DetailEditIntent;
  dirty: boolean;
  lastAccessedAt: number;
}

export type OpenDetailPageInput = {
  envId: string;
  envName: string;
  resource: DetailPageResource;
  /** 原始打开意图，如 editConfig / topology / taints */
  initialTabHint?: string | null;
};

const MAX_DETAIL_PAGES = 5;

const pages = ref<DetailPage[]>([]);
const activePageId = ref<string | null>(null);
const visible = ref(false);

export function buildDetailPageKey(envId: string, resource: DetailPageResource): string {
  const api = resource.dynamic?.api_version ?? "";
  return `${envId}|${resource.kind}|${resource.namespace ?? ""}|${resource.name}|${api}`;
}

/** 将打开意图（含 editConfig）解析为页内 DetailTab。 */
export function resolveDetailInitialTab(
  resource: DetailPageResource,
  initialTab: string | null | undefined
): DetailTab {
  const kind = resource.kind;
  if (initialTab === "editConfig" || initialTab === "edit") return "edit";
  if (initialTab === "yaml") return "yaml";
  if (
    initialTab === "logs" &&
    (kind === "Pod" ||
      kind === "Deployment" ||
      kind === "StatefulSet" ||
      kind === "DaemonSet")
  ) {
    return "logs";
  }
  if (initialTab === "taints" && kind === "Node") return "taints";
  if (initialTab === "topology" && !resource.dynamic) return "topology";
  if (initialTab === "snapshots") return "snapshots";
  if (initialTab === "describe") return "describe";
  return "yaml";
}

export function resolveDetailEditIntent(initialTab: string | null | undefined): DetailEditIntent {
  if (initialTab === "editConfig") return { mode: "kv" };
  if (initialTab === "edit") return { mode: "structured" };
  return null;
}

function touchPage(page: DetailPage) {
  page.lastAccessedAt = Date.now();
}

function findPageById(id: string): DetailPage | undefined {
  return pages.value.find((p) => p.id === id);
}

export function useDetailDrawerStore() {
  const activePage = computed(() =>
    activePageId.value ? (findPageById(activePageId.value) ?? null) : null
  );

  function hideDrawer() {
    visible.value = false;
  }

  function setPageDirty(pageId: string, dirty: boolean) {
    const page = findPageById(pageId);
    if (page) page.dirty = dirty;
  }

  function setPageTab(pageId: string, tab: DetailTab) {
    const page = findPageById(pageId);
    if (page) {
      page.activeTab = tab;
      touchPage(page);
    }
  }

  function setActivePage(pageId: string) {
    if (!findPageById(pageId)) return;
    activePageId.value = pageId;
    const page = findPageById(pageId);
    if (page) touchPage(page);
    visible.value = true;
  }

  /** 关闭单个详情页；若无剩余页则隐藏抽屉。 */
  function closePage(pageId: string) {
    const next = pages.value.filter((p) => p.id !== pageId);
    pages.value = next;
    if (activePageId.value === pageId) {
      activePageId.value = next[0]?.id ?? null;
      if (next[0]) touchPage(next[0]);
    }
    if (!next.length) {
      visible.value = false;
      activePageId.value = null;
    }
  }

  function clearEnvPages(envId: string) {
    const next = pages.value.filter((p) => p.envId !== envId);
    const removedActive = activePage.value?.envId === envId;
    pages.value = next;
    if (removedActive) {
      activePageId.value = next[0]?.id ?? null;
      if (next[0]) touchPage(next[0]);
    }
    if (!next.length) {
      visible.value = false;
      activePageId.value = null;
    }
  }

  /**
   * 切到目标环境时：若该环境有缓存页则激活最近一页；否则保持当前页（便于跨环境对照）。
   * 当前页有未保存修改时不自动切换，避免草稿丢失。
   */
  function preferEnvPage(envId: string) {
    if (activePage.value?.dirty) return;
    const forEnv = pages.value
      .filter((p) => p.envId === envId)
      .sort((a, b) => b.lastAccessedAt - a.lastAccessedAt);
    if (!forEnv.length) return;
    activePageId.value = forEnv[0].id;
    touchPage(forEnv[0]);
  }

  /**
   * 选出可淘汰页：优先非当前、非 dirty、最旧；否则返回最旧非当前（含 dirty，需确认）。
   */
  function pickEvictionCandidate(excludeId: string | null): DetailPage | null {
    const candidates = pages.value.filter((p) => p.id !== excludeId);
    if (!candidates.length) return null;
    const clean = candidates
      .filter((p) => !p.dirty)
      .sort((a, b) => a.lastAccessedAt - b.lastAccessedAt);
    if (clean.length) return clean[0];
    return [...candidates].sort((a, b) => a.lastAccessedAt - b.lastAccessedAt)[0] ?? null;
  }

  /**
   * 打开或聚焦详情页。若离开脏页或淘汰脏页，经 `confirmDiscard` 确认后继续。
   * @returns 新/已有 page id；用户取消时返回 null
   */
  async function openOrFocusPage(
    input: OpenDetailPageInput,
    confirmDiscard?: () => Promise<boolean>
  ): Promise<string | null> {
    const key = buildDetailPageKey(input.envId, input.resource);
    const existing = pages.value.find((p) => p.key === key) ?? null;
    const hint = input.initialTabHint;
    const resolvedTab = resolveDetailInitialTab(input.resource, hint);
    const resolvedIntent = resolveDetailEditIntent(hint);

    if (existing) {
      if (
        activePageId.value &&
        activePageId.value !== existing.id &&
        activePage.value?.dirty
      ) {
        const ok = confirmDiscard ? await confirmDiscard() : true;
        if (!ok) return null;
        setPageDirty(activePageId.value, false);
      }
      if (hint) {
        existing.activeTab = resolvedTab;
        existing.editIntent = resolvedIntent;
      }
      setActivePage(existing.id);
      return existing.id;
    }

    if (activePage.value?.dirty) {
      const ok = confirmDiscard ? await confirmDiscard() : true;
      if (!ok) return null;
      if (activePageId.value) setPageDirty(activePageId.value, false);
    }

    while (pages.value.length >= MAX_DETAIL_PAGES) {
      const victim = pickEvictionCandidate(activePageId.value);
      if (!victim) break;
      if (victim.dirty) {
        const ok = confirmDiscard ? await confirmDiscard() : true;
        if (!ok) return null;
      }
      pages.value = pages.value.filter((p) => p.id !== victim.id);
      if (activePageId.value === victim.id) {
        activePageId.value = pages.value[0]?.id ?? null;
      }
    }

    const id = uid("detail");
    const page: DetailPage = {
      id,
      key,
      envId: input.envId,
      envName: input.envName,
      resource: {
        ...input.resource,
        dynamic: input.resource.dynamic ? { ...input.resource.dynamic } : undefined,
      },
      activeTab: resolvedTab,
      editIntent: resolvedIntent,
      dirty: false,
      lastAccessedAt: Date.now(),
    };
    pages.value = [page, ...pages.value];
    activePageId.value = id;
    visible.value = true;
    return id;
  }

  return {
    pages,
    activePageId,
    activePage,
    visible,
    hideDrawer,
    setActivePage,
    closePage,
    clearEnvPages,
    preferEnvPage,
    openOrFocusPage,
    setPageDirty,
    setPageTab,
    maxPages: MAX_DETAIL_PAGES,
  };
}
