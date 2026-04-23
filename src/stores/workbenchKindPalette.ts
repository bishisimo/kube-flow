import { ref } from "vue";
import { kubeSearchResourceKinds } from "../api/kube";
import type { ResolvedAliasTarget } from "../api/kube";
import { isWorkbenchBuiltinTarget } from "../features/workbench/builtinGvk";

/** 命令面板 @kind 时扩展资源异步搜索结果（已排除内置 GVK）。 */
export const workbenchKindPaletteExtensionHits = ref<ResolvedAliasTarget[]>([]);

let paletteSearchTimer: number | null = null;

export function clearWorkbenchKindPaletteSearch() {
  if (paletteSearchTimer != null) {
    clearTimeout(paletteSearchTimer);
    paletteSearchTimer = null;
  }
  workbenchKindPaletteExtensionHits.value = [];
}

/**
 * 防抖触发扩展资源搜索；`query` 为空则清空。
 */
export function scheduleWorkbenchKindPaletteSearch(
  envId: string | null,
  connected: boolean,
  rawQuery: string
): void {
  if (paletteSearchTimer != null) {
    clearTimeout(paletteSearchTimer);
    paletteSearchTimer = null;
  }
  const q = rawQuery.trim();
  if (!envId || !connected || !q) {
    workbenchKindPaletteExtensionHits.value = [];
    return;
  }
  paletteSearchTimer = window.setTimeout(() => {
    paletteSearchTimer = null;
    void (async () => {
      try {
        const hits = await kubeSearchResourceKinds(envId, q, 10);
        workbenchKindPaletteExtensionHits.value = hits.filter((h) => !isWorkbenchBuiltinTarget(h));
      } catch {
        workbenchKindPaletteExtensionHits.value = [];
      }
    })();
  }, 280);
}
