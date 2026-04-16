<script setup lang="ts">
import { computed } from "vue";
import { CodeEditor } from "monaco-editor-vue3";
import { useYamlMonacoTheme } from "../stores/yamlTheme";
import type { ResourceSnapshotItem } from "../stores/resourceSnapshots";
import { formatDateTime } from "../utils/dateFormat";

const props = defineProps<{
  visible: boolean;
  snapshot: ResourceSnapshotItem | null;
}>();

const emit = defineEmits<{
  (e: "close"): void;
}>();

const { monacoTheme } = useYamlMonacoTheme();

const monacoOptions = {
  fontSize: 13,
  minimap: { enabled: false },
  automaticLayout: true,
  wordWrap: "on",
  lineNumbers: "on",
  scrollBeyondLastLine: false,
  readOnly: true,
};

const title = computed(() => props.snapshot?.title || "资源快照");
const summary = computed(() => props.snapshot?.summary || "");
const yaml = computed(() => props.snapshot?.yaml || "");


</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="snapshot-viewer-overlay" @click.self="emit('close')">
      <div class="snapshot-viewer" role="dialog" aria-modal="true" aria-labelledby="snapshot-viewer-title">
        <header class="snapshot-viewer-header">
          <div>
            <h3 id="snapshot-viewer-title" class="snapshot-viewer-title">{{ title }}</h3>
            <p class="snapshot-viewer-meta">
              <span>{{ summary }}</span>
              <span>{{ formatDateTime(snapshot?.created_at) }}</span>
            </p>
          </div>
          <button type="button" class="snapshot-viewer-close" aria-label="关闭" @click="emit('close')">×</button>
        </header>
        <div class="snapshot-viewer-body">
          <CodeEditor
            :value="yaml"
            language="yaml"
            :theme="monacoTheme"
            :options="monacoOptions"
            class="snapshot-viewer-editor"
          />
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.snapshot-viewer-overlay {
  position: fixed;
  inset: 0;
  background: rgba(15, 23, 42, 0.42);
  z-index: 1200;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.5rem;
}
.snapshot-viewer {
  width: min(92vw, 980px);
  height: min(86vh, 780px);
  background: #fff;
  border-radius: 16px;
  overflow: hidden;
  box-shadow: 0 24px 60px rgba(15, 23, 42, 0.28);
  display: flex;
  flex-direction: column;
}
.snapshot-viewer-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem 1.25rem;
  border-bottom: 1px solid #e2e8f0;
  background: linear-gradient(180deg, #f8fbff 0%, #ffffff 100%);
}
.snapshot-viewer-title {
  margin: 0;
  font-size: 1rem;
  font-weight: 700;
  color: #0f172a;
}
.snapshot-viewer-meta {
  margin: 0.35rem 0 0;
  display: flex;
  gap: 0.9rem;
  flex-wrap: wrap;
  font-size: 0.75rem;
  color: #64748b;
}
.snapshot-viewer-close {
  width: 2rem;
  height: 2rem;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: #64748b;
  font-size: 1.25rem;
  cursor: pointer;
}
.snapshot-viewer-close:hover {
  background: #f1f5f9;
  color: #334155;
}
.snapshot-viewer-body {
  flex: 1;
  min-height: 0;
}
.snapshot-viewer-editor {
  width: 100%;
  height: 100%;
}
</style>
