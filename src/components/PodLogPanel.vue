<script setup lang="ts">
import { ref, watch, computed, onUnmounted } from "vue";
import { listen } from "@tauri-apps/api/event";
import { AnsiUp } from "ansi_up";
import {
  kubeGetPodContainers,
  kubePodLogs,
  kubePodLogStreamStart,
  kubePodLogStreamStop,
} from "../api/kube";

const props = withDefaults(
  defineProps<{
    envId: string | null;
    namespace: string | null;
    podName: string;
    /** 由父组件提供容器列表时，不在此处展示容器选择器 */
    externalContainers?: string[];
    externalContainer?: string;
    /** 外部容器列表加载中（仅当 externalContainers 时有效） */
    externalContainersLoading?: boolean;
  }>(),
  {
    externalContainers: undefined,
    externalContainer: undefined,
    externalContainersLoading: false,
  }
);

const emit = defineEmits<{
  (e: "update:externalContainer", v: string): void;
}>();

const showContainerInToolbar = computed(() => !props.externalContainers);
const effectiveContainers = computed(() => props.externalContainers ?? containers.value);
/** 实际用于拉取日志的容器名 */
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

const containers = ref<string[]>([]);
const selectedContainer = ref<string>("");
const rawContent = ref("");
const loading = ref(false);
const containersLoading = ref(false);
const error = ref<string | null>(null);
const tailLines = ref<number>(500);
const sinceSeconds = ref<number | null>(null);
const timestamps = ref(false);
const follow = ref(true);
const streamId = ref<string | null>(null);
const searchQuery = ref("");
const currentMatchIndex = ref(0);
const logContentRef = ref<HTMLElement | null>(null);

const LOG_BG_KEY = "kube-flow:log-bg-theme";
type LogBgTheme = "light" | "dark";
function getInitialLogBgTheme(): LogBgTheme {
  try {
    const s = localStorage.getItem(LOG_BG_KEY);
    if (s === "light" || s === "dark") return s;
  } catch {}
  return "light";
}
const logBgTheme = ref<LogBgTheme>(getInitialLogBgTheme());
watch(logBgTheme, (v) => {
  try {
    localStorage.setItem(LOG_BG_KEY, v);
  } catch {}
});

const TAIL_OPTIONS = [
  { value: 100, label: "最近 100 行" },
  { value: 500, label: "最近 500 行" },
  { value: 1000, label: "最近 1000 行" },
  { value: 5000, label: "最近 5000 行" },
];

const SINCE_OPTIONS = [
  { value: null as number | null, label: "全部" },
  { value: 3600, label: "最近 1 小时" },
  { value: 21600, label: "最近 6 小时" },
  { value: 86400, label: "最近 24 小时" },
];

const logLines = computed(() => {
  const s = rawContent.value;
  if (!s.trim()) return [];
  return s.split("\n");
});

const matchIndices = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return [];
  return logLines.value
    .map((line, i) => (line.toLowerCase().includes(q) ? i : -1))
    .filter((i) => i >= 0);
});

const matchCount = computed(() => matchIndices.value.length);

const ansiUp = new AnsiUp();

/** 将 ANSI 转义转为 HTML，并在文本内容中高亮搜索词（不修改标签属性） */
function highlightLine(line: string): string {
  let html = ansiUp.ansi_to_html(line);
  const q = searchQuery.value.trim();
  if (!q) return html;
  const re = new RegExp(`(${escapeRegex(q)})`, "gi");
  const parts = html.split(/(<[^>]+>)/g);
  return parts
    .map((p) => (p.startsWith("<") ? p : p.replace(re, "<mark>$1</mark>")))
    .join("");
}

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function goToMatch(delta: number) {
  if (matchCount.value === 0) return;
  const next = (currentMatchIndex.value + delta + matchCount.value) % matchCount.value;
  currentMatchIndex.value = next;
  scrollToMatch(matchIndices.value[next]);
}

function scrollToMatch(lineIndex: number) {
  const el = logContentRef.value;
  if (!el) return;
  const lineEl = el.querySelector(`[data-line-index="${lineIndex}"]`);
  lineEl?.scrollIntoView({ block: "center", behavior: "smooth" });
}

async function loadContainers() {
  if (!props.envId || !props.namespace || !props.podName) return;
  containersLoading.value = true;
  try {
    containers.value = await kubeGetPodContainers(
      props.envId,
      props.namespace,
      props.podName
    );
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

async function loadLogs() {
  if (!props.envId || !props.namespace || !props.podName || follow.value) return;
  loading.value = true;
  error.value = null;
  rawContent.value = "";
  try {
    rawContent.value = await kubePodLogs(props.envId, props.namespace, props.podName, {
      container: effectiveContainer.value || null,
      tailLines: tailLines.value,
      sinceSeconds: sinceSeconds.value ?? undefined,
      timestamps: timestamps.value,
    });
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
}

async function startFollow() {
  if (!props.envId || !props.namespace || !props.podName || !effectiveContainer.value) return;
  await stopFollow();
  loading.value = true;
  error.value = null;
  rawContent.value = "";
  try {
    const id = await kubePodLogStreamStart(
      props.envId,
      props.namespace,
      props.podName,
      {
        container: effectiveContainer.value,
        tailLines: tailLines.value,
        sinceSeconds: sinceSeconds.value ?? undefined,
        timestamps: timestamps.value,
      }
    );
    streamId.value = id;
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    follow.value = false;
  } finally {
    loading.value = false;
  }
}

async function stopFollow() {
  if (streamId.value) {
    await kubePodLogStreamStop(streamId.value);
    streamId.value = null;
  }
}

watch(
  () => [props.envId, props.namespace, props.podName] as const,
  async ([envId, ns, name]) => {
    if (envId && ns && name) {
      await stopFollow();
      follow.value = false;
      loadContainers();
    } else {
      await stopFollow();
      follow.value = false;
      containers.value = [];
      selectedContainer.value = "";
      rawContent.value = "";
      error.value = null;
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

watch([tailLines, sinceSeconds, timestamps], () => {
  if (props.envId && props.namespace && props.podName && effectiveContainer.value && !follow.value)
    loadLogs();
});

watch(
  () => containers.value,
  (list) => {
    if (list.length > 0 && props.envId && props.namespace && props.podName) {
      if (follow.value) startFollow();
      else loadLogs();
    }
  },
  { deep: true }
);

watch(follow, async (on) => {
  if (on) {
    await startFollow();
  } else {
    await stopFollow();
    if (props.envId && props.namespace && props.podName && effectiveContainer.value) {
      loadLogs();
    }
  }
});

watch(searchQuery, () => {
  currentMatchIndex.value = 0;
});

onUnmounted(() => {
  stopFollow();
});

let unlistenChunk: (() => void) | null = null;
let unlistenEnd: (() => void) | null = null;

async function setupStreamListeners() {
  unlistenChunk?.();
  unlistenEnd?.();
  unlistenChunk = await listen<{ stream_id: string; chunk: string }>(
    "pod-log-chunk",
    (ev) => {
      if (ev.payload?.stream_id === streamId.value) {
        rawContent.value += ev.payload.chunk;
      }
    }
  );
  unlistenEnd = await listen<{ stream_id: string; error?: string }>(
    "pod-log-stream-end",
    (ev) => {
      if (ev.payload?.stream_id === streamId.value) {
        streamId.value = null;
        follow.value = false;
        if (ev.payload?.error) {
          error.value = ev.payload.error;
        }
        if (props.envId && props.namespace && props.podName && effectiveContainer.value) {
          loadLogs();
        }
      }
    }
  );
}

setupStreamListeners();
</script>

<template>
  <div class="pod-log-panel">
    <div v-if="showContainerInToolbar" class="target-bar">
      <div class="toolbar-row">
        <label class="field-label">容器</label>
        <select
          :value="effectiveContainer"
          class="select"
          :disabled="containersLoading || effectiveContainers.length === 0"
          @change="setEffectiveContainer(($event.target as HTMLSelectElement).value)"
        >
          <option value="">-- 选择容器 --</option>
          <option v-for="c in effectiveContainers" :key="c" :value="c">{{ c }}</option>
        </select>
      </div>
    </div>
    <div class="log-toolbar">
      <div class="toolbar-row">
        <label class="field-label">行数</label>
        <select v-model="tailLines" class="select" :disabled="follow">
          <option v-for="o in TAIL_OPTIONS" :key="String(o.value)" :value="o.value">
            {{ o.label }}
          </option>
        </select>
      </div>
      <div class="toolbar-row">
        <label class="field-label">时间范围</label>
        <select v-model="sinceSeconds" class="select" :disabled="follow">
          <option v-for="o in SINCE_OPTIONS" :key="o.value ?? 'all'" :value="o.value">
            {{ o.label }}
          </option>
        </select>
      </div>
      <label class="checkbox-label">
        <input v-model="timestamps" type="checkbox" :disabled="follow" />
        <span>时间戳</span>
      </label>
      <label class="checkbox-label follow-toggle">
        <input v-model="follow" type="checkbox" :disabled="!effectiveContainer || loading" />
        <span>Follow</span>
      </label>
      <div class="toolbar-row">
        <label class="field-label">底色</label>
        <select v-model="logBgTheme" class="select">
          <option value="light">白色</option>
          <option value="dark">黑色</option>
        </select>
      </div>
      <button
        type="button"
        class="btn-refresh"
        :disabled="loading || !effectiveContainer || follow"
        @click="loadLogs"
      >
        {{ loading ? "加载中…" : "刷新" }}
      </button>
    </div>
    <div v-if="logLines.length > 0" class="search-toolbar">
      <div class="search-nav">
        <button
          type="button"
          class="btn-nav"
          :disabled="matchCount === 0"
          title="上一个"
          @click="goToMatch(-1)"
        >
          ↑
        </button>
        <span v-if="searchQuery.trim()" class="match-info">
          {{ currentMatchIndex + 1 }}/{{ matchCount }}
        </span>
        <button
          type="button"
          class="btn-nav"
          :disabled="matchCount === 0"
          title="下一个"
          @click="goToMatch(1)"
        >
          ↓
        </button>
      </div>
      <input
        v-model="searchQuery"
        type="text"
        class="search-input"
        placeholder="搜索…"
        :title="matchCount ? `${matchCount} 处匹配` : ''"
      />
    </div>
    <div v-if="error" class="error-banner">{{ error }}</div>
    <div
      v-else
      class="log-content"
      :class="logBgTheme === 'light' ? 'log-content-light' : 'log-content-dark'"
      role="log"
    >
      <template v-if="(showContainerInToolbar && containersLoading) || (!showContainerInToolbar && props.externalContainersLoading)">
        <p class="log-empty">加载容器列表…</p>
      </template>
      <template v-else-if="containers.length === 0 && props.podName">
        <p class="log-empty">未找到容器</p>
      </template>
      <template v-else-if="!effectiveContainer">
        <p class="log-empty">请选择容器</p>
      </template>
      <template v-else-if="logLines.length">
        <div
          ref="logContentRef"
          class="log-scroll"
        >
          <div
            v-for="(line, i) in logLines"
            :key="i"
            :data-line-index="i"
            class="log-line"
            :class="{
              'log-line-current-match':
                searchQuery.trim() &&
                matchCount > 0 &&
                matchIndices[currentMatchIndex] === i,
            }"
            v-html="highlightLine(line)"
          />
        </div>
      </template>
      <template v-else>
        <p class="log-empty">暂无日志</p>
      </template>
    </div>
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
  border-bottom: 1px solid #e2e8f0;
  flex-shrink: 0;
}
.log-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.75rem 1rem;
  padding: 0.5rem 0 0.5rem 1rem;
  border-bottom: 1px solid #e2e8f0;
  flex-shrink: 0;
}
.toolbar-row {
  display: flex;
  align-items: center;
  gap: 0.35rem;
}
.field-label {
  font-size: 0.8125rem;
  color: #64748b;
}
.select {
  padding: 0.25rem 0.5rem;
  border: 1px solid #e2e8f0;
  border-radius: 4px;
  font-size: 0.8125rem;
  min-width: 120px;
}
.checkbox-label {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.8125rem;
  color: #64748b;
  cursor: pointer;
}
.btn-refresh {
  padding: 0.35rem 0.75rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  background: #fff;
  font-size: 0.8125rem;
  cursor: pointer;
}
.btn-refresh:hover:not(:disabled) {
  background: #f8fafc;
}
.btn-refresh:disabled {
  opacity: 0.7;
  cursor: not-allowed;
}
.follow-toggle span {
  color: #2563eb;
}
.search-toolbar {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.5rem;
  padding: 0.35rem 0 0.35rem 1rem;
  border-bottom: 1px solid #e2e8f0;
  flex-shrink: 0;
}
.search-nav {
  display: flex;
  align-items: center;
  gap: 0.35rem;
}
.search-input {
  padding: 0.25rem 0.5rem;
  border: 1px solid #e2e8f0;
  border-radius: 4px;
  font-size: 0.8125rem;
  min-width: 200px;
  width: 240px;
  flex-shrink: 0;
}
.btn-nav {
  padding: 0.2rem 0.4rem;
  border: 1px solid #e2e8f0;
  border-radius: 4px;
  background: #fff;
  font-size: 0.75rem;
  cursor: pointer;
}
.btn-nav:hover:not(:disabled) {
  background: #f8fafc;
}
.btn-nav:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.match-info {
  font-size: 0.75rem;
  color: #64748b;
  min-width: 3rem;
}
.error-banner {
  padding: 0.5rem 0;
  color: #dc2626;
  font-size: 0.8125rem;
  flex-shrink: 0;
}
.log-content {
  flex: 1;
  padding: 0.75rem;
  overflow: auto;
  font-family: ui-monospace, monospace;
  font-size: 0.8125rem;
  line-height: 1.5;
}
.log-content-light {
  background: #ffffff;
  color: #1e293b;
}
.log-content-dark {
  background: #1e293b;
  color: #e2e8f0;
}
.log-scroll {
  min-height: min-content;
}
.log-line {
  white-space: pre-wrap;
  word-break: break-all;
  padding: 0.05rem 0;
}
.log-content-light .log-line :deep(mark) {
  background: #fbbf24;
  color: #1e293b;
  padding: 0 0.1em;
  border-radius: 2px;
}
.log-content-dark .log-line :deep(mark) {
  background: #fbbf24;
  color: #1e293b;
  padding: 0 0.1em;
  border-radius: 2px;
}
.log-content-light .log-line-current-match {
  background: rgba(251, 191, 36, 0.35);
}
.log-content-dark .log-line-current-match {
  background: rgba(251, 191, 36, 0.2);
}
.log-content-light .log-empty {
  color: #64748b;
  font-style: italic;
  margin: 0;
}
.log-content-dark .log-empty {
  color: #94a3b8;
  font-style: italic;
  margin: 0;
}
</style>
