<script setup lang="ts">
/**
 * 命令面板：全局 Cmd/Ctrl+K 呼出，支持两种输入模式：
 * - 自由文本：模糊匹配所有已注册 CommandItem（tab 切换、全局动作、会话等）
 * - Token 语法：以 `@` / `#` / `>` 触发结构化输入；`>` 下列出当前工作台选中资源的可执行动作并可继续输入筛选
 *
 * 键盘：
 * - ArrowUp/Down / Home/End  候选导航
 * - Tab                      仅推进（advance）：选 spec / value / 自由命令
 * - Enter                    先推进；无法推进时（如组合已就绪）则提交，与 ⌘Enter 相同
 * - ⌘Enter / Ctrl+Enter      提交（submit）：落 value 并执行 executor / 高亮命令
 * - Backspace (draft 为空)   弹回最后一个 chip 继续编辑
 * - Esc                      关闭
 */
import { computed, nextTick, ref, watch } from "vue";
import { NButton, NModal } from "naive-ui";
import type { Token } from "../features/commandPalette";
import {
  useCommandPalette,
  type Candidate,
} from "../features/commandPalette";

const palette = useCommandPalette();
const {
  isOpen,
  draft,
  tokens,
  activeIndex,
  parsed,
  candidates,
  executorPlan,
  close,
  commitDraft,
  advancePalette,
  submitPlan,
  popLastToken,
  removeToken,
  displayTokenValue,
} = palette;

const inputRef = ref<HTMLInputElement | null>(null);
const listRef = ref<HTMLDivElement | null>(null);

const modeLabel = computed(() => {
  switch (parsed.value.mode) {
    case "keying":
      if (parsed.value.symbol === "#") return "过滤器";
      if (parsed.value.symbol === ">") return "资源动作";
      return "对象";
    case "valuing":
      if (parsed.value.symbol === ">") return "资源动作";
      return parsed.value.keyBuffer ?? "值";
    default:
      return "命令";
  }
});

const placeholder = computed(() => {
  const p = parsed.value;
  if (p.mode === "valuing") {
    if (p.symbol === ">") return "筛选或选择动作…";
    return `${p.keyBuffer} 的值…`;
  }
  if (p.mode === "keying") return `选择 ${p.symbol} 后的类别…`;
  return "输入命令，或用 @ / # / > 组合语法";
});

interface GroupedRow {
  candidate: Candidate;
  /** 在扁平 candidates 中的下标；分区仅用于展示，高亮必须以该下标为准，否则模糊排序打散分区后会出现多处同时选中 */
  globalIndex: number;
}

interface Group {
  section: string;
  items: GroupedRow[];
}

const groupedCandidates = computed<Group[]>(() => {
  const out: Group[] = [];
  const map = new Map<string, Group>();
  let running = 0;
  for (const c of candidates.value) {
    const section = sectionOf(c);
    let g = map.get(section);
    if (!g) {
      g = { section, items: [] };
      map.set(section, g);
      out.push(g);
    }
    g.items.push({ candidate: c, globalIndex: running });
    running += 1;
  }
  return out;
});

function sectionOf(c: Candidate): string {
  if (c.kind === "command") return c.item.section ?? "其他";
  if (c.kind === "spec") {
    if (c.spec.symbol === "#") return "过滤器";
    if (c.spec.symbol === ">") return "资源";
    return "对象";
  }
  return c.value.section ?? c.spec.label;
}

watch(
  () => candidates.value.length,
  (len) => {
    if (!len) activeIndex.value = 0;
    else if (activeIndex.value >= len) activeIndex.value = len - 1;
  },
);

watch([draft, tokens], () => {
  activeIndex.value = 0;
});

watch(isOpen, (v) => {
  if (v) {
    nextTick(() => {
      inputRef.value?.focus();
      inputRef.value?.select();
    });
  }
});

function move(delta: number) {
  const len = candidates.value.length;
  if (!len) return;
  activeIndex.value = (activeIndex.value + delta + len) % len;
  scrollActiveIntoView();
}

function scrollActiveIntoView() {
  nextTick(() => {
    const el = listRef.value?.querySelector<HTMLElement>("[data-active=\"true\"]");
    if (el) el.scrollIntoView({ block: "nearest" });
  });
}

function isMetaEnter(e: KeyboardEvent): boolean {
  if (e.key !== "Enter") return false;
  const mac = typeof navigator !== "undefined" && navigator.platform.toLowerCase().includes("mac");
  return mac ? e.metaKey : e.ctrlKey;
}

function onKeydown(e: KeyboardEvent) {
  if (e.isComposing) return;
  if (e.key === "ArrowDown") {
    e.preventDefault();
    move(1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    move(-1);
  } else if (e.key === "Home") {
    e.preventDefault();
    activeIndex.value = 0;
    scrollActiveIntoView();
  } else if (e.key === "End") {
    e.preventDefault();
    activeIndex.value = Math.max(0, candidates.value.length - 1);
    scrollActiveIntoView();
  } else if (e.key === "Enter" && isMetaEnter(e)) {
    e.preventDefault();
    submitPlan();
  } else if (e.key === "Tab") {
    e.preventDefault();
    advancePalette();
  } else if (e.key === "Enter") {
    e.preventDefault();
    if (!advancePalette()) {
      submitPlan();
    }
  } else if (e.key === "Escape") {
    e.preventDefault();
    close();
  } else if (e.key === "Backspace") {
    if (!draft.value && tokens.value.length) {
      e.preventDefault();
      popLastToken();
    }
  }
}

function onCandidateClick(index: number) {
  advancePalette(index);
  nextTick(() => inputRef.value?.focus());
}

function onChipRemove(t: Token) {
  removeToken(t.symbol, t.key);
  nextTick(() => inputRef.value?.focus());
}

interface TitleSegment { text: string; hit: boolean }

function titleOf(c: Candidate): { text: string; indices: number[] } {
  if (c.kind === "command") return { text: c.item.title, indices: c.matchedIndices };
  if (c.kind === "spec") return { text: `${c.spec.symbol}${c.spec.key} · ${c.spec.label}`, indices: [] };
  return { text: c.value.title, indices: [] };
}

function renderSegments(c: Candidate): TitleSegment[] {
  const { text, indices } = titleOf(c);
  if (!indices.length) return [{ text, hit: false }];
  const hits = new Set(indices);
  const out: TitleSegment[] = [];
  let buf = "";
  let bufHit = false;
  for (let i = 0; i < text.length; i += 1) {
    const h = hits.has(i);
    if (i === 0) { buf = text[i]; bufHit = h; continue; }
    if (h === bufHit) buf += text[i];
    else { out.push({ text: buf, hit: bufHit }); buf = text[i]; bufHit = h; }
  }
  if (buf) out.push({ text: buf, hit: bufHit });
  return out;
}

function subtitleOf(c: Candidate): string | undefined {
  if (c.kind === "command") return c.item.subtitle;
  if (c.kind === "spec") return c.spec.hint;
  return c.value.subtitle;
}

function hintOf(c: Candidate): string | undefined {
  if (c.kind === "command") return c.item.hint;
  if (c.kind === "spec") return `${c.spec.symbol}${c.spec.key}`;
  return c.value.hint;
}

function iconOf(c: Candidate): string | (() => unknown) | undefined {
  if (c.kind === "command") return c.item.icon as string | (() => unknown) | undefined;
  if (c.kind === "spec") return c.spec.icon;
  return c.value.icon;
}

function onInputFixup(e: Event) {
  // 当 draft 仅剩一个空格时，可视作用户想删除 draft 回到 free。
  const v = (e.target as HTMLInputElement).value;
  if (v === " ") draft.value = "";
}

function commitOnSpace(e: KeyboardEvent) {
  if (e.key !== " ") return;
  const p = parsed.value;
  if (p.mode !== "valuing") return;
  if (!p.value || p.value.trim() === "") return;
  if (commitDraft()) e.preventDefault();
}
</script>

<template>
  <NModal
    :show="isOpen"
    :mask-closable="true"
    :auto-focus="false"
    :trap-focus="true"
    @mask-click="close"
    @esc="close"
  >
    <div class="cmdk-panel" role="dialog" aria-label="命令面板" @keydown="onKeydown">
      <div class="cmdk-search">
        <span class="cmdk-scope" :data-mode="parsed.mode">{{ modeLabel }}</span>
        <div class="cmdk-field">
          <span
            v-for="(t, i) in tokens"
            :key="`${t.symbol}${t.key}-${i}`"
            class="cmdk-chip"
            :data-sym="t.symbol"
          >
            <span class="cmdk-chip-key">{{ t.symbol }}{{ t.key }}</span>
            <span class="cmdk-chip-eq">=</span>
            <span class="cmdk-chip-val">{{ displayTokenValue(t) }}</span>
            <NButton text class="cmdk-chip-x" size="tiny" aria-label="移除" @click.stop="onChipRemove(t)">×</NButton>
          </span>
          <input
            ref="inputRef"
            v-model="draft"
            class="cmdk-input"
            type="text"
            :placeholder="tokens.length ? '' : placeholder"
            autocomplete="off"
            spellcheck="false"
            @keydown="commitOnSpace"
            @input="onInputFixup"
          />
        </div>
        <kbd class="cmdk-kbd">Esc</kbd>
      </div>

      <div ref="listRef" class="cmdk-list" role="listbox">
        <template v-if="groupedCandidates.length">
          <div
            v-for="(group, gIdx) in groupedCandidates"
            :key="`${gIdx}-${group.section}`"
            class="cmdk-group"
          >
            <div class="cmdk-group-title">{{ group.section }}</div>
            <div
              v-for="row in group.items"
              :key="row.globalIndex"
              class="cmdk-item"
              role="option"
              :data-active="row.globalIndex === activeIndex"
              :aria-selected="row.globalIndex === activeIndex"
              @mousemove="activeIndex = row.globalIndex"
              @click="onCandidateClick(row.globalIndex)"
            >
              <span class="cmdk-item-icon" aria-hidden="true">
                <template v-if="typeof iconOf(row.candidate) === 'string'">{{ iconOf(row.candidate) }}</template>
                <component v-else-if="iconOf(row.candidate)" :is="iconOf(row.candidate)" />
                <template v-else>•</template>
              </span>
              <div class="cmdk-item-body">
                <div class="cmdk-item-title">
                  <template v-for="(seg, si) in renderSegments(row.candidate)" :key="si">
                    <span v-if="seg.hit" class="cmdk-hit">{{ seg.text }}</span>
                    <span v-else>{{ seg.text }}</span>
                  </template>
                </div>
                <div v-if="subtitleOf(row.candidate)" class="cmdk-item-subtitle">{{ subtitleOf(row.candidate) }}</div>
              </div>
              <span v-if="hintOf(row.candidate)" class="cmdk-item-hint">{{ hintOf(row.candidate) }}</span>
            </div>
          </div>
        </template>
        <div v-else class="cmdk-empty">
          <div class="cmdk-empty-emoji">🔍</div>
          <div class="cmdk-empty-text">
            <template v-if="parsed.mode === 'valuing'">尚无匹配；可继续输入，或按 Enter 提交</template>
            <template v-else-if="parsed.mode === 'free' && tokens.length && !parsed.rawText.trim() && executorPlan">
              组合已就绪，按 <kbd class="cmdk-kbd-inline">Enter</kbd> 跳转工作台并聚焦列表
            </template>
            <template v-else-if="draft">未找到与「{{ draft }}」相关的候选</template>
            <template v-else>输入以搜索命令…</template>
          </div>
        </div>
      </div>

      <div v-if="executorPlan" class="cmdk-plan">
        <span class="cmdk-plan-icon">{{ executorPlan.icon ?? "⏎" }}</span>
        <div class="cmdk-plan-body">
          <div class="cmdk-plan-title">{{ executorPlan.title }}</div>
          <div v-if="executorPlan.subtitle" class="cmdk-plan-sub">{{ executorPlan.subtitle }}</div>
        </div>
        <kbd class="cmdk-kbd">Enter</kbd>
      </div>

      <div class="cmdk-footer">
        <span class="cmdk-hint-group"><kbd>↑↓</kbd><span>选择</span></span>
        <span class="cmdk-hint-group"><kbd>Tab</kbd><span>推进</span></span>
        <span class="cmdk-hint-group"><kbd>Enter</kbd><span>推进或提交</span></span>
        <span class="cmdk-hint-group cmdk-hint-muted"><kbd>⌘</kbd><kbd>Enter</kbd><span>亦可提交</span></span>
        <span class="cmdk-hint-group"><kbd>@</kbd><kbd>#</kbd><kbd>></kbd><span>组合</span></span>
        <span class="cmdk-hint-spacer" />
        <span class="cmdk-hint-group cmdk-hint-muted">{{ candidates.length }} 条</span>
      </div>
    </div>
  </NModal>
</template>

<style scoped>
.cmdk-panel {
  box-sizing: border-box;
  width: min(680px, 92vw);
  height: min(72vh, 580px);
  max-height: min(72vh, 580px);
  display: flex;
  flex-direction: column;
  background: #ffffff;
  border-radius: 14px;
  box-shadow: 0 24px 64px rgba(15, 23, 42, 0.28), 0 0 0 1px rgba(15, 23, 42, 0.06);
  overflow: hidden;
}
.cmdk-search {
  display: flex;
  align-items: center;
  gap: 0.55rem;
  padding: 0.55rem 0.75rem;
  border-bottom: 1px solid var(--kf-border, rgba(148, 163, 184, 0.22));
  flex-shrink: 0;
  min-height: 3rem;
}
.cmdk-scope {
  font-size: 0.72rem;
  padding: 0.18rem 0.5rem;
  border-radius: 6px;
  background: rgba(100, 116, 139, 0.12);
  color: #475569;
  font-weight: 600;
  white-space: nowrap;
  flex-shrink: 0;
}
.cmdk-scope[data-mode="keying"] {
  background: rgba(168, 85, 247, 0.14);
  color: #7c3aed;
}
.cmdk-scope[data-mode="valuing"] {
  background: rgba(16, 185, 129, 0.14);
  color: #059669;
}
.cmdk-field {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  gap: 0.3rem;
  overflow-x: auto;
  overflow-y: hidden;
  overscroll-behavior-x: contain;
  scrollbar-width: thin;
}
.cmdk-chip {
  display: inline-flex;
  align-items: center;
  gap: 0.15rem;
  flex-shrink: 0;
  padding: 0.14rem 0.2rem 0.14rem 0.45rem;
  border-radius: 6px;
  font-size: 0.78rem;
  background: rgba(37, 99, 235, 0.1);
  color: #1d4ed8;
  line-height: 1.2;
  white-space: nowrap;
  max-width: 260px;
}
.cmdk-chip[data-sym="#"] {
  background: rgba(234, 88, 12, 0.12);
  color: #c2410c;
}
.cmdk-chip[data-sym=">"] {
  background: rgba(124, 58, 237, 0.12);
  color: #6d28d9;
}
.cmdk-chip-key { font-weight: 600; }
.cmdk-chip-eq { opacity: 0.5; padding: 0 0.1rem; }
.cmdk-chip-val { max-width: 140px; overflow: hidden; text-overflow: ellipsis; }
.cmdk-chip-x {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  margin-left: 0.2rem;
  border-radius: 4px;
  border: none;
  background: transparent;
  color: inherit;
  opacity: 0.6;
  cursor: pointer;
  font-size: 0.85rem;
  line-height: 1;
}
.cmdk-chip-x:hover { opacity: 1; background: rgba(15, 23, 42, 0.08); }
.cmdk-input {
  flex: 1 1 120px;
  min-width: 120px;
  border: none;
  outline: none;
  background: transparent;
  font-size: 0.95rem;
  color: var(--kf-text-primary, #0f172a);
}
.cmdk-kbd-inline {
  font-size: 0.68rem;
  font-family: inherit;
  padding: 0.06rem 0.28rem;
  margin: 0 0.06rem;
  border-radius: 4px;
  border: 1px solid rgba(148, 163, 184, 0.35);
  color: #64748b;
  background: #f8fafc;
}
.cmdk-kbd {
  font-size: 0.7rem;
  font-family: inherit;
  padding: 0.1rem 0.35rem;
  border-radius: 4px;
  border: 1px solid rgba(148, 163, 184, 0.35);
  color: #64748b;
  background: #f8fafc;
  flex-shrink: 0;
}
.cmdk-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0.25rem 0.25rem 0.35rem;
  display: flex;
  flex-direction: column;
}
.cmdk-group {
  flex-shrink: 0;
}
.cmdk-group-title {
  font-size: 0.68rem;
  color: #94a3b8;
  padding: 0.55rem 0.7rem 0.15rem;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
.cmdk-item {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  padding: 0.45rem 0.7rem;
  margin: 0 0.15rem;
  border-radius: 8px;
  cursor: pointer;
  user-select: none;
}
.cmdk-item[data-active="true"] {
  background: rgba(37, 99, 235, 0.1);
}
.cmdk-item-icon {
  width: 22px;
  height: 22px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 0.95rem;
  color: #475569;
  flex-shrink: 0;
}
.cmdk-item-body { flex: 1; min-width: 0; }
.cmdk-item-title {
  font-size: 0.88rem;
  color: #0f172a;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.cmdk-item-subtitle {
  font-size: 0.72rem;
  color: #64748b;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-top: 1px;
}
.cmdk-hit { color: #2563eb; font-weight: 600; }
.cmdk-item-hint {
  font-size: 0.7rem;
  color: #94a3b8;
  padding: 0.08rem 0.4rem;
  border-radius: 4px;
  background: rgba(148, 163, 184, 0.12);
  flex-shrink: 0;
}
.cmdk-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  padding: 2.2rem 1rem;
  min-height: 0;
  color: #64748b;
  font-size: 0.85rem;
}
.cmdk-empty-emoji { font-size: 1.6rem; opacity: 0.65; }
.cmdk-plan {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  padding: 0.5rem 0.75rem;
  border-top: 1px solid var(--kf-border, rgba(148, 163, 184, 0.22));
  background: rgba(37, 99, 235, 0.06);
  flex-shrink: 0;
  min-height: 3.25rem;
  box-sizing: border-box;
}
.cmdk-plan-icon {
  width: 24px;
  height: 24px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 0.95rem;
  color: #1d4ed8;
  background: rgba(37, 99, 235, 0.12);
  border-radius: 6px;
  flex-shrink: 0;
}
.cmdk-plan-body { flex: 1; min-width: 0; }
.cmdk-plan-title {
  font-size: 0.85rem;
  color: #0f172a;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.cmdk-plan-sub {
  font-size: 0.72rem;
  color: #64748b;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-top: 1px;
}
.cmdk-footer {
  display: flex;
  align-items: center;
  gap: 0.9rem;
  padding: 0.4rem 0.75rem;
  border-top: 1px solid var(--kf-border, rgba(148, 163, 184, 0.22));
  background: #f8fafc;
  font-size: 0.72rem;
  color: #64748b;
  flex-shrink: 0;
}
.cmdk-hint-group { display: inline-flex; align-items: center; gap: 0.3rem; }
.cmdk-hint-group kbd {
  font-family: inherit;
  font-size: 0.68rem;
  padding: 0.06rem 0.32rem;
  border-radius: 4px;
  border: 1px solid rgba(148, 163, 184, 0.35);
  background: #ffffff;
  color: #475569;
}
.cmdk-hint-muted { color: #94a3b8; }
.cmdk-hint-spacer { flex: 1; }
</style>
