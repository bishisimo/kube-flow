<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from "vue";
import * as jsYaml from "js-yaml";
import { marked } from "marked";
import { CodeEditor } from "monaco-editor-vue3";
import ConfigMapEditor from "./ConfigMapEditor.vue";
import PodLogPanel from "./PodLogPanel.vue";
import ResourceTopologyPanel from "./ResourceTopologyPanel.vue";
import WorkloadLogPanel from "./WorkloadLogPanel.vue";
import SecretEditor from "./SecretEditor.vue";
import {
  kubeApplyResource,
  kubeDescribeResource,
  kubeGetResource,
} from "../api/kube";
import { useYamlTheme } from "../stores/yamlTheme";
import { useShellStore } from "../stores/shell";
import { useEnvStore } from "../stores/env";

const DRAWER_WIDTH_KEY = "kube-flow:drawer-width";
const DRAWER_MIN = 360;
const DRAWER_MAX = 1200;
const DRAWER_DEFAULT = 560;

export interface SelectedResource {
  kind: string;
  name: string;
  namespace: string | null;
}

const props = defineProps<{
  visible: boolean;
  envId: string | null;
  resource: SelectedResource | null;
  initialTab?: string | null;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "navigate", payload: {
    targetKind: string;
    namespace: string | null;
    labelSelector?: string | null;
    resourceName?: string | null;
  }): void;
}>();

type DetailTab = "yaml" | "describe" | "edit" | "editConfig" | "logs" | "topology";
const activeTab = ref<DetailTab>("yaml");

const rawYaml = ref("");
const editYaml = ref("");
const describeMarkdown = ref("");
const loading = ref(false);
const describeLoading = ref(false);
const error = ref<string | null>(null);
const describeError = ref<string | null>(null);
const editError = ref<string | null>(null);
const editSaving = ref(false);
const showManagedFields = ref(false);
const { themeId } = useYamlTheme();
const { pendingOpen, requestSwitchToShell } = useShellStore();
const { openedEnvs } = useEnvStore();

const SHELL_WORKLOAD_KINDS = new Set(["Pod", "Deployment", "StatefulSet", "DaemonSet"]);

function openPodShell() {
  const r = props.resource;
  if (!r || !SHELL_WORKLOAD_KINDS.has(r.kind) || !props.envId) return;
  const env = openedEnvs.value.find((e) => e.id === props.envId);
  if (!env) return;
  const ns = r.namespace ?? "default";
  if (r.kind === "Pod") {
    pendingOpen.value = {
      envId: props.envId,
      envName: env.display_name,
      namespace: ns,
      podName: r.name,
    };
  } else {
    pendingOpen.value = {
      envId: props.envId,
      envName: env.display_name,
      namespace: ns,
      workloadKind: r.kind,
      workloadName: r.name,
    };
  }
  requestSwitchToShell();
}


const MONACO_DARK_THEMES = new Set([
  "atom-one-dark",
  "monokai",
  "dracula",
  "nord",
  "github-dark",
  "vs2015",
  "tomorrow-night-bright",
  "shades-of-purple",
]);
const monacoTheme = computed(() =>
  MONACO_DARK_THEMES.has(themeId.value) ? "vs-dark" : "vs"
);
const monacoOptions = {
  fontSize: 13,
  minimap: { enabled: false },
  automaticLayout: true,
  wordWrap: "on",
  lineNumbers: "on",
  scrollBeyondLastLine: false,
};
const monacoReadOnlyOptions = {
  ...monacoOptions,
  readOnly: true,
};

function getInitialDrawerWidth(): number {
  try {
    const s = localStorage.getItem(DRAWER_WIDTH_KEY);
    if (s) {
      const n = parseInt(s, 10);
      if (!isNaN(n) && n >= DRAWER_MIN && n <= DRAWER_MAX) return n;
    }
  } catch {}
  return DRAWER_DEFAULT;
}
const drawerWidth = ref(getInitialDrawerWidth());

let resizeStartX = 0;
let resizeStartW = 0;

function onResizeStart(e: MouseEvent) {
  resizeStartX = e.clientX;
  resizeStartW = drawerWidth.value;
  document.addEventListener("mousemove", onResizeMove);
  document.addEventListener("mouseup", onResizeEnd);
  document.body.style.cursor = "col-resize";
  document.body.style.userSelect = "none";
}

function onResizeMove(e: MouseEvent) {
  const delta = resizeStartX - e.clientX;
  const w = Math.min(DRAWER_MAX, Math.max(DRAWER_MIN, resizeStartW + delta));
  drawerWidth.value = w;
  try {
    localStorage.setItem(DRAWER_WIDTH_KEY, String(w));
  } catch {}
}

function onResizeEnd() {
  document.removeEventListener("mousemove", onResizeMove);
  document.removeEventListener("mouseup", onResizeEnd);
  document.body.style.cursor = "";
  document.body.style.userSelect = "";
}

onUnmounted(() => {
  document.removeEventListener("mousemove", onResizeMove);
  document.removeEventListener("mouseup", onResizeEnd);
});

function stripManagedFields(yamlStr: string): string {
  if (!yamlStr) return "";
  try {
    const obj = jsYaml.load(yamlStr) as Record<string, unknown>;
    if (!obj || typeof obj !== "object") return yamlStr;
    if (obj.metadata && typeof obj.metadata === "object") {
      const meta = obj.metadata as Record<string, unknown>;
      if ("managedFields" in meta) {
        const { managedFields: _, ...rest } = meta;
        obj.metadata = rest;
      }
    }
    return jsYaml.dump(obj, { lineWidth: -1 });
  } catch {
    return yamlStr;
  }
}

const displayYaml = computed(() => {
  if (!rawYaml.value) return "";
  try {
    const obj = jsYaml.load(rawYaml.value) as Record<string, unknown>;
    if (!obj || typeof obj !== "object") return rawYaml.value;
    if (!showManagedFields.value && obj.metadata && typeof obj.metadata === "object") {
      const meta = obj.metadata as Record<string, unknown>;
      if ("managedFields" in meta) {
        const { managedFields: _, ...rest } = meta;
        obj.metadata = rest;
      }
    }
    return jsYaml.dump(obj, { lineWidth: -1 });
  } catch {
    return rawYaml.value;
  }
});

const yamlContent = ref("");
watch(
  () => displayYaml.value,
  (v) => {
    yamlContent.value = v || "暂无内容";
  },
  { immediate: true }
);

async function fetchYaml() {
  if (!props.envId || !props.resource) return;
  loading.value = true;
  error.value = null;
  rawYaml.value = "";
  try {
    rawYaml.value = await kubeGetResource(
      props.envId,
      props.resource.kind,
      props.resource.name,
      props.resource.namespace
    );
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
}

async function fetchDescribe() {
  if (!props.envId || !props.resource) return;
  describeLoading.value = true;
  describeError.value = null;
  describeMarkdown.value = "";
  try {
    const res = await kubeDescribeResource(
      props.envId,
      props.resource.kind,
      props.resource.name,
      props.resource.namespace
    );
    describeMarkdown.value = res.markdown;
  } catch (e) {
    describeError.value = e instanceof Error ? e.message : String(e);
  } finally {
    describeLoading.value = false;
  }
}

async function applyEdit(yamlOverride?: string) {
  const yaml = yamlOverride ?? editYaml.value;
  if (!props.envId || !props.resource || !yaml.trim()) return;
  editSaving.value = true;
  editError.value = null;
  try {
    await kubeApplyResource(props.envId, yaml);
    await fetchYaml();
    activeTab.value = "yaml";
  } catch (e) {
    editError.value = e instanceof Error ? e.message : String(e);
  } finally {
    editSaving.value = false;
  }
}

watch(
  () => [props.visible, props.envId, props.resource?.kind, props.resource?.name, props.resource?.namespace, props.initialTab] as const,
  ([visible, envId, kind, name, _namespace, initialTab]) => {
    if (visible && envId && kind && name) {
      let nextTab: DetailTab = "yaml";
      if (initialTab === "editConfig" && (kind === "ConfigMap" || kind === "Secret")) {
        nextTab = "editConfig";
      } else if (
        initialTab === "logs" &&
        (kind === "Pod" ||
          kind === "Deployment" ||
          kind === "StatefulSet" ||
          kind === "DaemonSet")
      ) {
        nextTab = "logs";
      } else if (initialTab === "topology") {
        nextTab = "topology";
      }

      activeTab.value = nextTab;
      fetchYaml();
    } else {
      rawYaml.value = "";
      yamlContent.value = "";
      editYaml.value = "";
      describeMarkdown.value = "";
      error.value = null;
      describeError.value = null;
      editError.value = null;
    }
  },
  { immediate: true }
);

watch(
  () => [activeTab.value, rawYaml.value] as const,
  ([tab, yaml]) => {
    if (tab === "describe" && props.resource && !describeMarkdown.value && !describeLoading.value) {
      fetchDescribe();
    }
    if ((tab === "edit" || tab === "editConfig") && yaml) {
      editYaml.value = stripManagedFields(yaml);
      editError.value = null;
    }
  }
);

</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="drawer-overlay" @click.self="emit('close')">
      <div
        class="resize-handle"
        aria-label="拖拽调整宽度"
        @mousedown.prevent="onResizeStart"
      />
      <aside class="drawer" role="dialog" aria-labelledby="drawer-title" :style="{ width: drawerWidth + 'px' }">
        <header class="drawer-header">
          <h2 id="drawer-title" class="drawer-title">
            {{ resource ? `${resource.kind} / ${resource.name}` : "资源详情" }}
          </h2>
          <button type="button" class="btn-close" aria-label="关闭" @click="emit('close')">×</button>
        </header>
        <div v-if="props.resource" class="drawer-toolbar">
          <div class="toolbar-row">
            <div class="tab-buttons">
              <button
                type="button"
                class="tab-btn"
                :class="{ active: activeTab === 'yaml' }"
                @click="activeTab = 'yaml'"
              >
                YAML
              </button>
              <button
                type="button"
                class="tab-btn"
                :class="{ active: activeTab === 'edit' }"
                @click="activeTab = 'edit'"
              >
                Edit
              </button>
              <button
                type="button"
                class="tab-btn"
                :class="{ active: activeTab === 'describe' }"
                @click="activeTab = 'describe'"
              >
                Describe
              </button>
              <button
                v-if="
                  resource &&
                  (resource.kind === 'Pod' ||
                    resource.kind === 'Deployment' ||
                    resource.kind === 'StatefulSet' ||
                    resource.kind === 'DaemonSet')
                "
                type="button"
                class="tab-btn"
                :class="{ active: activeTab === 'logs' }"
                @click="activeTab = 'logs'"
              >
                日志
              </button>
              <button
                v-if="resource && SHELL_WORKLOAD_KINDS.has(resource.kind)"
                type="button"
                class="tab-btn tab-btn-shell"
                title="在 Pod Shell 界面打开"
                @click="openPodShell"
              >
                打开 Shell
              </button>
              <button
                v-if="resource && (resource.kind === 'ConfigMap' || resource.kind === 'Secret')"
                type="button"
                class="tab-btn"
                :class="{ active: activeTab === 'editConfig' }"
                @click="activeTab = 'editConfig'"
              >
                修改配置
              </button>
              <button
                type="button"
                class="tab-btn"
                :class="{ active: activeTab === 'topology' }"
                @click="activeTab = 'topology'"
              >
                关联资源
              </button>
            </div>
            <template v-if="activeTab === 'yaml' && rawYaml">
              <label class="checkbox-label">
                <input v-model="showManagedFields" type="checkbox" />
                <span>显示 managedFields</span>
              </label>
            </template>
            <template v-else-if="activeTab === 'edit' && rawYaml">
              <button
                type="button"
                class="btn-primary toolbar-apply"
                :disabled="editSaving"
                @click="applyEdit()"
              >
                {{ editSaving ? "保存中…" : "应用" }}
              </button>
            </template>
          </div>
        </div>
        <div class="drawer-body">
          <div v-if="loading && (activeTab === 'yaml' || activeTab === 'edit' || activeTab === 'editConfig')" class="loading-state">加载中…</div>
          <div v-else-if="describeLoading && activeTab === 'describe'" class="loading-state">加载中…</div>
          <div v-else-if="error && (activeTab === 'yaml' || activeTab === 'edit' || activeTab === 'editConfig')" class="error-state">{{ error }}</div>
          <div v-else-if="describeError && activeTab === 'describe'" class="error-state">{{ describeError }}</div>
          <div v-else-if="activeTab === 'describe'" class="describe-panel">
            <div v-if="describeMarkdown" class="describe-scroll describe-markdown" v-html="marked.parse(describeMarkdown)"></div>
            <p v-else class="describe-empty">暂无内容</p>
          </div>
          <div v-else-if="activeTab === 'topology'" class="topology-panel-wrap">
            <ResourceTopologyPanel
              :env-id="props.envId"
              :resource="resource"
              @navigate="(p) => emit('navigate', p)"
            />
          </div>
          <div
            v-else-if="
              activeTab === 'logs' &&
              resource &&
              (resource.kind === 'Pod' ||
                resource.kind === 'Deployment' ||
                resource.kind === 'StatefulSet' ||
                resource.kind === 'DaemonSet')
            "
            class="logs-panel"
          >
            <PodLogPanel
              v-if="resource.kind === 'Pod'"
              :env-id="props.envId"
              :namespace="resource.namespace ?? 'default'"
              :pod-name="resource.name"
            />
            <WorkloadLogPanel
              v-else
              :env-id="props.envId"
              :namespace="resource.namespace ?? 'default'"
              :workload-kind="resource.kind"
              :workload-name="resource.name"
            />
          </div>
          <div v-else-if="activeTab === 'editConfig' && rawYaml && (resource?.kind === 'ConfigMap' || resource?.kind === 'Secret')" class="edit-panel">
            <div v-if="editError" class="edit-error">{{ editError }}</div>
            <div v-if="resource?.kind === 'ConfigMap'" class="edit-scroll">
              <ConfigMapEditor
                :raw-yaml="editYaml"
                :saving="editSaving"
                @save="(y) => applyEdit(y)"
                @error="(m) => (editError = m)"
              />
            </div>
            <div v-else-if="resource?.kind === 'Secret'" class="edit-scroll">
              <SecretEditor
                :raw-yaml="editYaml"
                :saving="editSaving"
                @save="(y) => applyEdit(y)"
                @error="(m) => (editError = m)"
              />
            </div>
          </div>
          <div v-else-if="activeTab === 'edit' && rawYaml" class="edit-panel">
            <div v-if="editError" class="edit-error">{{ editError }}</div>
            <div class="edit-scroll">
              <CodeEditor
                v-model:value="editYaml"
                language="yaml"
                :theme="monacoTheme"
                :options="monacoOptions"
                class="edit-monaco"
              />
            </div>
          </div>
          <div v-else-if="rawYaml" class="yaml-scroll">
            <CodeEditor
              v-model:value="yamlContent"
              language="yaml"
              :theme="monacoTheme"
              :options="monacoReadOnlyOptions"
              class="yaml-monaco"
            />
          </div>
          <div v-else class="loading-state">加载中…</div>
        </div>
      </aside>
    </div>
  </Teleport>
</template>

<style scoped>
.drawer-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.3);
  z-index: 1000;
  display: flex;
  justify-content: flex-end;
  align-items: stretch;
}
.resize-handle {
  width: 6px;
  flex-shrink: 0;
  cursor: col-resize;
  background: transparent;
}
.resize-handle:hover {
  background: rgba(37, 99, 235, 0.15);
}
.drawer {
  height: 100%;
  max-width: 90vw;
  background: #fff;
  box-shadow: -4px 0 20px rgba(0, 0, 0, 0.1);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  flex-shrink: 0;
}
.drawer-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid #e2e8f0;
  flex-shrink: 0;
}
.drawer-title {
  margin: 0;
  font-size: 0.9375rem;
  font-weight: 600;
  color: #1e293b;
}
.btn-close {
  width: 2rem;
  height: 2rem;
  border: none;
  background: transparent;
  font-size: 1.5rem;
  line-height: 1;
  color: #64748b;
  cursor: pointer;
  border-radius: 4px;
}
.btn-close:hover {
  background: #f1f5f9;
  color: #334155;
}
.drawer-toolbar {
  padding: 0.5rem 1rem;
  border-bottom: 1px solid #e2e8f0;
  background: #f8fafc;
  flex-shrink: 0;
}
.toolbar-row {
  display: flex;
  align-items: center;
  gap: 1rem;
  flex-wrap: wrap;
  flex: 1;
  min-width: 0;
}
.toolbar-apply {
  margin-left: auto;
}
.tab-buttons {
  display: flex;
  gap: 0.25rem;
}
.tab-btn {
  padding: 0.25rem 0.6rem;
  border: 1px solid #e2e8f0;
  border-radius: 4px;
  background: #fff;
  font-size: 0.8125rem;
  cursor: pointer;
  color: #475569;
}
.tab-btn:hover {
  background: #f8fafc;
}
.tab-btn.active {
  background: rgba(37, 99, 235, 0.1);
  border-color: #2563eb;
  color: #2563eb;
  font-weight: 500;
}
.checkbox-label {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.8125rem;
  color: #475569;
  cursor: pointer;
}
.checkbox-label input {
  cursor: pointer;
}
.drawer-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 0;
}
.loading-state,
.error-state {
  padding: 1.5rem;
  text-align: center;
  font-size: 0.875rem;
}
.error-state {
  color: #dc2626;
}
.describe-panel {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.describe-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 1rem;
}
.describe-markdown {
  font-size: 0.8125rem;
  line-height: 1.6;
  color: #334155;
}
.describe-markdown :deep(h2) {
  margin: 1rem 0 0.5rem;
  font-size: 0.9375rem;
  font-weight: 600;
  color: #1e293b;
}
.describe-markdown :deep(h2:first-child) {
  margin-top: 0;
}
.describe-markdown :deep(table) {
  width: 100%;
  border-collapse: collapse;
  margin: 0.5rem 0 1rem;
  font-size: 0.8125rem;
}
.describe-markdown :deep(th),
.describe-markdown :deep(td) {
  padding: 0.4rem 0.6rem;
  text-align: left;
  border: 1px solid #e2e8f0;
}
.describe-markdown :deep(th) {
  background: #f8fafc;
  font-weight: 600;
  color: #475569;
}
.describe-markdown :deep(tr:hover td) {
  background: #f8fafc;
}
.describe-markdown :deep(pre) {
  margin: 0.5rem 0 1rem;
  padding: 0.75rem;
  font-family: ui-monospace, "SF Mono", Monaco, Consolas, monospace;
  font-size: 0.75rem;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 4px;
  overflow-x: auto;
}
.describe-markdown :deep(code) {
  font-family: ui-monospace, "SF Mono", Monaco, Consolas, monospace;
  font-size: 0.875em;
}
.describe-markdown :deep(p) {
  margin: 0.25rem 0;
}
.describe-empty {
  margin: 0;
  padding: 1rem;
  font-size: 0.8125rem;
  color: #64748b;
}
.topology-panel-wrap {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.logs-panel {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.edit-panel {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.edit-error {
  flex-shrink: 0;
  padding: 0.5rem 1rem;
  font-size: 0.8125rem;
  color: #dc2626;
  background: #fef2f2;
}
.edit-scroll {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.edit-monaco {
  flex: 1;
  min-height: 200px;
}
.edit-textarea:focus {
  outline: none;
  border-color: #2563eb;
}
.btn-primary {
  padding: 0.35rem 0.75rem;
  border: none;
  border-radius: 6px;
  background: #2563eb;
  color: #fff;
  font-size: 0.8125rem;
  cursor: pointer;
}
.btn-primary:hover:not(:disabled) {
  background: #1d4ed8;
}
.btn-primary:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.yaml-scroll {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.yaml-monaco {
  flex: 1;
  min-height: 200px;
}
</style>
