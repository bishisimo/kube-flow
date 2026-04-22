/**
 * 命令面板核心 composable：面板状态 + token 状态机 + 候选计算 + 执行器匹配。
 *
 * 状态组成：
 * - isOpen         面板可见性
 * - draft          用户未 commit 的字符串（对应 Draft 解析产物）
 * - tokens         已 commit 的 Token chip 数组
 * - activeIndex    当前候选项高亮下标
 *
 * 候选与执行按 draft.mode 分流：
 * - free   → CommandItem 集合（来自 registry）
 * - keying → 当前 context 下 symbol 对应的 TokenSpec 列表
 * - valuing → 对应 TokenSpec.values() 返回的值候选
 *
 * 执行入口：
 * - commitDraft：Tab 触发，把 draft 固化为 chip
 * - execute：Enter 触发，先 commit 再请求 executor.match 的最佳方案
 */
import { computed, ref } from "vue";
import type {
  CommandItem,
  Draft,
  ExecutorPlan,
  ScoredCommand,
  Token,
  TokenSpec,
  TokenValueCandidate,
} from "./types";
import { parseDraft, commitDraftToToken, writeKeySelection } from "./parser";
import { allCommands, usePaletteContext } from "./registry";
import { fuzzyMatch } from "./fuzzy";
import { useCommandMru } from "./mru";
import {
  findSpec,
  useExecutorsForContext,
  useSpecsForContext,
  useSpecsForSymbol,
} from "./tokenEngine";

const isOpen = ref(false);
const draft = ref("");
const tokens = ref<Token[]>([]);
const activeIndex = ref(0);

const { record: recordMru, scoreOf: mruScoreOf } = useCommandMru();
const paletteContext = usePaletteContext();

const parsed = computed<Draft>(() => parseDraft(draft.value));

/* ---------------- 候选集合 ---------------- */

export interface FreeCandidate {
  kind: "command";
  item: CommandItem;
  matchedIndices: number[];
}
export interface KeyingCandidate {
  kind: "spec";
  spec: TokenSpec;
  matchedIndices: number[];
}
export interface ValueCandidate {
  kind: "value";
  spec: TokenSpec;
  value: TokenValueCandidate;
  matchedIndices: number[];
}
export type Candidate = FreeCandidate | KeyingCandidate | ValueCandidate;

function categoryBoost(item: CommandItem): number {
  switch (item.category) {
    case "nav":
      return 3;
    case "action":
      return 2;
    case "session":
      return 2;
    default:
      return 0;
  }
}

function scoreCommandItem(
  item: CommandItem,
  text: string,
  ctx: string | null,
): ScoredCommand | null {
  const base = (item.weight ?? 0) + mruScoreOf(item.id) * 0.6 + categoryBoost(item);
  const ctxBonus = ctx && item.context === ctx ? 12 : 0;
  if (!text) {
    return { item, score: base + ctxBonus + 1, matchedIndices: [], matchedField: null };
  }
  const titleMatch = fuzzyMatch(text, item.title);
  if (titleMatch.score > 0) {
    return {
      item,
      score: base + ctxBonus + titleMatch.score * 1.0,
      matchedIndices: titleMatch.indices,
      matchedField: "title",
    };
  }
  if (item.subtitle) {
    const subMatch = fuzzyMatch(text, item.subtitle);
    if (subMatch.score > 0) {
      return {
        item,
        score: base + ctxBonus + subMatch.score * 0.55,
        matchedIndices: subMatch.indices,
        matchedField: "subtitle",
      };
    }
  }
  if (item.keywords?.length) {
    let best = 0;
    for (const kw of item.keywords) {
      const m = fuzzyMatch(text, kw);
      if (m.score > best) best = m.score;
    }
    if (best > 0) {
      return {
        item,
        score: base + ctxBonus + best * 0.45,
        matchedIndices: [],
        matchedField: "keywords",
      };
    }
  }
  return null;
}

const symbolRef = computed(() => parsed.value.symbol);
const specsForSymbol = useSpecsForSymbol(paletteContext, symbolRef);
const specsForContext = useSpecsForContext(paletteContext);
const executorsForContext = useExecutorsForContext(paletteContext);

/** 当前 draft 对应的候选列表（已按得分降序）。 */
const candidates = computed<Candidate[]>(() => {
  const p = parsed.value;
  const ctx = paletteContext.value;

  if (p.mode === "free") {
    const text = p.rawText;
    const scored: Array<{ sc: ScoredCommand }> = [];
    for (const item of allCommands.value) {
      const s = scoreCommandItem(item, text, ctx);
      if (s) scored.push({ sc: s });
    }
    scored.sort((a, b) => {
      if (b.sc.score !== a.sc.score) return b.sc.score - a.sc.score;
      return a.sc.item.title.localeCompare(b.sc.item.title);
    });
    return scored.slice(0, 120).map<FreeCandidate>((x) => ({
      kind: "command",
      item: x.sc.item,
      matchedIndices: x.sc.matchedField === "title" ? x.sc.matchedIndices : [],
    }));
  }

  if (p.mode === "keying") {
    const text = (p.keyBuffer ?? "").toLowerCase();
    const seen = new Set<string>();
    const scored: Array<{ spec: TokenSpec; score: number; indices: number[] }> = [];
    for (const spec of specsForSymbol.value) {
      if (seen.has(spec.key)) continue;
      seen.add(spec.key);
      if (!text) {
        scored.push({ spec, score: spec.weight ?? 0, indices: [] });
        continue;
      }
      const keyMatch = fuzzyMatch(text, spec.key);
      const labelMatch = fuzzyMatch(text, spec.label);
      const best = keyMatch.score >= labelMatch.score ? keyMatch : labelMatch;
      if (best.score > 0) {
        scored.push({ spec, score: (spec.weight ?? 0) + best.score, indices: best.indices });
      }
    }
    scored.sort((a, b) => {
      if (b.score !== a.score) return b.score - a.score;
      return a.spec.key.localeCompare(b.spec.key);
    });
    return scored.slice(0, 60).map<KeyingCandidate>((x) => ({
      kind: "spec",
      spec: x.spec,
      matchedIndices: x.indices,
    }));
  }

  if (p.mode === "valuing" && p.symbol && p.keyBuffer) {
    const spec = findSpec(paletteContext, p.symbol, p.keyBuffer);
    if (!spec) return [];
    const values = spec.values(p.value ?? "");
    return values.slice(0, 120).map<ValueCandidate>((v) => ({
      kind: "value",
      spec,
      value: v,
      matchedIndices: [],
    }));
  }

  return [];
});

/* ---------------- 执行计划 ---------------- */

const executorPlan = computed<ExecutorPlan | null>(() => {
  const p = parsed.value;
  const freeText = p.mode === "free" ? p.rawText : "";
  const ctx: { tokens: Token[]; freeText: string; context: string | null } = {
    tokens: tokens.value,
    freeText,
    context: paletteContext.value,
  };
  for (const e of executorsForContext.value) {
    const plan = e.match(ctx);
    if (plan) return plan;
  }
  return null;
});

/* ---------------- 操作 ---------------- */

function open(initial?: string) {
  draft.value = initial ?? "";
  tokens.value = [];
  activeIndex.value = 0;
  isOpen.value = true;
}

function close() {
  isOpen.value = false;
}

function toggle() {
  if (isOpen.value) close();
  else open();
}

/** 把当前 draft 固化为 chip；成功返回 true，失败不改动任何状态。 */
function commitDraft(forcedValue?: string): boolean {
  const p = parsed.value;
  if (p.mode !== "valuing") return false;
  const spec = p.symbol && p.keyBuffer ? findSpec(paletteContext, p.symbol, p.keyBuffer) : null;
  if (!spec) return false;
  const raw = forcedValue ?? (p.value ?? "").trim();
  if (!raw) return false;
  if (!spec.freeText) {
    const matched = spec.values(raw).find(
      (v) => v.value.toLowerCase() === raw.toLowerCase() || v.title.toLowerCase() === raw.toLowerCase(),
    );
    if (!matched) return false;
    const token = commitDraftToToken(p, matched.value);
    if (!token) return false;
    appendToken(token);
  } else {
    const token = commitDraftToToken(p, raw);
    if (!token) return false;
    appendToken(token);
  }
  draft.value = "";
  activeIndex.value = 0;
  return true;
}

function appendToken(token: Token) {
  const idx = tokens.value.findIndex(
    (t) => t.symbol === token.symbol && t.key === token.key,
  );
  if (idx >= 0) {
    const next = [...tokens.value];
    next[idx] = token;
    tokens.value = next;
  } else {
    tokens.value = [...tokens.value, token];
  }
}

/** 选中某个候选的统一入口：根据候选类型自动完成 draft 推进 / chip 追加 / 命令执行。 */
function acceptCandidate(index?: number): boolean {
  const list = candidates.value;
  const c = list[index ?? activeIndex.value];
  if (!c) return false;
  if (c.kind === "command") {
    recordMru(c.item.id);
    close();
    queueMicrotask(() => void c.item.run());
    return true;
  }
  if (c.kind === "spec") {
    draft.value = writeKeySelection(c.spec.symbol, c.spec.key);
    activeIndex.value = 0;
    return true;
  }
  if (c.kind === "value") {
    appendToken({ symbol: c.spec.symbol, key: c.spec.key, value: c.value.value });
    draft.value = "";
    activeIndex.value = 0;
    return true;
  }
  return false;
}

/** Enter 的完整语义：先尝试 commit 当前 draft，再执行 executor。 */
function executePlan() {
  const p = parsed.value;
  if (p.mode === "valuing") {
    const list = candidates.value;
    const first = list[activeIndex.value];
    if (first && first.kind === "value") {
      appendToken({ symbol: first.spec.symbol, key: first.spec.key, value: first.value.value });
    } else {
      commitDraft();
    }
    draft.value = "";
  }
  const plan = executorPlan.value;
  if (plan) {
    close();
    queueMicrotask(() => void plan.run());
    tokens.value = [];
    draft.value = "";
    return;
  }
  if (p.mode === "free") {
    const list = candidates.value;
    const first = list[activeIndex.value];
    if (first && first.kind === "command") {
      recordMru(first.item.id);
      close();
      queueMicrotask(() => void first.item.run());
    }
  }
}

function popLastToken() {
  if (!tokens.value.length) return;
  const next = [...tokens.value];
  const last = next.pop()!;
  tokens.value = next;
  draft.value = `${last.symbol}${last.key} ${last.value}`;
  activeIndex.value = 0;
}

function removeToken(symbol: string, key: string) {
  tokens.value = tokens.value.filter((t) => !(t.symbol === symbol && t.key === key));
}

/* ---------------- 快捷键安装 ---------------- */

export function installPaletteShortcut(toggleFn: () => void): () => void {
  function onKeyDown(e: KeyboardEvent) {
    const isMac = navigator.platform.toLowerCase().includes("mac");
    const isMeta = isMac ? e.metaKey : e.ctrlKey;
    if (!isMeta || e.shiftKey || e.altKey) return;
    if (e.key !== "k" && e.key !== "K") return;
    e.preventDefault();
    e.stopPropagation();
    toggleFn();
  }
  window.addEventListener("keydown", onKeyDown, { capture: true });
  return () => window.removeEventListener("keydown", onKeyDown, { capture: true });
}

export function useCommandPalette() {
  return {
    isOpen,
    draft,
    tokens,
    activeIndex,
    parsed,
    candidates,
    executorPlan,
    specsForContext,
    open,
    close,
    toggle,
    commitDraft,
    acceptCandidate,
    executePlan,
    popLastToken,
    removeToken,
  };
}
