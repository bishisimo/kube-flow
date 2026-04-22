/**
 * TokenSpec 与 Executor 的全局注册表（进程内单例）。
 *
 * AppShell 或具体 view 在 onMounted 阶段注册自己的 spec/executor，返回的 dispose
 * 在 onBeforeUnmount 调用。查询时按 context（当前 tab）过滤。
 */
import { computed, ref } from "vue";
import type { Executor, TokenSpec, TokenSymbol } from "./types";

const tokenSpecs = ref<Map<string, TokenSpec>>(new Map());
const executors = ref<Map<string, Executor>>(new Map());

function makeSpecId(spec: TokenSpec): string {
  return `${spec.symbol}:${spec.key}:${spec.context ?? "*"}`;
}

export function registerTokenSpec(spec: TokenSpec): () => void {
  const id = makeSpecId(spec);
  tokenSpecs.value.set(id, spec);
  tokenSpecs.value = new Map(tokenSpecs.value);
  return () => {
    tokenSpecs.value.delete(id);
    tokenSpecs.value = new Map(tokenSpecs.value);
  };
}

export function registerTokenSpecs(specs: TokenSpec[]): () => void {
  const disposers = specs.map((s) => registerTokenSpec(s));
  return () => {
    for (const d of disposers) d();
  };
}

export function registerExecutor(executor: Executor): () => void {
  executors.value.set(executor.id, executor);
  executors.value = new Map(executors.value);
  return () => {
    executors.value.delete(executor.id);
    executors.value = new Map(executors.value);
  };
}

export function registerExecutors(list: Executor[]): () => void {
  const disposers = list.map((e) => registerExecutor(e));
  return () => {
    for (const d of disposers) d();
  };
}

/** 按当前上下文取所有可用 TokenSpec（context 为空表示通用）。 */
export function useSpecsForContext(contextRef: { value: string | null }) {
  return computed<TokenSpec[]>(() => {
    const ctx = contextRef.value;
    const out: TokenSpec[] = [];
    for (const spec of tokenSpecs.value.values()) {
      if (!spec.context || spec.context === ctx) out.push(spec);
    }
    return out;
  });
}

/** 按 symbol 查询可用的 key 列表（用于 keying 模式的候选）。 */
export function useSpecsForSymbol(
  contextRef: { value: string | null },
  symbolRef: { value: TokenSymbol | undefined },
) {
  return computed<TokenSpec[]>(() => {
    const ctx = contextRef.value;
    const sym = symbolRef.value;
    if (!sym) return [];
    const out: TokenSpec[] = [];
    for (const spec of tokenSpecs.value.values()) {
      if (spec.symbol !== sym) continue;
      if (!spec.context || spec.context === ctx) out.push(spec);
    }
    return out;
  });
}

export function findSpec(
  contextRef: { value: string | null },
  symbol: TokenSymbol,
  key: string,
): TokenSpec | null {
  const ctx = contextRef.value;
  for (const spec of tokenSpecs.value.values()) {
    if (spec.symbol !== symbol) continue;
    if (spec.key !== key) continue;
    if (spec.context && spec.context !== ctx) continue;
    return spec;
  }
  return null;
}

export function useExecutorsForContext(contextRef: { value: string | null }) {
  return computed<Executor[]>(() => {
    const ctx = contextRef.value;
    const out: Executor[] = [];
    for (const e of executors.value.values()) {
      if (!e.context || e.context === ctx) out.push(e);
    }
    out.sort((a, b) => {
      if (b.priority !== a.priority) return b.priority - a.priority;
      return a.id.localeCompare(b.id);
    });
    return out;
  });
}
