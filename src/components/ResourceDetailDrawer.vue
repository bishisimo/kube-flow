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
import ResourceSnapshotPanel from "./ResourceSnapshotPanel.vue";
import ResourceSnapshotViewer from "./ResourceSnapshotViewer.vue";
import {
  kubeApplyResource,
  kubeDescribeResource,
  kubeGetResource,
} from "../api/kube";
import { useYamlMonacoTheme } from "../stores/yamlTheme";
import {
  createResourceSnapshot,
  deleteResourceSnapshot,
  formatResourceSnapshotYaml,
  listResourceSnapshotsByCategory,
  summarizeResourceYaml,
  toggleResourceSnapshotPinned,
  type ResourceSnapshotItem,
} from "../stores/resourceSnapshots";
import { ensureAutoSnapshotSettingLoaded } from "../stores/appSettings";
import { useStrongholdAuthStore } from "../stores/strongholdAuth";

const DRAWER_WIDTH_KEY = "kube-flow:drawer-width";
const DRAWER_MIN = 360;
const DRAWER_MAX = 1200;
const DRAWER_DEFAULT = 560;

export interface SelectedResource {
  kind: string;
  name: string;
  namespace: string | null;
}

interface NodeTaintDraft {
  key: string;
  value: string;
  effect: "NoSchedule" | "PreferNoSchedule" | "NoExecute" | "";
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

type DetailTab = "yaml" | "describe" | "edit" | "editConfig" | "logs" | "topology" | "snapshots" | "taints";
const activeTab = ref<DetailTab>("yaml");

const rawYaml = ref("");
const editYaml = ref("");
const editConfigYaml = ref("");
const describeMarkdown = ref("");
const loading = ref(false);
const describeLoading = ref(false);
const error = ref<string | null>(null);
const describeError = ref<string | null>(null);
const editError = ref<string | null>(null);
const editInfo = ref<string | null>(null);
const editSaving = ref(false);
const snapshotSaving = ref(false);
const viewingSnapshot = ref<ResourceSnapshotItem | null>(null);
const showManagedFields = ref(false);
const { monacoTheme } = useYamlMonacoTheme();
const strongholdAuth = useStrongholdAuthStore();
const nodeTaints = ref<NodeTaintDraft[]>([]);


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

const snapshotResourceRef = computed(() =>
  props.envId && props.resource
    ? {
        env_id: props.envId,
        resource_kind: props.resource.kind,
        resource_name: props.resource.name,
        resource_namespace: props.resource.namespace ?? null,
      }
    : null
);

const genericSnapshots = computed(() => listResourceSnapshotsByCategory(snapshotResourceRef.value, "all"));
const currentSnapshotSummary = computed(() => summarizeResourceYaml(resolveCurrentDraftYaml() || rawYaml.value));
const isNodeResource = computed(() => props.resource?.kind === "Node");

const taintsValidationError = computed(() => {
  const seen = new Set<string>();
  for (const taint of nodeTaints.value) {
    const key = taint.key.trim();
    const effect = taint.effect.trim();
    if (!key) return "污点 key 不能为空。";
    if (!effect) return "请选择污点 effect。";
    const dedupKey = `${key}|${effect}`;
    if (seen.has(dedupKey)) return `存在重复污点：${key} / ${effect}`;
    seen.add(dedupKey);
  }
  return null;
});

function resolveCurrentDraftYaml(): string {
  if (activeTab.value === "taints" && isNodeResource.value && rawYaml.value) {
    return buildNodeTaintsYaml();
  }
  if (props.resource?.kind === "ConfigMap" || props.resource?.kind === "Secret") {
    return editConfigYaml.value || editYaml.value || rawYaml.value;
  }
  return editYaml.value || rawYaml.value;
}

function parseNodeTaintsFromYaml(yamlStr: string): NodeTaintDraft[] {
  if (!yamlStr) return [];
  try {
    const parsed = jsYaml.load(yamlStr) as Record<string, unknown> | null;
    const spec =
      parsed?.spec && typeof parsed.spec === "object" ? (parsed.spec as Record<string, unknown>) : null;
    const taints = Array.isArray(spec?.taints) ? (spec?.taints as Array<Record<string, unknown>>) : [];
    return taints.map((item) => ({
      key: typeof item.key === "string" ? item.key : "",
      value: typeof item.value === "string" ? item.value : "",
      effect:
        item.effect === "NoSchedule" || item.effect === "PreferNoSchedule" || item.effect === "NoExecute"
          ? item.effect
          : "",
    }));
  } catch {
    return [];
  }
}

function buildNodeTaintsYaml(): string {
  if (!rawYaml.value) return "";
  try {
    const parsed = jsYaml.load(rawYaml.value) as Record<string, unknown> | null;
    if (!parsed || typeof parsed !== "object") return rawYaml.value;
    const spec =
      parsed.spec && typeof parsed.spec === "object" ? (parsed.spec as Record<string, unknown>) : {};
    const nextTaints = nodeTaints.value.map((item) => {
      const taint: Record<string, string> = {
        key: item.key.trim(),
        effect: item.effect,
      };
      if (item.value.trim()) taint.value = item.value.trim();
      return taint;
    });
    spec.taints = nextTaints;
    parsed.spec = spec;
    return jsYaml.dump(parsed, { lineWidth: -1 });
  } catch {
    return rawYaml.value;
  }
}

function resetNodeTaintsDraft() {
  nodeTaints.value = parseNodeTaintsFromYaml(rawYaml.value);
}

function addNodeTaint() {
  nodeTaints.value = [
    ...nodeTaints.value,
    { key: "", value: "", effect: "NoSchedule" },
  ];
  editError.value = null;
  editInfo.value = null;
}

function removeNodeTaint(index: number) {
  nodeTaints.value = nodeTaints.value.filter((_, idx) => idx !== index);
  editError.value = null;
  editInfo.value = null;
}

function taintEffectLabel(effect: string): string {
  switch (effect) {
    case "NoSchedule":
      return "禁止调度";
    case "PreferNoSchedule":
      return "尽量不调度";
    case "NoExecute":
      return "禁止运行";
    default:
      return "未设置";
  }
}

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

async function handleStrongholdLocked(message: string, onConfirmed: () => void): Promise<boolean> {
  return strongholdAuth.checkAndHandle(message, onConfirmed, {
    title: "解锁资源凭证",
    description: "当前资源操作需要访问已保存凭证，请先输入 Stronghold 主密码解锁。",
  });
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
    const msg = e instanceof Error ? e.message : String(e);
    const isStrongholdRequired = await handleStrongholdLocked(msg, () => {
      void fetchYaml();
    });
    if (isStrongholdRequired) return;
    error.value = msg;
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
    const msg = e instanceof Error ? e.message : String(e);
    const isStrongholdRequired = await handleStrongholdLocked(msg, () => {
      void fetchDescribe();
    });
    if (isStrongholdRequired) return;
    describeError.value = msg;
  } finally {
    describeLoading.value = false;
  }
}

async function applyEdit(yamlOverride?: string) {
  const yaml = yamlOverride ?? editYaml.value;
  if (!props.envId || !props.resource || !yaml.trim()) return;
  editSaving.value = true;
  editError.value = null;
  editInfo.value = null;
  try {
    const snapshotYaml =
      activeTab.value === "editConfig"
        ? (editConfigYaml.value || yaml).trim()
        : yaml.trim();
    const autoSnapshotEnabled = await ensureAutoSnapshotSettingLoaded();
    if (autoSnapshotEnabled && snapshotYaml && snapshotResourceRef.value) {
      createResourceSnapshot(snapshotResourceRef.value, {
        yaml: snapshotYaml,
        category: activeTab.value === "editConfig" ? "config" : "resource",
        source: "before-apply",
        title: activeTab.value === "editConfig" ? "应用前配置快照" : "应用前资源快照",
      });
    }
    await kubeApplyResource(props.envId, yaml);
    await fetchYaml();
    editInfo.value = "已自动保存当前编辑草稿快照，可在“快照”栏目统一查看。";
    activeTab.value = "yaml";
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    const isStrongholdRequired = await handleStrongholdLocked(msg, () => {
      void applyEdit(yaml);
    });
    if (isStrongholdRequired) return;
    editError.value = msg;
  } finally {
    editSaving.value = false;
  }
}

async function applyNodeTaints() {
  if (!isNodeResource.value || !rawYaml.value) return;
  if (taintsValidationError.value) {
    editError.value = taintsValidationError.value;
    editInfo.value = null;
    return;
  }
  await applyEdit(buildNodeTaintsYaml());
  resetNodeTaintsDraft();
}

function openSnapshotViewer(snapshot: ResourceSnapshotItem) {
  viewingSnapshot.value = snapshot;
}

function removeSnapshot(snapshot: ResourceSnapshotItem) {
  deleteResourceSnapshot(snapshot.id);
  if (viewingSnapshot.value?.id === snapshot.id) {
    viewingSnapshot.value = null;
  }
  editInfo.value = "快照已删除。";
}

function togglePinSnapshot(snapshot: ResourceSnapshotItem) {
  const next = toggleResourceSnapshotPinned(snapshot.id);
  if (!next) return;
  if (viewingSnapshot.value?.id === snapshot.id) {
    viewingSnapshot.value = next;
  }
  editInfo.value = next.pinned
    ? "快照已置顶，不会参与自动淘汰。"
    : "已取消置顶，该快照会重新参与自动快照淘汰规则。";
}

function handleEditorError(message: string) {
  editError.value = message;
  editInfo.value = null;
}

function handleConfigYamlUpdate(yaml: string) {
  editConfigYaml.value = yaml;
}

function saveManualSnapshot() {
  if (!snapshotResourceRef.value) return;
  const snapshotYaml = formatResourceSnapshotYaml(resolveCurrentDraftYaml().trim());
  if (!snapshotYaml) return;
  const category = activeTab.value === "editConfig" ? "config" : "resource";
  snapshotSaving.value = true;
  try {
    createResourceSnapshot(snapshotResourceRef.value, {
      yaml: snapshotYaml,
      category,
      source: "manual",
      title: category === "config" ? "手动配置快照" : "手动资源快照",
    });
    editError.value = null;
    editInfo.value = "当前资源已保存为快照。";
  } finally {
    snapshotSaving.value = false;
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
      } else if (initialTab === "taints" && kind === "Node") {
        nextTab = "taints";
      } else if (initialTab === "topology") {
        nextTab = "topology";
      }

      activeTab.value = nextTab;
      fetchYaml();
    } else {
      rawYaml.value = "";
      yamlContent.value = "";
      editYaml.value = "";
      editConfigYaml.value = "";
      describeMarkdown.value = "";
      error.value = null;
      describeError.value = null;
      editError.value = null;
      editInfo.value = null;
      nodeTaints.value = [];
      viewingSnapshot.value = null;
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
      editConfigYaml.value = stripManagedFields(yaml);
      editError.value = null;
      editInfo.value = null;
    }
    if (tab === "taints" && yaml && isNodeResource.value) {
      resetNodeTaintsDraft();
      editError.value = null;
      editInfo.value = null;
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
          <div class="drawer-header-actions">
            <label v-if="activeTab === 'yaml' && rawYaml" class="checkbox-label checkbox-label-inline drawer-header-toggle">
              <input v-model="showManagedFields" type="checkbox" />
              <span>managedFields</span>
            </label>
            <button type="button" class="btn-close" aria-label="关闭" @click="emit('close')">×</button>
          </div>
        </header>
        <div v-if="props.resource" class="drawer-toolbar">
          <div class="toolbar-head">
            <template v-if="(activeTab === 'edit' || activeTab === 'taints') && rawYaml">
              <button
                type="button"
                class="btn-primary toolbar-apply"
                :disabled="editSaving"
                @click="activeTab === 'taints' ? applyNodeTaints() : applyEdit()"
              >
                {{ editSaving ? "保存中…" : "应用" }}
              </button>
            </template>
          </div>
          <div class="toolbar-row">
            <div class="tab-grid">
              <button
                type="button"
                class="tab-btn tab-compact"
                :class="{ active: activeTab === 'yaml' }"
                @click="activeTab = 'yaml'"
              >
                <span class="tab-icon" aria-hidden="true">Y</span>
                <span class="tab-title">YAML</span>
              </button>
              <button
                type="button"
                class="tab-btn tab-compact"
                :class="{ active: activeTab === 'edit' }"
                @click="activeTab = 'edit'"
              >
                <span class="tab-icon" aria-hidden="true">E</span>
                <span class="tab-title">编辑</span>
              </button>
              <button
                type="button"
                class="tab-btn tab-compact"
                :class="{ active: activeTab === 'describe' }"
                @click="activeTab = 'describe'"
              >
                <span class="tab-icon" aria-hidden="true">D</span>
                <span class="tab-title">详情</span>
              </button>
              <button
                v-if="resource?.kind === 'Node'"
                type="button"
                class="tab-btn tab-compact"
                :class="{ active: activeTab === 'taints' }"
                @click="activeTab = 'taints'"
              >
                <span class="tab-icon" aria-hidden="true">T</span>
                <span class="tab-title">污点</span>
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
                class="tab-btn tab-compact"
                :class="{ active: activeTab === 'logs' }"
                @click="activeTab = 'logs'"
              >
                <span class="tab-icon" aria-hidden="true">L</span>
                <span class="tab-title">日志</span>
              </button>
              <button
                v-if="resource && (resource.kind === 'ConfigMap' || resource.kind === 'Secret')"
                type="button"
                class="tab-btn tab-compact"
                :class="{ active: activeTab === 'editConfig' }"
                @click="activeTab = 'editConfig'"
              >
                <span class="tab-icon" aria-hidden="true">C</span>
                <span class="tab-title">配置</span>
              </button>
              <button
                type="button"
                class="tab-btn tab-compact"
                :class="{ active: activeTab === 'topology' }"
                @click="activeTab = 'topology'"
              >
                <span class="tab-icon" aria-hidden="true">R</span>
                <span class="tab-title">关联</span>
              </button>
              <button
                type="button"
                class="tab-btn tab-compact"
                :class="{ active: activeTab === 'snapshots' }"
                @click="activeTab = 'snapshots'"
              >
                <span class="tab-icon" aria-hidden="true">S</span>
                <span class="tab-title">快照</span>
              </button>
            </div>
          </div>
        </div>
        <div class="drawer-body">
          <div v-if="loading && (activeTab === 'yaml' || activeTab === 'edit' || activeTab === 'editConfig' || activeTab === 'taints')" class="loading-state">加载中…</div>
          <div v-else-if="describeLoading && activeTab === 'describe'" class="loading-state">加载中…</div>
          <div v-else-if="error && (activeTab === 'yaml' || activeTab === 'edit' || activeTab === 'editConfig' || activeTab === 'taints')" class="error-state">{{ error }}</div>
          <div v-else-if="describeError && activeTab === 'describe'" class="error-state">{{ describeError }}</div>
          <div v-else-if="activeTab === 'describe'" class="describe-panel">
            <div v-if="describeMarkdown" class="describe-scroll describe-markdown" v-html="marked.parse(describeMarkdown)"></div>
            <p v-else class="describe-empty">暂无内容</p>
          </div>
          <div v-else-if="activeTab === 'taints' && resource?.kind === 'Node'" class="taints-panel">
            <div v-if="editError" class="edit-error">{{ editError }}</div>
            <div v-else-if="editInfo" class="edit-info">{{ editInfo }}</div>
            <div v-if="!nodeTaints.length" class="taints-empty">当前节点没有配置污点。</div>
            <div class="taints-list">
              <div v-for="(taint, index) in nodeTaints" :key="index" class="taint-card">
                <div class="taint-card-head">
                  <div class="taint-card-index">污点 {{ index + 1 }}</div>
                  <button type="button" class="btn-close taint-remove-btn" aria-label="删除污点" @click="removeNodeTaint(index)">×</button>
                </div>
                <label class="taint-field">
                  <span class="taint-field-label">Key</span>
                  <input v-model="taint.key" type="text" class="taint-input" placeholder="例如 dedicated" />
                </label>
                <label class="taint-field">
                  <span class="taint-field-label">Value</span>
                  <input v-model="taint.value" type="text" class="taint-input" placeholder="例如 infra（可选）" />
                </label>
                <label class="taint-field">
                  <span class="taint-field-label">Effect</span>
                  <div class="taint-effect-row">
                    <select v-model="taint.effect" class="taint-input taint-select">
                      <option value="" disabled>选择生效方式</option>
                      <option value="NoSchedule">NoSchedule</option>
                      <option value="PreferNoSchedule">PreferNoSchedule</option>
                      <option value="NoExecute">NoExecute</option>
                    </select>
                    <span class="taint-effect-pill" :class="`effect-${taint.effect || 'empty'}`">
                      {{ taintEffectLabel(taint.effect) }}
                    </span>
                  </div>
                </label>
              </div>
              <button type="button" class="taint-add-card" :disabled="editSaving" @click="addNodeTaint">
                <span class="taint-add-card-plus">+</span>
                <span class="taint-add-card-text">新增污点</span>
              </button>
            </div>
            <div v-if="taintsValidationError" class="edit-error taints-validation">{{ taintsValidationError }}</div>
          </div>
          <div v-else-if="activeTab === 'topology'" class="topology-panel-wrap">
            <ResourceTopologyPanel
              :env-id="props.envId"
              :resource="resource"
              @navigate="(p) => emit('navigate', p)"
            />
          </div>
          <div v-else-if="activeTab === 'snapshots'" class="snapshot-tab-wrap">
            <ResourceSnapshotPanel
              title="资源快照"
              subtitle="统一查看和管理当前资源的历史快照；普通 YAML 编辑会保存完整 YAML（不含 managedFields）。手动快照和置顶快照不会参与自动淘汰。"
              create-label="生成快照"
              :snapshots="genericSnapshots"
              :creating="snapshotSaving"
              :current-summary="currentSnapshotSummary"
              empty-text="还没有资源快照。编辑 YAML 或配置后，可以随时在这里保存和管理快照。"
              @create="saveManualSnapshot"
              @view="openSnapshotViewer"
              @delete="removeSnapshot"
              @toggle-pin="togglePinSnapshot"
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
            <div v-else-if="editInfo" class="edit-info">{{ editInfo }}</div>
            <div v-if="resource?.kind === 'ConfigMap'" class="edit-scroll">
              <ConfigMapEditor
                :raw-yaml="editYaml"
                :saving="editSaving"
                @save="(y) => applyEdit(y)"
                @error="handleEditorError"
                @update:yaml="handleConfigYamlUpdate"
              />
            </div>
            <div v-else-if="resource?.kind === 'Secret'" class="edit-scroll">
              <SecretEditor
                :raw-yaml="editYaml"
                :saving="editSaving"
                @save="(y) => applyEdit(y)"
                @error="handleEditorError"
                @update:yaml="handleConfigYamlUpdate"
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
  <ResourceSnapshotViewer
    :visible="!!viewingSnapshot"
    :snapshot="viewingSnapshot"
    @close="viewingSnapshot = null"
  />
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
  gap: 0.75rem;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid #e2e8f0;
  flex-shrink: 0;
}
.drawer-title {
  margin: 0;
  font-size: 0.9375rem;
  font-weight: 600;
  color: #1e293b;
  min-width: 0;
}
.drawer-header-actions {
  display: flex;
  align-items: center;
  gap: 0.55rem;
  flex-shrink: 0;
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
  padding: 0.75rem 1rem 0.9rem;
  border-bottom: 1px solid #e2e8f0;
  background:
    radial-gradient(circle at top left, rgba(59, 130, 246, 0.12), transparent 28%),
    linear-gradient(180deg, #f8fbff 0%, #f8fafc 100%);
  flex-shrink: 0;
}
.toolbar-head {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.75rem;
  margin-bottom: 0.75rem;
}
.toolbar-row {
  display: flex;
  align-items: stretch;
  gap: 0.75rem;
  min-width: 0;
}
.toolbar-apply {
  flex-shrink: 0;
}
.tab-grid {
  display: flex;
  align-items: center;
  gap: 0.55rem;
  min-width: 0;
  overflow-x: auto;
  overflow-y: hidden;
  padding-bottom: 0.1rem;
  scrollbar-width: thin;
}
.tab-btn {
  flex-shrink: 0;
  border: 1px solid #dbe3ee;
  border-radius: 999px;
  background: #fff;
  cursor: pointer;
  color: #475569;
  transition:
    border-color 0.16s ease,
    box-shadow 0.16s ease,
    background 0.16s ease;
}
.tab-btn:hover {
  background: #fff;
  border-color: #93c5fd;
  box-shadow: 0 6px 16px rgba(148, 163, 184, 0.12);
}
.tab-compact {
  min-height: 0;
  padding: 0.5rem 0.8rem;
  display: flex;
  align-items: center;
  gap: 0.45rem;
  white-space: nowrap;
}
.tab-btn.active {
  background: linear-gradient(180deg, rgba(219, 234, 254, 0.88) 0%, rgba(239, 246, 255, 1) 100%);
  border-color: #60a5fa;
  color: #1d4ed8;
  box-shadow: inset 0 0 0 1px rgba(96, 165, 250, 0.22), 0 8px 18px rgba(59, 130, 246, 0.12);
}
.tab-icon {
  width: 1.35rem;
  height: 1.35rem;
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  background: #eff6ff;
  color: #2563eb;
  font-size: 0.68rem;
  font-weight: 800;
}
.tab-btn.active .tab-icon {
  background: #2563eb;
  color: #fff;
}
.tab-title {
  font-size: 0.78rem;
  font-weight: 700;
  line-height: 1;
  color: #1e293b;
}
.tab-btn.active .tab-title {
  color: #1d4ed8;
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
.checkbox-label-ghost {
  padding: 0.55rem 0.8rem;
  border: 1px solid #dbe3ee;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.82);
}
.checkbox-label-inline {
  flex-shrink: 0;
  align-self: center;
  padding: 0.42rem 0.72rem;
  border: 1px solid #dbe3ee;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.88);
  white-space: nowrap;
}
.drawer-header-toggle {
  max-width: min(34vw, 180px);
  overflow: hidden;
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
.snapshot-tab-wrap {
  flex: 1;
  min-height: 0;
  padding: 1rem;
  overflow: hidden;
}
.snapshot-tab-wrap :deep(.snapshot-panel) {
  width: 100%;
  min-width: 0;
  max-width: none;
  height: 100%;
  border-left: none;
  border: 1px solid #e2e8f0;
  border-radius: 16px;
}
.logs-panel {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.taints-panel {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 1rem;
  display: grid;
  gap: 0.75rem;
  background: linear-gradient(180deg, #f8fafc 0%, #ffffff 100%);
}
.taints-empty {
  padding: 0.9rem 1rem;
  border: 1px dashed #cbd5e1;
  border-radius: 14px;
  background: #fff;
  font-size: 0.8rem;
  color: #64748b;
}
.taints-list {
  display: flex;
  flex-direction: column;
  gap: 0.8rem;
}
.taint-card {
  display: flex;
  flex-direction: column;
  gap: 0.8rem;
  padding: 1rem;
  border: 1px solid #e2e8f0;
  border-radius: 18px;
  background:
    radial-gradient(circle at top right, rgba(37, 99, 235, 0.08), transparent 28%),
    #fff;
  box-shadow: 0 12px 28px rgba(15, 23, 42, 0.06);
}
.taint-card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}
.taint-card-index {
  display: inline-flex;
  align-items: center;
  min-height: 1.45rem;
  padding: 0 0.5rem;
  border-radius: 999px;
  background: #eff6ff;
  color: #1d4ed8;
  font-size: 0.68rem;
  font-weight: 700;
}
.taint-effect-pill {
  display: inline-flex;
  align-items: center;
  min-height: 1.5rem;
  padding: 0 0.55rem;
  border-radius: 999px;
  font-size: 0.72rem;
  font-weight: 700;
}
.taint-effect-pill.effect-NoSchedule {
  background: #fee2e2;
  color: #b91c1c;
}
.taint-effect-pill.effect-PreferNoSchedule {
  background: #fef3c7;
  color: #b45309;
}
.taint-effect-pill.effect-NoExecute {
  background: #dbeafe;
  color: #1d4ed8;
}
.taint-effect-pill.effect-empty {
  background: #e2e8f0;
  color: #475569;
}
.taint-effect-code {
  font-size: 0.75rem;
  color: #64748b;
  font-family: ui-monospace, "SF Mono", Monaco, Consolas, monospace;
}
.taint-field {
  display: grid;
  gap: 0.35rem;
}
.taint-field-label {
  font-size: 0.74rem;
  font-weight: 700;
  color: #475569;
}
.taint-effect-row {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  flex-wrap: wrap;
}
.taint-remove-btn {
  width: 1.8rem;
  height: 1.8rem;
  font-size: 1.2rem;
  border-radius: 999px;
  color: #94a3b8;
}
.taint-remove-btn:hover {
  background: #fef2f2;
  color: #dc2626;
}
.taint-add-card {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 0.42rem;
  width: 100%;
  min-height: 48px;
  padding: 0.7rem 0.9rem;
  border: 1px dashed #93c5fd;
  border-radius: 14px;
  background: linear-gradient(180deg, #f8fbff, #eff6ff);
  color: #1d4ed8;
  cursor: pointer;
}
.taint-add-card:hover {
  border-color: #60a5fa;
  background: linear-gradient(180deg, #eff6ff, #dbeafe);
}
.taint-add-card:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.taint-add-card-plus {
  font-size: 0.95rem;
  line-height: 1;
  font-weight: 700;
}
.taint-add-card-text {
  font-size: 0.8rem;
  font-weight: 700;
}
.taint-input {
  width: 100%;
  min-width: 0;
  height: 2.2rem;
  border: 1px solid #dbe3ee;
  border-radius: 10px;
  padding: 0 0.7rem;
  font-size: 0.82rem;
  color: #0f172a;
  background: #f8fafc;
}
.taint-select {
  flex: 1 1 280px;
  max-width: 100%;
}
.taint-input:focus {
  outline: none;
  border-color: #2563eb;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
  background: #fff;
}
.taints-validation {
  margin: 0;
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
.edit-info {
  flex-shrink: 0;
  padding: 0.5rem 1rem;
  font-size: 0.8125rem;
  color: #0f766e;
  background: #ecfeff;
  border-bottom: 1px solid #cffafe;
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
  padding: 0.55rem 0.95rem;
  border: none;
  border-radius: 12px;
  background: #2563eb;
  color: #fff;
  font-size: 0.8125rem;
  font-weight: 600;
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

@media (max-width: 960px) {
  .toolbar-head {
    flex-direction: column;
    align-items: stretch;
  }
  .toolbar-row {
    flex-direction: column;
  }
  .tab-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 640px) {
  .tab-grid {
    grid-template-columns: minmax(0, 1fr);
  }
  .toolbar-resource-meta {
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>
