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
 * - commitDraft：将当前 value 草稿匹配为 chip（由 executePlan 或空格提交调用）
 * - advancePalette：仅推进（选 spec/value、选命令）；已提交 chip 且草稿为空时返回 false；面板在 Enter 上会先调用，失败再 submitPlan
 * - submitPlan：executePlan（落 value chip + 跑 executor / 自由命令）；面板在 Enter 无法推进时会回退调用
 */
import { computed, ref, watch } from "vue";
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
import { useEnvStore } from "../../stores/env";
import { useConnectionStore } from "../../stores/connection";
import {
  clearWorkbenchKindPaletteSearch,
  scheduleWorkbenchKindPaletteSearch,
  workbenchKindPaletteExtensionHits,
} from "../../stores/workbenchKindPalette";
import { workbenchResourcePaletteAdapter } from "../../stores/workbenchResourcePalette";

/** 已落 `@env` / `@term` / `@log` 的 value 后，自由区为空时仍要展示其「二阶段」动作命令候选。 */
function hasTokenActionFreePhase(tokens: Token[]): boolean {
  return tokens.some(
    (t) =>
      Boolean(t.value) &&
      t.symbol === "@" &&
      (t.key === "env" || t.key === "term" || t.key === "log"),
  );
}

/** 二阶段环境/终端/日志动作命令：排序应仅由 weight 等显式配置决定，不受 MRU 影响。 */
function isContextActionCommandId(id: string): boolean {
  return (
    id.startsWith("env:act:") || id.startsWith("term:act:") || id.startsWith("log:act:")
  );
}

/** 同分时的稳定顺序：与 providers 中「环境/终端/日志动作」定义顺序一致（工作台始终优先于终端等）。 */
function contextActionTieIndex(id: string): number {
  if (id.startsWith("env:act:workbench:")) return 0;
  if (id.startsWith("env:act:open:")) return 1;
  if (id.startsWith("env:act:host-terminal:")) return 2;
  if (id.startsWith("env:act:orchestrator:")) return 3;
  if (id.startsWith("env:act:logs:")) return 4;
  if (id.startsWith("env:act:close:")) return 5;
  if (id.startsWith("term:act:focus:")) return 0;
  if (id.startsWith("term:act:close:")) return 1;
  if (id.startsWith("log:act:focus:")) return 0;
  if (id.startsWith("log:act:close:")) return 1;
  return 100;
}

const isOpen = ref(false);
const draft = ref("");
const tokens = ref<Token[]>([]);
const activeIndex = ref(0);

const { record: recordMru, scoreOf: mruScoreOf } = useCommandMru();
const paletteContext = usePaletteContext();
const { currentId } = useEnvStore();
const connectionStore = useConnectionStore();

const parsed = computed<Draft>(() => parseDraft(draft.value));

watch(
  () =>
    [
      isOpen.value,
      parsed.value.mode,
      parsed.value.symbol,
      parsed.value.keyBuffer,
      parsed.value.value ?? "",
    ] as const,
  ([open, mode, sym, keyBuf, val]) => {
    if (!open || mode !== "valuing" || sym !== "@" || keyBuf !== "kind") {
      clearWorkbenchKindPaletteSearch();
      return;
    }
    const id = currentId.value;
    const ok = Boolean(id && connectionStore.getState(id) === "connected");
    scheduleWorkbenchKindPaletteSearch(id, ok, val);
  }
);

watch(isOpen, (open) => {
  if (!open) clearWorkbenchKindPaletteSearch();
});

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
  opts?: { ignoreMruForContextActions: boolean; tokenActionFreePhase: boolean },
): ScoredCommand | null {
  const mruOff =
    opts?.tokenActionFreePhase &&
    opts?.ignoreMruForContextActions &&
    isContextActionCommandId(item.id);
  const mru = mruOff ? 0 : mruScoreOf(item.id) * 0.6;
  const base = (item.weight ?? 0) + mru + categoryBoost(item);
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

  if (p.mode === "valuing" && p.symbol === "@" && p.keyBuffer === "kind") {
    void workbenchKindPaletteExtensionHits.value;
  }

  if (p.mode === "free") {
    const text = p.rawText;
    if (tokens.value.length > 0 && !text.trim() && !hasTokenActionFreePhase(tokens.value)) {
      return [];
    }
    const tokenPhase = hasTokenActionFreePhase(tokens.value);
    const scored: Array<{ sc: ScoredCommand }> = [];
    for (const item of allCommands.value) {
      const s = scoreCommandItem(item, text, ctx, {
        ignoreMruForContextActions: true,
        tokenActionFreePhase: tokenPhase,
      });
      if (s) scored.push({ sc: s });
    }
    scored.sort((a, b) => {
      if (b.sc.score !== a.sc.score) return b.sc.score - a.sc.score;
      if (tokenPhase && isContextActionCommandId(a.sc.item.id) && isContextActionCommandId(b.sc.item.id)) {
        return contextActionTieIndex(a.sc.item.id) - contextActionTieIndex(b.sc.item.id);
      }
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

  if (p.mode === "valuing" && p.symbol !== undefined && p.keyBuffer !== undefined) {
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
  const spec =
    p.symbol !== undefined && p.keyBuffer !== undefined
      ? findSpec(paletteContext, p.symbol, p.keyBuffer)
      : null;
  if (!spec) return false;
  const raw = forcedValue ?? (p.value ?? "").trim();
  if (!raw) return false;
  if (!spec.freeText) {
    const matched = spec.values(raw).find(
      (v) => v.value.toLowerCase() === raw.toLowerCase() || v.title.toLowerCase() === raw.toLowerCase(),
    );
    if (!matched) return false;
    if (isResourceActionValueSpec(spec)) {
      return tryRunResourcePaletteActionImmediate(matched.value);
    }
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

function isResourceActionValueSpec(spec: TokenSpec): boolean {
  return spec.symbol === ">" && spec.key === "";
}

/** 「>」下列出的是可执行动作：选中即 runAction 并关面板，不落 chip、不经 executor 预览条。 */
function tryRunResourcePaletteActionImmediate(valueId: string): boolean {
  if (valueId === "__please_select") return false;
  const a = workbenchResourcePaletteAdapter.value;
  if (!a) return false;
  close();
  tokens.value = [];
  draft.value = "";
  activeIndex.value = 0;
  queueMicrotask(() => a.runAction(valueId));
  return true;
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
    if (isResourceActionValueSpec(c.spec)) {
      return tryRunResourcePaletteActionImmediate(c.value.value);
    }
    appendToken({ symbol: c.spec.symbol, key: c.spec.key, value: c.value.value });
    draft.value = "";
    activeIndex.value = 0;
    return true;
  }
  return false;
}

/** 先落当前 value chip（若有），再执行 executor；无方案时仅在自由模式执行高亮命令。 */
function executePlan() {
  const p0 = parsed.value;
  if (p0.mode === "valuing") {
    const list = candidates.value;
    const first = list[activeIndex.value];
    if (first && first.kind === "value") {
      if (isResourceActionValueSpec(first.spec)) {
        if (tryRunResourcePaletteActionImmediate(first.value.value)) return;
        draft.value = "";
        return;
      }
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
  const p1 = parsed.value;
  if (p1.mode === "free") {
    const list = candidates.value;
    const first = list[activeIndex.value];
    if (first && first.kind === "command") {
      recordMru(first.item.id);
      close();
      queueMicrotask(() => void first.item.run());
    }
  }
}

/** Enter / Tab：只推进状态机，不执行工作台 executor。 */
function advancePalette(forcedIndex?: number): boolean {
  if (forcedIndex !== undefined) activeIndex.value = forcedIndex;
  const p = parsed.value;
  if (
    tokens.value.length > 0 &&
    p.mode === "free" &&
    !p.rawText.trim() &&
    !hasTokenActionFreePhase(tokens.value)
  ) {
    return false;
  }
  if (p.mode === "valuing" || p.mode === "keying") {
    return acceptCandidate();
  }
  return acceptCandidate();
}

function submitPlan(): void {
  executePlan();
}

function popLastToken() {
  if (!tokens.value.length) return;
  const next = [...tokens.value];
  const last = next.pop()!;
  tokens.value = next;
  draft.value =
    last.symbol === ">" && last.key === ""
      ? `>${last.value}`
      : `${last.symbol}${last.key} ${last.value}`;
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
    advancePalette,
    submitPlan,
    popLastToken,
    removeToken,
  };
}
