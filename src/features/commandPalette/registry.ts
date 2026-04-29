/**
 * 命令面板全局注册表：进程内单例，保存当前所有可用命令的"提供者"函数。
 * 每次面板打开或查询变化时，调用所有 provider 汇总命令，再按上下文/查询/MRU 打分排序。
 */
import { computed, ref } from "vue";
import type { CommandItem, CommandProvider } from "./types";

const providers = ref<Map<string, CommandProvider>>(new Map());
const currentContext = ref<string | null>(null);

export function registerProvider(id: string, provider: CommandProvider): () => void {
  providers.value.set(id, provider);
  providers.value = new Map(providers.value);
  return () => {
    providers.value.delete(id);
    providers.value = new Map(providers.value);
  };
}

export function setPaletteContext(ctx: string | null) {
  currentContext.value = ctx;
}

export function usePaletteContext() {
  return computed(() => currentContext.value);
}

/** 聚合所有 provider 的命令；会在 provider 变化或其内部响应式依赖变化时自动重算。 */
export const allCommands = computed<CommandItem[]>(() => {
  const out: CommandItem[] = [];
  const seen = new Set<string>();
  for (const provider of providers.value.values()) {
    const items = provider();
    for (const item of items) {
      if (seen.has(item.id)) continue;
      if (item.availableWhen && !item.availableWhen()) continue;
      seen.add(item.id);
      out.push(item);
    }
  }
  return out;
});
