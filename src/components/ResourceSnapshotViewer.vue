<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NButton, NEmpty, NModal, NScrollbar, NSpin, NTag } from "naive-ui";
import { CodeEditor } from "monaco-editor-vue3";
import { useYamlMonacoTheme } from "../stores/yamlTheme";
import type { ResourceSnapshotItem } from "../stores/resourceSnapshots";
import { formatDateTime } from "../utils/dateFormat";
import { kubeGetResource } from "../api/kube";
import { buildDiffRows, normalizeYamlForDiff, formatCodeCell, type DiffRow } from "../features/orchestrator/yamlDiff";
import { stripManagedFields } from "../utils/yaml";

const props = defineProps<{
  visible: boolean;
  snapshot: ResourceSnapshotItem | null;
  envId?: string | null;
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

type ViewMode = "yaml" | "image-diff" | "env-diff";
const viewMode = ref<ViewMode>("yaml");

// 镜像双版本视图
const hasDualView = computed(() => Boolean(props.snapshot?.afterYaml));

// 环境 diff 状态
const envDiffLoading = ref(false);
const envDiffError = ref<string | null>(null);
const envDiffRows = ref<DiffRow[]>([]);
const envDiffLiveYaml = ref("");
const canEnvDiff = computed(() =>
  Boolean(props.envId && props.snapshot?.env_id && props.snapshot?.resource_kind && props.snapshot?.resource_name)
);

const title = computed(() => props.snapshot?.title || "资源快照");
const summary = computed(() => props.snapshot?.summary || "");
const yaml = computed(() => props.snapshot?.yaml || "");
const afterYaml = computed(() => props.snapshot?.afterYaml || "");

async function loadEnvDiff() {
  if (!props.envId || !props.snapshot) return;
  envDiffLoading.value = true;
  envDiffError.value = null;
  envDiffRows.value = [];
  try {
    const live = await kubeGetResource(
      props.envId,
      props.snapshot.resource_kind,
      props.snapshot.resource_name,
      props.snapshot.resource_namespace ?? null
    );
    envDiffLiveYaml.value = live;
    const snapshotNorm = normalizeYamlForDiff(stripManagedFields(props.snapshot.yaml));
    const liveNorm = normalizeYamlForDiff(stripManagedFields(live));
    envDiffRows.value = buildDiffRows(snapshotNorm, liveNorm);
  } catch (e) {
    envDiffError.value = e instanceof Error ? e.message : String(e);
  } finally {
    envDiffLoading.value = false;
  }
}

function switchMode(mode: ViewMode) {
  viewMode.value = mode;
  if (mode === "env-diff" && !envDiffRows.value.length && !envDiffLoading.value) {
    void loadEnvDiff();
  }
}

const diffHasChanges = computed(() =>
  envDiffRows.value.some((r) => r.type !== "context")
);
const diffStats = computed(() => {
  const added = envDiffRows.value.filter((r) => r.type === "added" || r.type === "modified").length;
  const removed = envDiffRows.value.filter((r) => r.type === "removed" || r.type === "modified").length;
  return { added, removed };
});

watch(
  () => [props.visible, props.snapshot?.id] as const,
  ([visible]) => {
    if (!visible) {
      viewMode.value = "yaml";
      envDiffRows.value = [];
      envDiffError.value = null;
      envDiffLiveYaml.value = "";
    }
  }
);
</script>

<template>
  <NModal
    :show="visible"
    :mask-closable="true"
    :auto-focus="false"
    :trap-focus="false"
    class="o-snapshot-n-modal"
    :zIndex="2100"
    @mask-click="emit('close')"
    @esc="emit('close')"
  >
    <div
      class="snapshot-viewer"
      :class="{ 'snapshot-viewer-wide': viewMode === 'image-diff' || viewMode === 'env-diff' }"
      role="dialog"
      aria-modal="true"
      aria-labelledby="snapshot-viewer-title"
    >
        <header class="snapshot-viewer-header">
          <div class="snapshot-viewer-header-info">
            <h3 id="snapshot-viewer-title" class="snapshot-viewer-title">{{ title }}</h3>
            <p class="snapshot-viewer-meta">
              <span>{{ summary }}</span>
              <span>{{ formatDateTime(snapshot?.created_at) }}</span>
            </p>
          </div>
          <div class="snapshot-viewer-header-actions">
            <div class="snapshot-viewer-tabs">
              <NButton
                size="tiny"
                :type="viewMode === 'yaml' ? 'primary' : 'default'"
                :secondary="viewMode !== 'yaml'"
                class="snapshot-tab-btn"
                @click="switchMode('yaml')"
              >YAML</NButton>
              <NButton
                v-if="hasDualView"
                size="tiny"
                :type="viewMode === 'image-diff' ? 'primary' : 'default'"
                :secondary="viewMode !== 'image-diff'"
                class="snapshot-tab-btn"
                @click="switchMode('image-diff')"
              >镜像对比</NButton>
              <NButton
                v-if="canEnvDiff"
                size="tiny"
                :type="viewMode === 'env-diff' ? 'primary' : 'default'"
                :secondary="viewMode !== 'env-diff'"
                class="snapshot-tab-btn"
                @click="switchMode('env-diff')"
              >与环境对比</NButton>
            </div>
            <NButton quaternary class="snapshot-viewer-close" aria-label="关闭" @click="emit('close')">×</NButton>
          </div>
        </header>

        <!-- 镜像前后对比 -->
        <div v-if="viewMode === 'image-diff'" class="snapshot-viewer-dual">
          <div class="snapshot-viewer-pane">
            <div class="snapshot-viewer-pane-label snapshot-viewer-pane-label-before">变更前</div>
            <CodeEditor :value="yaml" language="yaml" :theme="monacoTheme" :options="monacoOptions" class="snapshot-viewer-editor" />
          </div>
          <div class="snapshot-viewer-divider" />
          <div class="snapshot-viewer-pane">
            <div class="snapshot-viewer-pane-label snapshot-viewer-pane-label-after">变更后</div>
            <CodeEditor :value="afterYaml" language="yaml" :theme="monacoTheme" :options="monacoOptions" class="snapshot-viewer-editor" />
          </div>
        </div>

        <!-- 与环境 diff -->
        <div v-else-if="viewMode === 'env-diff'" class="snapshot-viewer-diff-wrap">
          <div v-if="envDiffLoading" class="snapshot-diff-status snapshot-diff-loading">
            <NSpin size="small" />
            <span>正在获取环境中的当前资源…</span>
          </div>
          <div v-else-if="envDiffError" class="snapshot-diff-status snapshot-diff-error">{{ envDiffError }}</div>
          <NEmpty v-else-if="!envDiffRows.length" class="snapshot-diff-empty" description="暂无对比数据" />
          <template v-else>
            <div class="snapshot-diff-legend">
              <span class="snapshot-diff-legend-left">快照</span>
              <template v-if="diffHasChanges">
                <NTag size="small" :bordered="false" type="error" class="diff-legend-tag">−{{ diffStats.removed }}</NTag>
                <NTag size="small" :bordered="false" type="success" class="diff-legend-tag">+{{ diffStats.added }}</NTag>
              </template>
              <NTag v-else size="small" :bordered="false" class="diff-legend-tag">与当前环境一致</NTag>
              <span class="snapshot-diff-legend-right">当前环境</span>
              <NButton size="tiny" secondary class="snapshot-diff-refresh" @click="loadEnvDiff">刷新</NButton>
            </div>
            <NScrollbar class="snapshot-diff-table-scroll" trigger="hover">
            <div class="snapshot-diff-table-wrap">
              <table class="snapshot-diff-table">
                <tbody>
                  <tr
                    v-for="(row, idx) in envDiffRows"
                    :key="idx"
                    :class="`diff-row-${row.type}`"
                  >
                    <td class="diff-lineno">{{ row.leftLineNo ?? '' }}</td>
                    <td class="diff-code diff-left" v-html="formatCodeCell(row, 'left')" />
                    <td class="diff-lineno">{{ row.rightLineNo ?? '' }}</td>
                    <td class="diff-code diff-right" v-html="formatCodeCell(row, 'right')" />
                  </tr>
                </tbody>
              </table>
            </div>
            </NScrollbar>
          </template>
        </div>

        <!-- 默认 YAML 视图 -->
        <div v-else class="snapshot-viewer-body">
          <CodeEditor :value="yaml" language="yaml" :theme="monacoTheme" :options="monacoOptions" class="snapshot-viewer-editor" />
        </div>
    </div>
  </NModal>
</template>

<style scoped>
:deep(.o-snapshot-n-modal .n-dialog) {
  max-width: min(96vw, 1440px);
  width: 100% !important;
  padding: 0;
  margin: 0 auto;
  background: transparent;
  box-shadow: none;
}
:deep(.o-snapshot-n-modal .n-dialog__content) {
  padding: 0;
  margin: 0;
  border: none;
  background: transparent;
  box-shadow: none;
  width: 100% !important;
  max-width: 100% !important;
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
  transition: width 0.2s ease;
}
.snapshot-viewer-wide {
  width: min(96vw, 1440px);
}
.snapshot-viewer-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.9rem 1.25rem;
  border-bottom: 1px solid #e2e8f0;
  background: linear-gradient(180deg, #f8fbff 0%, #ffffff 100%);
  flex-shrink: 0;
}
.snapshot-viewer-header-info {
  min-width: 0;
  flex: 1;
}
.snapshot-viewer-header-actions {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  flex-shrink: 0;
}
.snapshot-viewer-tabs {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.2rem;
  border: 1px solid #e2e8f0;
  border-radius: 999px;
  background: #f8fafc;
}
.snapshot-viewer-tabs :deep(.snapshot-tab-btn) {
  border-radius: 999px;
  font-size: 0.75rem;
  font-weight: 600;
  height: auto;
  min-height: unset;
  padding: 0.28rem 0.75rem;
}
.snapshot-diff-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.55rem;
  flex: 1;
  min-height: 6rem;
}
.snapshot-diff-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 8rem;
}
.snapshot-diff-table-scroll {
  flex: 1;
  min-height: 0;
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
  min-width: 2rem;
  height: 2rem;
  font-size: 1.25rem;
  padding: 0 !important;
}
.snapshot-viewer-body {
  flex: 1;
  min-height: 0;
}
.snapshot-viewer-editor {
  width: 100%;
  height: 100%;
}
/* 镜像对比双栏 */
.snapshot-viewer-dual {
  flex: 1;
  min-height: 0;
  display: flex;
  overflow: hidden;
}
.snapshot-viewer-pane {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.snapshot-viewer-pane-label {
  flex-shrink: 0;
  padding: 0.35rem 1rem;
  font-size: 0.75rem;
  font-weight: 700;
  letter-spacing: 0.02em;
}
.snapshot-viewer-pane-label-before {
  background: #fef3c7;
  color: #92400e;
  border-bottom: 1px solid #fde68a;
}
.snapshot-viewer-pane-label-after {
  background: #d1fae5;
  color: #065f46;
  border-bottom: 1px solid #a7f3d0;
}
.snapshot-viewer-divider {
  width: 1px;
  background: #e2e8f0;
  flex-shrink: 0;
}
/* 环境 diff */
.snapshot-viewer-diff-wrap {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.snapshot-diff-status {
  padding: 1.25rem 1.5rem;
  font-size: 0.875rem;
  color: #64748b;
}
.snapshot-diff-error {
  color: #dc2626;
  background: #fef2f2;
}
.snapshot-diff-legend {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 0.6rem;
  padding: 0.45rem 1rem;
  background: #f8fafc;
  border-bottom: 1px solid #e2e8f0;
  font-size: 0.75rem;
}
.snapshot-diff-legend-left,
.snapshot-diff-legend-right {
  font-weight: 700;
  color: #475569;
  flex: 1;
}
.snapshot-diff-legend-right {
  text-align: right;
}
.snapshot-diff-stat {
  padding: 0.1rem 0.45rem;
  border-radius: 999px;
  font-weight: 700;
  font-size: 0.7rem;
}
.snapshot-diff-stat.added {
  background: #dcfce7;
  color: #166534;
}
.snapshot-diff-stat.removed {
  background: #fee2e2;
  color: #991b1b;
}
.snapshot-diff-stat.same {
  background: #f1f5f9;
  color: #475569;
}
.snapshot-diff-legend .snapshot-diff-refresh {
  margin-left: auto;
  font-size: 0.7rem;
}
.diff-legend-tag {
  font-size: 0.7rem !important;
  font-weight: 700 !important;
}
.snapshot-diff-table-wrap {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.snapshot-diff-table {
  width: 100%;
  border-collapse: collapse;
  font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  font-size: 0.78rem;
  line-height: 1.5;
  table-layout: fixed;
}
.snapshot-diff-table colgroup col:nth-child(1),
.snapshot-diff-table colgroup col:nth-child(3) {
  width: 42px;
}
.diff-lineno {
  width: 42px;
  min-width: 42px;
  padding: 0 0.5rem;
  text-align: right;
  color: #94a3b8;
  background: #f8fafc;
  user-select: none;
  border-right: 1px solid #e2e8f0;
  vertical-align: top;
  white-space: nowrap;
}
.diff-code {
  padding: 0 0.65rem;
  white-space: pre-wrap;
  word-break: break-all;
  vertical-align: top;
}
.diff-left {
  border-right: 1px solid #e2e8f0;
}
.diff-row-context td {
  background: #fff;
  color: #334155;
}
.diff-row-added td.diff-right {
  background: #f0fdf4;
  color: #166534;
}
.diff-row-added td.diff-lineno:last-of-type {
  background: #dcfce7;
}
.diff-row-removed td.diff-left {
  background: #fff7f7;
  color: #991b1b;
}
.diff-row-removed td.diff-lineno:first-of-type {
  background: #fee2e2;
}
.diff-row-modified td.diff-left {
  background: #fffbeb;
  color: #92400e;
}
.diff-row-modified td.diff-right {
  background: #f0fdf4;
  color: #166534;
}
.diff-row-modified td.diff-lineno:first-of-type {
  background: #fef3c7;
}
.diff-row-modified td.diff-lineno:last-of-type {
  background: #dcfce7;
}
:deep(.inline-removed) {
  background: #fca5a5;
  border-radius: 2px;
  padding: 0 1px;
}
:deep(.inline-added) {
  background: #86efac;
  border-radius: 2px;
  padding: 0 1px;
}
</style>
