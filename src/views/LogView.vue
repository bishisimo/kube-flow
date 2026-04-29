<script setup lang="ts">
import { ref, computed, watch, onMounted } from "vue";
import { NAlert, NButton, NEmpty, NScrollbar } from "naive-ui";
import {
  logRead,
  logClear,
  logGetDisplaySettings,
  type LogDisplayOrder,
  type LogDisplayFormat,
} from "../api/log";
import { useLogStore } from "../stores/log";
import { extractErrorMessage } from "../utils/errorMessage";

const props = defineProps<{ visible?: boolean }>();
const { logRefreshTrigger } = useLogStore();
const rawContent = ref("");
const loading = ref(false);
const clearing = ref(false);
const error = ref<string | null>(null);
const displayOrder = ref<LogDisplayOrder>("asc");
const displayFormat = ref<LogDisplayFormat>("json");

interface LogLine {
  level: string;
  text: string;
}

function parseLevel(line: string): string {
  try {
    const obj = JSON.parse(line) as { level?: string };
    return (obj.level ?? "info").toLowerCase();
  } catch {
    return "info";
  }
}

const lines = computed<LogLine[]>(() => {
  const rawLines = rawContent.value
    .split("\n")
    .filter((line) => line.trim().length > 0);
  const ordered = displayOrder.value === "desc" ? [...rawLines].reverse() : rawLines;
  return ordered.map((line) => {
    const level = parseLevel(line);
    let text: string;
    if (displayFormat.value === "text") {
      try {
        const obj = JSON.parse(line) as {
          ts?: string;
          level?: string;
          resource?: string;
          env_id?: string;
          result?: string;
          item_count?: number;
          error?: string;
          raw_sample?: string;
        };
        const parts: string[] = [];
        if (obj.ts) parts.push(`[${obj.ts}]`);
        if (obj.level) parts.push(`[${obj.level}]`);
        if (obj.resource) parts.push(obj.resource);
        if (obj.result) parts.push(obj.result);
        const extras: string[] = [];
        if (obj.env_id) extras.push(`env=${obj.env_id}`);
        if (obj.item_count != null) extras.push(`${obj.item_count} items`);
        if (obj.error) extras.push(`err=${obj.error}`);
        if (obj.raw_sample) extras.push(obj.raw_sample);
        if (extras.length) parts.push("(" + extras.join(" ") + ")");
        text = parts.join(" ");
      } catch {
        text = line;
      }
    } else {
      text = line;
    }
    return { level, text };
  });
});

async function refresh() {
  loading.value = true;
  error.value = null;
  try {
    const [raw, settings] = await Promise.all([logRead(), logGetDisplaySettings()]);
    rawContent.value = raw;
    displayOrder.value = settings.order;
    displayFormat.value = settings.format;
  } catch (e) {
    error.value = extractErrorMessage(e);
  } finally {
    loading.value = false;
  }
}

async function clear() {
  clearing.value = true;
  error.value = null;
  try {
    await logClear();
    rawContent.value = "";
  } catch (e) {
    error.value = extractErrorMessage(e);
  } finally {
    clearing.value = false;
  }
}

watch(logRefreshTrigger, refresh);
watch(
  () => (props.visible ?? true),
  (visible) => {
    if (visible) refresh();
  }
);
onMounted(refresh);
</script>

<template>
  <div class="log-view">
    <header class="toolbar">
      <h2 class="title">调试日志</h2>
      <div class="actions">
        <NButton
          :loading="loading"
          :disabled="clearing"
          @click="refresh"
        >{{ loading ? "刷新中…" : "刷新" }}</NButton>
        <NButton
          type="error"
          secondary
          :loading="clearing"
          :disabled="loading"
          @click="clear"
        >{{ clearing ? "清除中…" : "清除" }}</NButton>
      </div>
    </header>
    <NAlert v-if="error" type="error" :show-icon="true" class="err-box">{{ error }}</NAlert>
    <NScrollbar
      v-else
      class="log-scroll"
      trigger="hover"
    >
      <div class="log-content" role="log">
        <template v-if="lines.length">
          <div
            v-for="(line, i) in lines"
            :key="i"
            class="log-line"
            :class="'log-level-' + line.level"
          >{{ line.text }}</div>
        </template>
        <NEmpty v-else class="log-empty" description="暂无日志" />
      </div>
    </NScrollbar>
  </div>
</template>

<style scoped>
.log-view {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid #e2e8f0;
  background: #fff;
  flex-shrink: 0;
}
.title {
  margin: 0;
  font-size: 1rem;
  font-weight: 600;
  color: #1e293b;
}
.actions {
  display: flex;
  gap: 0.5rem;
}
.err-box {
  margin: 0.75rem 1rem 0;
}
.log-scroll {
  flex: 1;
  min-height: 0;
}
.log-content {
  margin: 0;
  padding: 1rem;
  font-family: ui-monospace, monospace;
  font-size: 0.8125rem;
  line-height: 1.5;
  background: #f8fafc;
  min-height: 120px;
}
.log-line {
  white-space: pre-wrap;
  word-break: break-all;
  padding: 0.1rem 0;
}
.log-level-error {
  color: #dc2626;
  font-weight: 500;
}
.log-level-warn {
  color: #d97706;
}
.log-level-info {
  color: #334155;
}
.log-level-debug {
  color: #64748b;
}
.log-level-off {
  color: #94a3b8;
}
.log-empty {
  padding: 2rem 0;
}
</style>
