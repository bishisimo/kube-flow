<script setup lang="ts">
import { ref, watch, computed, onUnmounted, onMounted, nextTick } from "vue";
import { NButton, NCheckbox, NInput, NSelect, NSwitch, NPopover, NTooltip } from "naive-ui";
import { listen } from "@tauri-apps/api/event";
import { extractErrorMessage } from "../utils/errorMessage";
import { AnsiUp } from "ansi_up";
import {
  kubeGetPodContainers,
  kubePodLogs,
  kubePodLogStreamStart,
  kubePodLogStreamStop,
} from "../api/kube";
import { logGetDisplaySettings, logSetDisplaySettings, type LogDisplayOrder } from "../api/log";
import { useLogStore } from "../stores/log";
import { useStrongholdAuthStore } from "../stores/strongholdAuth";
import { registerLogStreamSession, unregisterLogStreamSession } from "../stores/logStreamManager";
import { createStorage } from "../utils/storage";
import { useLogBuffer, type LogEntry } from "../composables/useLogBuffer";
import VirtualLogList, { type VirtualLogItem } from "./VirtualLogList.vue";

const props = withDefaults(
  defineProps<{
    envId: string | null;
    namespace: string | null;
    podName: string;
    externalContainers?: string[];
    externalContainer?: string;
    externalContainersLoading?: boolean;
    sessionId?: string;
  }>(),
  {
    externalContainers: undefined,
    externalContainer: undefined,
    externalContainersLoading: false,
    sessionId: "",
  }
);

const emit = defineEmits<{
  (e: "update:externalContainer", v: string): void;
}>();

// ─── Container logic ──────────────────────────────────────

const showContainerInToolbar = computed(() => !props.externalContainers);
const effectiveContainers = computed(() => props.externalContainers ?? containers.value);
const effectiveContainer = computed(() =>
  props.externalContainers ? (props.externalContainer ?? "") : selectedContainer.value
);
function setEffectiveContainer(v: string) {
  if (props.externalContainers) {
    emit("update:externalContainer", v);
  } else {
    selectedContainer.value = v;
  }
}

// ─── State ────────────────────────────────────────────────

const containers = ref<string[]>([]);
const selectedContainer = ref("");
const loading = ref(false);
const containersLoading = ref(false);
const error = ref<string | null>(null);
const tailLines = ref<number>(100);
const sinceSeconds = ref<number | null>(null);
const timestamps = ref(false);
const previousLogs = ref(false);
const follow = ref(true);
const streamId = ref<string | null>(null);
const streamAllowed = ref(true);
type LogRuntimePhase = "idle" | "loading" | "streaming" | "snapshot" | "error";
const runtimePhase = ref<LogRuntimePhase>("idle");
let activeRequestSeq = 0;
const displayOrder = ref<LogDisplayOrder>("asc");
const { logRefreshTrigger } = useLogStore();
const strongholdAuth = useStrongholdAuthStore();

const runtimePhaseMeta = computed(() => {
  switch (runtimePhase.value) {
    case "streaming":
      return {
        label: "流式中",
        description: "正在实时接收日志流（Follow）",
      };
    case "loading":
      return {
        label: "加载中",
        description: "正在请求日志数据",
      };
    case "snapshot":
      return {
        label: "快照模式",
        description: "当前展示静态快照，未持续跟随新日志",
      };
    case "error":
      return {
        label: "异常",
        description: "日志链路发生错误，需重试或切换模式",
      };
    default:
      return {
        label: "空闲",
        description: "等待选择目标容器后开始加载日志",
      };
  }
});

const errorCategory = computed(() => {
  const msg = (error.value || "").toLowerCase();
  if (!msg) return "unknown";
  if (
    msg.includes("stronghold") ||
    msg.includes("unlock") ||
    msg.includes("auth") ||
    msg.includes("permission denied")
  ) {
    return "auth";
  }
  if (msg.includes("utf") || msg.includes("decode") || msg.includes("编码")) {
    return "encoding";
  }
  if (
    msg.includes("timeout") ||
    msg.includes("connection") ||
    msg.includes("network") ||
    msg.includes("broken pipe")
  ) {
    return "network";
  }
  if (
    msg.includes("forbidden") ||
    msg.includes("unauthorized") ||
    msg.includes("denied") ||
    msg.includes("forbidden")
  ) {
    return "permission";
  }
  return "unknown";
});

const errorCategoryLabel = computed(() => {
  switch (errorCategory.value) {
    case "auth":
      return "认证问题";
    case "encoding":
      return "编码问题";
    case "network":
      return "网络问题";
    case "permission":
      return "权限问题";
    default:
      return "运行时错误";
  }
});

const errorHint = computed(() => {
  switch (errorCategory.value) {
    case "auth":
      return "请先确认凭证/解锁状态，再点击重试。";
    case "encoding":
      return "日志中可能包含非 UTF-8 字节，建议继续使用流式查看并导出原文排查。";
    case "network":
      return "连接可能短暂中断，建议重试或切换到快照模式查看历史日志。";
    case "permission":
      return "当前账号可能缺少日志读取权限，请检查 Kubernetes RBAC。";
    default:
      return "可先点击重试；若持续失败，请切换容器或关闭 Follow 后刷新。";
  }
});

// ─── Log buffer ───────────────────────────────────────────

const { entries, setLines, setLinesAsync, appendLines, countLinesInChunk, clear: clearBuffer, levelCounts, lineCount } = useLogBuffer();

// ─── Search / filter state ────────────────────────────────

const searchQuery = ref("");
const excludeQuery = ref("");
const regexMode = ref(false);
const currentMatchIndex = ref(0);
const searchVisible = ref(false);
const searchInputRef = ref<InstanceType<typeof NInput> | null>(null);
type LogQuickFilter = "error" | "warn" | "info" | "debug";
const selectedLevels = ref<Set<LogQuickFilter>>(new Set());
const onlyMatches = ref(false);
const contextLines = ref(-1);

// ─── Search history ───────────────────────────────────────

const SEARCH_HISTORY_KEY = "kube-flow:log-search-history";
const searchHistoryStorage = createStorage<string[]>({
  key: SEARCH_HISTORY_KEY,
  version: 1,
  fallback: [],
});
const searchHistory = ref<string[]>(searchHistoryStorage.read());
const searchInputFocused = ref(false);

function recordSearchHistory(query: string) {
  const q = query.trim();
  if (!q) return;
  const next = [q, ...searchHistory.value.filter((h) => h !== q)].slice(0, 10);
  searchHistory.value = next;
  searchHistoryStorage.write(next);
}

function applyHistoryItem(item: string) {
  searchQuery.value = item;
  searchInputFocused.value = false;
}

function onSearchBlur() {
  setTimeout(() => { searchInputFocused.value = false; }, 150);
}

const filteredHistory = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return searchHistory.value;
  return searchHistory.value.filter((h) => h.toLowerCase().includes(q));
});

// ─── Display settings ─────────────────────────────────────

const DENSITY_KEY = "kube-flow:log-density";
type DisplayDensity = "compact" | "standard" | "relaxed";
const densityStorage = createStorage<DisplayDensity>({
  key: DENSITY_KEY,
  version: 1,
  fallback: "standard",
  migrate: () => "standard",
});
const displayDensity = ref<DisplayDensity>(densityStorage.read());
watch(displayDensity, (v) => densityStorage.write(v));

const lineHeight = computed(() => {
  switch (displayDensity.value) {
    case "compact": return 16;
    case "relaxed": return 28;
    default: return 20;
  }
});
const linePaddingClass = computed(() => `density-${displayDensity.value}`);

const TAIL_OPTIONS = [
  { value: 100, label: "最近 100 行" },
  { value: 500, label: "最近 500 行" },
  { value: 1000, label: "最近 1000 行" },
  { value: 5000, label: "最近 5000 行" },
];

const sinceSelectValue = computed({
  get(): "all" | number {
    const s = sinceSeconds.value;
    if (s === null) return "all";
    if (s === 3600 || s === 21600 || s === 86400) return s;
    return "all";
  },
  set(v: "all" | number) {
    sinceSeconds.value = v === "all" ? null : v;
  },
});

const sinceOptionsForSelect = [
  { value: "all" as const, label: "全部" },
  { value: 3600, label: "最近 1 小时" },
  { value: 21600, label: "最近 6 小时" },
  { value: 86400, label: "最近 24 小时" },
];

const containerNOptions = computed(() => {
  const list = effectiveContainers.value.map((c) => ({ label: c, value: c as string }));
  if (!list.length) return [{ label: "（无）", value: "", disabled: true }];
  return [{ label: "— 选择容器 —", value: "" }, ...list];
});

const densityOptions = [
  { label: "紧凑", value: "compact" as DisplayDensity },
  { label: "标准", value: "standard" as DisplayDensity },
  { label: "宽松", value: "relaxed" as DisplayDensity },
];

const CONTEXT_OPTIONS = [
  { value: -1, label: "全部" },
  { value: 1, label: "±1行" },
  { value: 2, label: "±2行" },
  { value: 5, label: "±5行" },
  { value: 10, label: "±10行" },
];

// ─── Filtering logic (using parsed entries) ───────────────

function matchLevel(entry: LogEntry): boolean {
  if (selectedLevels.value.size === 0) return true;
  return selectedLevels.value.has(entry.level as LogQuickFilter);
}

const includedQuery = computed(() => searchQuery.value.trim());
const excludedQuery = computed(() => excludeQuery.value.trim());
const hasFocusFilter = computed(() => Boolean(includedQuery.value) || selectedLevels.value.size > 0);
const hasTextSearch = computed(() => Boolean(includedQuery.value));
const hasLevelFilter = computed(() => selectedLevels.value.size > 0);
function buildSearchRegex(pattern: string): RegExp | null {
  if (!pattern) return null;
  if (regexMode.value) {
    try { return new RegExp(pattern, "gi"); } catch { return null; }
  }
  return new RegExp(pattern.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi");
}

function textMatchesRegex(text: string, re: RegExp | null): boolean {
  if (!re) return true;
  re.lastIndex = 0;
  return re.test(text);
}

const includeRe = computed(() => buildSearchRegex(includedQuery.value));
const excludeRe = computed(() => buildSearchRegex(excludedQuery.value));

const matchingIndices = computed(() => {
  const incRe = includeRe.value;
  const excRe = excludeRe.value;
  const result: number[] = [];
  for (let i = 0; i < entries.value.length; i++) {
    const entry = entries.value[i];
    // 级别过滤：不通过则跳过
    if (!matchLevel(entry)) continue;
    // 排除过滤
    if (excRe && textMatchesRegex(entry.raw, excRe)) continue;
    // 文本搜索：未命中则跳过（无文本搜索时所有级别匹配的行都算命中）
    if (incRe && !textMatchesRegex(entry.raw, incRe)) continue;
    result.push(i);
  }
  return result;
});

const matchingIndexSet = computed(() => new Set(matchingIndices.value));

const displayedEntries = computed(() => {
  const matched = matchingIndexSet.value;
  const hasMatch = matched.size > 0;
  // 排除过滤始终生效
  const excRe = excludeRe.value;
  let base = excRe ? entries.value.filter((e) => !textMatchesRegex(e.raw, excRe)) : entries.value;

  // 级别过滤：直接筛选
  if (hasLevelFilter.value) {
    base = base.filter((e) => matchLevel(e));
  }

  let result: typeof base;
  // 文本搜索：默认高亮，仅匹配/上下文模式下才进一步筛选
  if (hasTextSearch.value && hasMatch) {
    if (onlyMatches.value) {
      result = base.filter((_, i) => matched.has(i));
    } else if (contextLines.value > 0) {
      const ctx = contextLines.value;
      result = base.filter((_, i) => {
        if (matched.has(i)) return true;
        for (const mi of matched) {
          if (Math.abs(i - mi) <= ctx) return true;
        }
        return false;
      });
    } else {
      result = base;
    }
  } else {
    result = base;
  }

  // 倒序：新日志在前
  if (displayOrder.value === "desc") {
    result = [...result].reverse();
  }
  return result;
});

const matchCount = computed(() => matchingIndices.value.length);
const filteredLineCount = computed(() => displayedEntries.value.length);

// ─── ANSI + highlight ─────────────────────────────────────

const ansiUp = new AnsiUp();

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function highlightLine(entry: VirtualLogItem): string {
  const raw = (entry as LogEntry).raw;
  let html = ansiUp.ansi_to_html(raw);
  const q = includedQuery.value;
  if (!q) return html;
  let re: RegExp;
  if (regexMode.value) {
    try { re = new RegExp(`(${q})`, "gi"); } catch { return html; }
  } else {
    re = new RegExp(`(${escapeRegex(q)})`, "gi");
  }
  const parts = html.split(/(<[^>]+>)/g);
  return parts.map((p) => (p.startsWith("<") ? p : p.replace(re, "<mark>$1</mark>"))).join("");
}

// ─── Virtual list + auto-scroll ───────────────────────────

const virtualListRef = ref<InstanceType<typeof VirtualLogList> | null>(null);
const newLogCount = ref(0);
const isDescOrder = computed(() => displayOrder.value === "desc");

/** 判断是否在「锚点」位置：正序=底部，倒序=顶部 */
function isAtAnchor(): boolean {
  const el = virtualListRef.value?.containerRef;
  if (!el) return true;
  if (isDescOrder.value) {
    return el.scrollTop < 50;
  }
  return el.scrollHeight - el.scrollTop - el.clientHeight < 50;
}

function goToMatch(delta: number) {
  if (matchCount.value === 0) return;
  const next = (currentMatchIndex.value + delta + matchCount.value) % matchCount.value;
  currentMatchIndex.value = next;
  const targetEntry = entries.value[matchingIndices.value[next]];
  if (!targetEntry) return;
  const displayIdx = displayedEntries.value.findIndex((e) => e.index === targetEntry.index);
  if (displayIdx >= 0) {
    virtualListRef.value?.scrollToIndex(displayIdx);
  }
}

function resumeAutoScroll() {
  newLogCount.value = 0;
  nextTick(() => {
    if (isDescOrder.value) {
      virtualListRef.value?.scrollToIndex(0);
    } else {
      virtualListRef.value?.scrollToBottom();
    }
  });
}

function onVirtualScroll() {
  if (isAtAnchor()) {
    newLogCount.value = 0;
  }
}

// ─── Actions ──────────────────────────────────────────────

async function handleStrongholdLocked(message: string, onConfirmed: () => void): Promise<boolean> {
  return strongholdAuth.checkAndHandle(message, onConfirmed, {
    title: "解锁日志凭证",
    description: "当前日志操作需要访问已保存凭证，请先输入 Stronghold 主密码解锁。",
  });
}

async function loadContainers() {
  if (!props.envId || !props.namespace || !props.podName) return;
  containersLoading.value = true;
  try {
    containers.value = await kubeGetPodContainers(props.envId, props.namespace, props.podName);
    if (containers.value.length > 0 && !containers.value.includes(selectedContainer.value)) {
      selectedContainer.value = containers.value[0];
    } else if (containers.value.length === 0) {
      selectedContainer.value = "";
    }
  } catch {
    containers.value = [];
    selectedContainer.value = "";
  } finally {
    containersLoading.value = false;
  }
}

async function loadDisplaySettings() {
  try {
    const settings = await logGetDisplaySettings();
    displayOrder.value = settings.order;
    tailLines.value = Math.max(1, Math.floor(settings.tailLines || 100));
  } catch {
    displayOrder.value = "asc";
    tailLines.value = 100;
  }
}

async function toggleDisplayOrder() {
  const next: LogDisplayOrder = displayOrder.value === "asc" ? "desc" : "asc";
  displayOrder.value = next;
  try {
    await logSetDisplaySettings(next, "json", tailLines.value);
  } catch { /* ignore */ }
}

async function loadLogs() {
  if (!props.envId || !props.namespace || !props.podName || follow.value) return;
  const requestSeq = ++activeRequestSeq;
  loading.value = true;
  runtimePhase.value = "loading";
  error.value = null;
  cancelStreamFlush();
  clearBuffer();
  try {
    const content = await kubePodLogs(props.envId, props.namespace, props.podName, {
      container: effectiveContainer.value || null,
      tailLines: tailLines.value,
      sinceSeconds: sinceSeconds.value ?? undefined,
      timestamps: timestamps.value,
      previous: previousLogs.value,
    });
    if (requestSeq !== activeRequestSeq) return;
    if (content.length > 200_000) {
      await setLinesAsync(content);
    } else {
      setLines(content);
    }
    runtimePhase.value = "snapshot";
  } catch (e) {
    const msg = extractErrorMessage(e);
    const isStrongholdRequired = await handleStrongholdLocked(msg, () => void loadLogs());
    if (isStrongholdRequired) return;
    if (requestSeq !== activeRequestSeq) return;
    error.value = msg;
    runtimePhase.value = "error";
  } finally {
    if (requestSeq === activeRequestSeq) {
      loading.value = false;
    }
  }
}

async function startFollow() {
  if (!streamAllowed.value || !props.envId || !props.namespace || !props.podName || !effectiveContainer.value) return;
  const requestSeq = ++activeRequestSeq;
  await stopFollow();
  loading.value = true;
  runtimePhase.value = "loading";
  error.value = null;
  cancelStreamFlush();
  clearBuffer();
  newLogCount.value = 0;
  try {
    const id = await kubePodLogStreamStart(
      props.envId, props.namespace, props.podName,
      {
        container: effectiveContainer.value,
        tailLines: tailLines.value,
        sinceSeconds: sinceSeconds.value ?? undefined,
        timestamps: timestamps.value,
        previous: previousLogs.value,
      }
    );
    if (requestSeq !== activeRequestSeq) {
      await kubePodLogStreamStop(id);
      return;
    }
    streamId.value = id;
    runtimePhase.value = "streaming";
  } catch (e) {
    const msg = extractErrorMessage(e);
    const isStrongholdRequired = await handleStrongholdLocked(msg, () => void startFollow());
    if (requestSeq !== activeRequestSeq) return;
    if (!isStrongholdRequired) error.value = msg;
    runtimePhase.value = "error";
    follow.value = false;
  } finally {
    if (requestSeq === activeRequestSeq) {
      loading.value = false;
    }
  }
}

async function stopFollow() {
  if (streamId.value) {
    await kubePodLogStreamStop(streamId.value);
    streamId.value = null;
  }
  if (runtimePhase.value === "streaming") {
    runtimePhase.value = "snapshot";
  }
}

function toggleLevel(level: LogQuickFilter) {
  const next = new Set(selectedLevels.value);
  if (next.has(level)) next.delete(level);
  else next.add(level);
  selectedLevels.value = next;
}

function clearLevels() {
  selectedLevels.value = new Set();
}

function toggleSearch() {
  searchVisible.value = !searchVisible.value;
  if (searchVisible.value) {
    nextTick(() => searchInputRef.value?.focus());
  } else {
    recordSearchHistory(searchQuery.value);
  }
}

// ─── Export ───────────────────────────────────────────────

const exportCopied = ref(false);

async function exportLogs() {
  const content = displayedEntries.value.map((e) => e.raw).join("\n");
  if (!content) return;
  try {
    await navigator.clipboard.writeText(content);
    exportCopied.value = true;
    setTimeout(() => { exportCopied.value = false; }, 2000);
  } catch {
    // fallback: select + copy via textarea
    const ta = document.createElement("textarea");
    ta.value = content;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
    exportCopied.value = true;
    setTimeout(() => { exportCopied.value = false; }, 2000);
  }
}

// ─── Watchers ─────────────────────────────────────────────

watch(
  () => [props.envId, props.namespace, props.podName] as const,
  async ([envId, ns, name]) => {
    if (envId && ns && name) {
      await stopFollow();
      await loadContainers();
      follow.value = true;
    } else {
      await stopFollow();
      follow.value = false;
      containers.value = [];
      selectedContainer.value = "";
      clearBuffer();
      error.value = null;
      runtimePhase.value = "idle";
    }
  },
  { immediate: true }
);

watch(
  () => [selectedContainer.value, props.externalContainer] as const,
  () => {
    if (props.envId && props.namespace && props.podName && effectiveContainer.value) {
      if (follow.value) startFollow();
      else loadLogs();
    }
  }
);

watch([tailLines, sinceSeconds, timestamps, previousLogs], () => {
  if (props.envId && props.namespace && props.podName && effectiveContainer.value && !follow.value)
    loadLogs();
});

watch(follow, async (on) => {
  if (!props.envId || !props.namespace || !props.podName || !effectiveContainer.value) return;
  if (on) {
    await startFollow();
  } else {
    await stopFollow();
    runtimePhase.value = "snapshot";
    loadLogs();
  }
});

watch(
  () => streamAllowed.value,
  async (allowed) => {
    if (!allowed) { await stopFollow(); return; }
    if (follow.value && props.envId && props.namespace && props.podName && effectiveContainer.value && !streamId.value) {
      await startFollow();
    }
  }
);

watch([searchQuery, excludeQuery, onlyMatches, contextLines, regexMode], () => {
  currentMatchIndex.value = 0;
});

watch(
  () => Array.from(selectedLevels.value).sort().join(","),
  () => { currentMatchIndex.value = 0; }
);

watch(logRefreshTrigger, () => loadDisplaySettings());
watch(displayOrder, () => {
  nextTick(() => {
    if (isDescOrder.value) {
      virtualListRef.value?.scrollToIndex(0);
    } else {
      virtualListRef.value?.scrollToBottom();
    }
  });
});

// ─── Keyboard shortcuts ───────────────────────────────────

function onKeydown(e: KeyboardEvent) {
  const mod = e.metaKey || e.ctrlKey;
  if (mod && e.key === "f") {
    e.preventDefault();
    searchVisible.value = true;
    nextTick(() => searchInputRef.value?.focus());
    return;
  }
  if (mod && e.key === "g") {
    e.preventDefault();
    goToMatch(e.shiftKey ? -1 : 1);
    return;
  }
  if (e.key === "F3") {
    e.preventDefault();
    goToMatch(e.shiftKey ? -1 : 1);
    return;
  }
  if (e.key === "Escape") {
    if (searchVisible.value) {
      recordSearchHistory(searchQuery.value);
      searchVisible.value = false;
      e.preventDefault();
    }
    return;
  }
  if (mod && e.shiftKey && e.key === "F") {
    e.preventDefault();
    follow.value = !follow.value;
    return;
  }
}

// ─── Stream listeners ─────────────────────────────────────

let unlistenChunk: (() => void) | null = null;
let unlistenEnd: (() => void) | null = null;
let streamDecoder = new TextDecoder("utf-8", { fatal: false });

/** 合并流式 chunk，减少逐行 IPC 触发的主线程压力 */
const STREAM_FLUSH_MS = 32;
const STREAM_FLUSH_MAX_CHARS = 65_536;
let pendingStreamText = "";
let streamFlushRaf = 0;
let streamFlushTimer: ReturnType<typeof setTimeout> | null = null;
let streamFlushAnchor = true;

function resetStreamDecoder() {
  streamDecoder = new TextDecoder("utf-8", { fatal: false });
}

function cancelStreamFlush() {
  if (streamFlushRaf) {
    cancelAnimationFrame(streamFlushRaf);
    streamFlushRaf = 0;
  }
  if (streamFlushTimer) {
    clearTimeout(streamFlushTimer);
    streamFlushTimer = null;
  }
  pendingStreamText = "";
}

function flushStreamBuffer() {
  streamFlushRaf = 0;
  if (streamFlushTimer) {
    clearTimeout(streamFlushTimer);
    streamFlushTimer = null;
  }
  if (!pendingStreamText) return;
  const text = pendingStreamText;
  const wasAtAnchor = streamFlushAnchor;
  streamFlushAnchor = true;
  pendingStreamText = "";
  appendLines(text);
  if (!wasAtAnchor) {
    newLogCount.value += countLinesInChunk(text);
  }
}

function scheduleStreamFlush(wasAtAnchor: boolean) {
  if (!wasAtAnchor) streamFlushAnchor = false;
  if (!streamFlushRaf) {
    streamFlushRaf = requestAnimationFrame(flushStreamBuffer);
  }
  if (!streamFlushTimer) {
    streamFlushTimer = setTimeout(flushStreamBuffer, STREAM_FLUSH_MS);
  }
  if (pendingStreamText.length >= STREAM_FLUSH_MAX_CHARS) {
    flushStreamBuffer();
  }
}

async function setupStreamListeners() {
  unlistenChunk?.();
  unlistenEnd?.();
  resetStreamDecoder();
  unlistenChunk = await listen<{ stream_id: string; chunk_bytes: number[] }>("pod-log-chunk", (ev) => {
    if (ev.payload?.stream_id === streamId.value) {
      const wasAtAnchor = isAtAnchor();
      const chunkText = streamDecoder.decode(new Uint8Array(ev.payload.chunk_bytes), { stream: true });
      if (!chunkText) return;
      pendingStreamText += chunkText;
      scheduleStreamFlush(wasAtAnchor);
    }
  });
  unlistenEnd = await listen<{ stream_id: string; error?: string }>("pod-log-stream-end", (ev) => {
    if (ev.payload?.stream_id === streamId.value) {
      const finalChunk = streamDecoder.decode();
      if (finalChunk) {
        pendingStreamText += finalChunk;
        flushStreamBuffer();
      } else {
        flushStreamBuffer();
      }
      resetStreamDecoder();
      streamId.value = null;
      follow.value = false;
      runtimePhase.value = ev.payload?.error ? "error" : "snapshot";
      if (ev.payload?.error) error.value = ev.payload.error;
    }
  });
}

setupStreamListeners();

onMounted(() => {
  loadDisplaySettings();
  window.addEventListener("keydown", onKeydown);
  if (props.sessionId) {
    registerLogStreamSession(props.sessionId, {
      setStreamAllowed: async (allowed) => { streamAllowed.value = allowed; },
    });
  }
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  cancelStreamFlush();
  if (props.sessionId) unregisterLogStreamSession(props.sessionId);
  stopFollow();
});
</script>

<template>
  <div class="pod-log-panel">
    <!-- Container bar (only when not externally managed) -->
    <div v-if="showContainerInToolbar" class="target-bar">
      <div class="toolbar-row">
        <label class="field-label">容器</label>
        <NSelect
          :value="effectiveContainer"
          class="select kf-select-toolbar"
          size="small"
          :options="containerNOptions"
          :disabled="containersLoading || effectiveContainers.length === 0"
          @update:value="(v: string) => setEffectiveContainer(typeof v === 'string' ? v : String(v ?? ''))"
        />
      </div>
    </div>

    <!-- Main toolbar -->
    <div class="log-toolbar">
      <div class="toolbar-left">
        <!-- Follow switch -->
        <div class="follow-control">
          <NSwitch
            :value="follow"
            :disabled="!effectiveContainer || loading"
            size="small"
            @update:value="(v: boolean) => follow = v"
          >
            <template #checked>
              <span class="follow-label">Follow</span>
            </template>
            <template #unchecked>
              <span class="follow-label">Follow</span>
            </template>
          </NSwitch>
          <span v-if="follow && streamId" class="follow-pulse" />
        </div>

        <!-- Level quick filters -->
        <div class="quick-filter-group">
          <NButton
            size="tiny"
            :secondary="selectedLevels.size !== 0"
            :type="selectedLevels.size === 0 ? 'primary' : 'default'"
            class="quick-filter-btn"
            @click="clearLevels"
          >全部</NButton>
          <NButton
            size="tiny"
            :secondary="!selectedLevels.has('error')"
            :type="selectedLevels.has('error') ? 'error' : 'default'"
            class="quick-filter-btn"
            @click="toggleLevel('error')"
          >
            <span class="level-dot error" />错误
            <span v-if="levelCounts.error" class="level-count">{{ levelCounts.error }}</span>
          </NButton>
          <NButton
            size="tiny"
            :secondary="!selectedLevels.has('warn')"
            :type="selectedLevels.has('warn') ? 'warning' : 'default'"
            class="quick-filter-btn"
            @click="toggleLevel('warn')"
          >
            <span class="level-dot warn" />警告
            <span v-if="levelCounts.warn" class="level-count">{{ levelCounts.warn }}</span>
          </NButton>
          <NButton
            size="tiny"
            :secondary="!selectedLevels.has('info')"
            :type="selectedLevels.has('info') ? 'info' : 'default'"
            class="quick-filter-btn"
            @click="toggleLevel('info')"
          >
            <span class="level-dot info" />信息
          </NButton>
          <NButton
            size="tiny"
            :secondary="!selectedLevels.has('debug')"
            :type="selectedLevels.has('debug') ? 'primary' : 'default'"
            class="quick-filter-btn"
            @click="toggleLevel('debug')"
          >
            <span class="level-dot debug" />调试
          </NButton>
        </div>
      </div>

      <div class="toolbar-right">
        <NTooltip trigger="hover">
          <template #trigger>
            <span class="runtime-phase-badge" :class="'phase-' + runtimePhase">
              {{ runtimePhaseMeta.label }}
            </span>
          </template>
          <span>{{ runtimePhaseMeta.description }}</span>
        </NTooltip>

        <!-- Search toggle -->
        <NTooltip trigger="hover">
          <template #trigger>
            <NButton
              size="small"
              :type="searchVisible ? 'primary' : 'default'"
              secondary
              @click="toggleSearch"
            >搜索</NButton>
          </template>
          <span>Ctrl+F</span>
        </NTooltip>

        <!-- Settings popover -->
        <NPopover trigger="click" placement="bottom-end" :width="280">
          <template #trigger>
            <NButton size="small" secondary>⋯</NButton>
          </template>
          <div class="settings-popover">
            <div class="setting-row">
              <label class="setting-label">行数</label>
              <NSelect v-model:value="tailLines" size="small" :options="TAIL_OPTIONS" :disabled="follow" class="setting-select" />
            </div>
            <div class="setting-row">
              <label class="setting-label">时间范围</label>
              <NSelect v-model:value="sinceSelectValue" size="small" :options="sinceOptionsForSelect" :disabled="follow" class="setting-select" />
            </div>
            <div class="setting-row">
              <label class="setting-label">时间戳</label>
              <NCheckbox v-model:checked="timestamps" :disabled="follow" />
            </div>
            <div class="setting-row">
              <label class="setting-label">重启前日志</label>
              <NCheckbox v-model:checked="previousLogs" />
            </div>
            <div class="setting-row">
              <label class="setting-label">密度</label>
              <NSelect v-model:value="displayDensity" size="small" :options="densityOptions" class="setting-select" />
            </div>
            <div class="setting-row">
              <label class="setting-label">排序</label>
              <NButton size="small" quaternary @click="toggleDisplayOrder">
                {{ displayOrder === 'asc' ? '正序 (旧→新)' : '倒序 (新→旧)' }}
              </NButton>
            </div>
            <div class="setting-divider" />
            <NButton
              size="small"
              block
              :disabled="loading || !effectiveContainer || follow"
              :loading="loading"
              @click="loadLogs"
            >刷新</NButton>
            <NButton size="small" block @click="exportLogs" :disabled="filteredLineCount === 0">{{ exportCopied ? '已复制 ✓' : '复制日志' }}</NButton>
          </div>
        </NPopover>
      </div>
    </div>

    <!-- Search bar -->
    <div v-if="searchVisible" class="search-toolbar">
      <div class="search-filters">
        <div class="search-input-wrapper">
          <NInput
            ref="searchInputRef"
            v-model:value="searchQuery"
            class="search-input"
            size="small"
            placeholder="包含关键词…"
            :title="matchCount ? `${matchCount} 处匹配` : ''"
            @focus="searchInputFocused = true"
            @blur="onSearchBlur"
          />
          <div v-if="searchInputFocused && filteredHistory.length > 0" class="search-history-dropdown">
            <button
              v-for="item in filteredHistory"
              :key="item"
              type="button"
              class="search-history-item"
              @mousedown.prevent="applyHistoryItem(item)"
            >{{ item }}</button>
          </div>
        </div>
        <NInput
          v-model:value="excludeQuery"
          class="search-input exclude"
          size="small"
          placeholder="排除关键词…"
        />
        <NTooltip trigger="hover">
          <template #trigger>
            <NButton
              size="tiny"
              :type="regexMode ? 'primary' : 'default'"
              :secondary="!regexMode"
              @click="regexMode = !regexMode"
            >.*</NButton>
          </template>
          <span>正则表达式</span>
        </NTooltip>
      </div>
      <div class="search-tools">
        <div class="checkbox-label compact">
          <NCheckbox v-model:checked="onlyMatches">仅匹配</NCheckbox>
        </div>
        <div class="toolbar-row">
          <label class="field-label">上下文</label>
          <NSelect
            v-model:value="contextLines"
            class="select kf-select-toolbar kf-select-toolbar--compact"
            size="small"
            :options="CONTEXT_OPTIONS"
            :disabled="onlyMatches"
          />
        </div>
      </div>
      <div class="search-nav">
        <NButton text class="btn-nav" :disabled="matchCount === 0" title="上一个 (Ctrl+Shift+G)" @click="goToMatch(-1)">↑</NButton>
        <span v-if="searchQuery.trim()" class="match-info">{{ currentMatchIndex + 1 }}/{{ matchCount }}</span>
        <NButton text class="btn-nav" :disabled="matchCount === 0" title="下一个 (Ctrl+G)" @click="goToMatch(1)">↓</NButton>
      </div>
    </div>

    <!-- Filter summary -->
    <div v-if="lineCount > 0 || loading" class="filter-summary">
      <span>共 {{ lineCount }} 行</span>
      <span v-if="hasFocusFilter && matchCount > 0">命中 {{ matchCount }} 行</span>
      <span v-if="hasFocusFilter && matchCount > 0 && filteredLineCount !== lineCount">显示 {{ filteredLineCount }} 行</span>
      <div v-if="matchCount > 0" class="filter-nav">
        <NButton text class="btn-nav" title="上一个 (Ctrl+Shift+G)" @click="goToMatch(-1)">↑</NButton>
        <span class="match-info">{{ currentMatchIndex + 1 }}/{{ matchCount }}</span>
        <NButton text class="btn-nav" title="下一个 (Ctrl+G)" @click="goToMatch(1)">↓</NButton>
      </div>
    </div>

    <!-- Error -->
    <div v-if="error" class="error-banner">
      <div class="error-banner-main">
        <span class="error-category-badge">{{ errorCategoryLabel }}</span>
        <span class="error-message">{{ error }}</span>
      </div>
      <div class="error-hint">{{ errorHint }}</div>
      <NButton size="tiny" @click="follow ? startFollow() : loadLogs()">重试</NButton>
    </div>

    <!-- Log content -->
    <template v-else>
      <template v-if="(showContainerInToolbar && containersLoading) || (!showContainerInToolbar && props.externalContainersLoading)">
        <div class="log-content log-content-themed">
          <div class="log-skeleton">
            <div v-for="i in 8" :key="i" class="skeleton-line" />
          </div>
        </div>
      </template>
      <template v-else-if="containers.length === 0 && props.podName">
        <div class="log-content log-content-themed">
          <p class="log-empty">未找到容器</p>
        </div>
      </template>
      <template v-else-if="!effectiveContainer">
        <div class="log-content log-content-themed">
          <p class="log-empty">请选择容器</p>
        </div>
      </template>
      <template v-else-if="filteredLineCount > 0">
        <div class="log-content-wrapper log-content-themed">
          <VirtualLogList
            ref="virtualListRef"
            :items="displayedEntries"
            :item-height="lineHeight"
            :buffer="30"
            :auto-anchor="isDescOrder ? 'top' : 'bottom'"
            :content-class="'log-scroll ' + linePaddingClass"
            :content-style="{ flex: '1', minHeight: '0' }"
            :render-line="highlightLine"
            @scroll="onVirtualScroll"
          />
          <!-- New logs badge -->
          <Transition name="badge-fade">
            <button
              v-if="newLogCount > 0 && follow"
              class="new-logs-badge"
              :class="isDescOrder ? 'badge-top' : 'badge-bottom'"
              @click="resumeAutoScroll"
            >
              {{ isDescOrder ? '↑' : '↓' }} {{ newLogCount }} 条新日志
            </button>
          </Transition>
        </div>
      </template>
      <template v-else>
        <div class="log-content log-content-themed">
          <p class="log-empty">
            {{ lineCount > 0 ? '没有匹配的日志（尝试关闭"仅匹配"）' : '暂无日志' }}
            <NButton v-if="!follow && effectiveContainer" size="tiny" @click="loadLogs" style="margin-top: 0.5rem">加载日志</NButton>
            <NButton v-if="!follow && effectiveContainer" size="tiny" @click="follow = true" style="margin-top: 0.5rem; margin-left: 0.5rem">开启 Follow</NButton>
          </p>
        </div>
      </template>
    </template>
  </div>
</template>

<style scoped>
.pod-log-panel {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.target-bar {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 0 0.5rem 1rem;
  border-bottom: 1px solid var(--kf-border);
  flex-shrink: 0;
}
.log-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.5rem 0.75rem;
  border-bottom: 1px solid var(--kf-border);
  flex-shrink: 0;
  flex-wrap: wrap;
}
.toolbar-left {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex-wrap: wrap;
}
.toolbar-right {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
.runtime-phase-badge {
  display: inline-flex;
  align-items: center;
  padding: 0.2rem 0.55rem;
  border-radius: 999px;
  border: 1px solid var(--kf-border);
  font-size: 0.72rem;
  font-weight: 700;
  line-height: 1.2;
  white-space: nowrap;
}
.runtime-phase-badge.phase-idle {
  color: var(--kf-text-secondary);
  background: var(--kf-bg-soft);
}
.runtime-phase-badge.phase-loading {
  color: var(--kf-info);
  background: color-mix(in srgb, var(--kf-info) 14%, transparent);
  border-color: color-mix(in srgb, var(--kf-info) 36%, var(--kf-border));
}
.runtime-phase-badge.phase-streaming {
  color: var(--kf-success);
  background: color-mix(in srgb, var(--kf-success) 14%, transparent);
  border-color: color-mix(in srgb, var(--kf-success) 36%, var(--kf-border));
}
.runtime-phase-badge.phase-snapshot {
  color: var(--kf-warning);
  background: color-mix(in srgb, var(--kf-warning) 14%, transparent);
  border-color: color-mix(in srgb, var(--kf-warning) 36%, var(--kf-border));
}
.runtime-phase-badge.phase-error {
  color: var(--kf-danger);
  background: color-mix(in srgb, var(--kf-danger) 14%, transparent);
  border-color: color-mix(in srgb, var(--kf-danger) 36%, var(--kf-border));
}

/* Follow control */
.follow-control {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}
.follow-label {
  font-size: 0.72rem;
  font-weight: 600;
}
.follow-pulse {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--kf-success);
  animation: pulse 1.5s ease-in-out infinite;
}
@keyframes pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.4; transform: scale(0.8); }
}

/* Level quick filters */
.quick-filter-group {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
}
.quick-filter-btn {
  font-size: 0.75rem;
}
.level-dot {
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  margin-right: 0.2rem;
}
.level-dot.error { background: var(--kf-danger); }
.level-dot.warn { background: var(--kf-warning); }
.level-dot.info { background: var(--kf-info); }
.level-dot.debug { background: #8b5cf6; }
.level-count {
  font-size: 0.65rem;
  opacity: 0.7;
  margin-left: 0.15rem;
}

/* Toolbar rows */
.toolbar-row {
  display: flex;
  align-items: center;
  gap: 0.35rem;
}
.field-label {
  font-size: 0.8125rem;
  color: var(--kf-text-secondary);
  white-space: nowrap;
  flex-shrink: 0;
}
.select {
  padding: 0.25rem 0.5rem;
  border: 1px solid var(--kf-border);
  border-radius: 4px;
  font-size: 0.8125rem;
  min-width: 120px;
}
.checkbox-label {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.8125rem;
  color: var(--kf-text-secondary);
}

/* Settings popover */
.settings-popover {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}
.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}
.setting-label {
  font-size: 0.8125rem;
  color: var(--kf-text-secondary);
  flex-shrink: 0;
}
.setting-select {
  min-width: 120px;
}
.setting-divider {
  height: 1px;
  background: var(--kf-border);
  margin: 0.2rem 0;
}

/* Search toolbar */
.search-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.35rem 0.75rem;
  border-bottom: 1px solid var(--kf-border);
  flex-shrink: 0;
  flex-wrap: wrap;
}
.search-filters {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
}
.search-tools {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-left: auto;
  flex-wrap: wrap;
}
.search-nav {
  display: flex;
  align-items: center;
  gap: 0.35rem;
}
.search-input {
  min-width: 180px;
  width: 220px;
  flex-shrink: 0;
}
.search-input.exclude {
  width: 180px;
  min-width: 150px;
}
.search-input-wrapper {
  position: relative;
}
.search-history-dropdown {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  z-index: 20;
  max-height: 200px;
  overflow-y: auto;
  border: 1px solid var(--kf-border);
  border-radius: 8px;
  background: var(--kf-surface-strong);
  box-shadow: var(--kf-shadow-md);
  padding: 0.25rem;
}
.search-history-item {
  display: block;
  width: 100%;
  padding: 0.35rem 0.6rem;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--kf-text-primary);
  font-size: 0.78rem;
  text-align: left;
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.search-history-item:hover {
  background: var(--kf-bg-soft);
}
.compact { font-size: 0.78rem; }
.btn-nav {
  padding: 0.2rem 0.4rem;
  border: 1px solid var(--kf-border);
  border-radius: 4px;
  background: var(--kf-surface-strong);
  font-size: 0.75rem;
  cursor: pointer;
}
.btn-nav:hover:not(:disabled) { background: var(--kf-bg-soft); }
.btn-nav:disabled { opacity: 0.5; cursor: not-allowed; }
.match-info {
  font-size: 0.75rem;
  color: var(--kf-text-secondary);
  min-width: 3rem;
  text-align: center;
}

/* Filter summary */
.filter-summary {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 0.3rem 0.75rem;
  border-bottom: 1px solid var(--kf-border);
  font-size: 0.75rem;
  color: var(--kf-text-secondary);
  flex-shrink: 0;
  flex-wrap: wrap;
}
.filter-nav {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  margin-left: auto;
}

/* Error */
.error-banner {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 0.4rem 0.75rem;
  align-items: center;
  padding: 0.5rem 0.75rem;
  color: var(--kf-danger);
  font-size: 0.8125rem;
  flex-shrink: 0;
}
.error-banner-main {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 0.45rem;
}
.error-category-badge {
  display: inline-flex;
  align-items: center;
  padding: 0.1rem 0.4rem;
  border-radius: 999px;
  font-size: 0.68rem;
  font-weight: 700;
  color: var(--kf-danger);
  background: color-mix(in srgb, var(--kf-danger) 14%, transparent);
  border: 1px solid color-mix(in srgb, var(--kf-danger) 28%, var(--kf-border));
  white-space: nowrap;
}
.error-message {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.error-hint {
  grid-column: 1 / 2;
  color: var(--kf-text-secondary);
  font-size: 0.75rem;
}

/* Log content area */
.log-content {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.log-content-wrapper {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  position: relative;
}
.log-content-themed {
  background: var(--kf-surface-strong);
  color: var(--kf-text-primary);
}
.log-scroll {
  font-family: ui-monospace, monospace;
  font-size: 0.8125rem;
  line-height: 1.5;
  padding: 0.75rem;
}
.density-compact .virtual-log-line { padding: 0; }
.density-standard .virtual-log-line { padding: 0.05rem 0; }
.density-relaxed .virtual-log-line { padding: 0.15rem 0; }

.log-content-themed :deep(mark) {
  background: #fbbf24;
  color: #1e293b;
  padding: 0 0.1em;
  border-radius: 2px;
}
.log-content-themed :deep(.log-line-match) {
  background: rgba(239, 68, 68, 0.12);
}
.log-empty {
  color: var(--kf-text-secondary);
  font-style: italic;
  margin: 2rem;
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.5rem;
}

/* Skeleton loading */
.log-skeleton {
  padding: 0.75rem;
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}
.skeleton-line {
  height: 14px;
  border-radius: 4px;
  background: linear-gradient(90deg, var(--kf-bg-soft) 25%, var(--kf-border) 50%, var(--kf-bg-soft) 75%);
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite;
}
.skeleton-line:nth-child(odd) { width: 80%; }
.skeleton-line:nth-child(even) { width: 60%; }
@keyframes shimmer {
  0% { background-position: 200% 0; }
  100% { background-position: -200% 0; }
}

/* New logs badge */
.new-logs-badge {
  position: absolute;
  left: 50%;
  transform: translateX(-50%);
  padding: 0.4rem 1rem;
  border-radius: 999px;
  background: var(--kf-primary);
  color: #fff;
  font-size: 0.78rem;
  font-weight: 600;
  border: none;
  cursor: pointer;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  z-index: 10;
  white-space: nowrap;
}
.new-logs-badge.badge-bottom {
  bottom: 1rem;
}
.new-logs-badge.badge-top {
  top: 1rem;
}
.new-logs-badge:hover {
  filter: brightness(1.1);
}
.badge-fade-enter-active,
.badge-fade-leave-active {
  transition: opacity 0.2s, transform 0.2s;
}
.badge-fade-enter-from.badge-bottom,
.badge-fade-leave-to.badge-bottom {
  opacity: 0;
  transform: translateX(-50%) translateY(8px);
}
.badge-fade-enter-from.badge-top,
.badge-fade-leave-to.badge-top {
  opacity: 0;
  transform: translateX(-50%) translateY(-8px);
}

</style>
