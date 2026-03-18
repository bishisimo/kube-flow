<script setup lang="ts">
import { computed, ref, watch } from "vue";
import * as jsYaml from "js-yaml";
import { CodeEditor } from "monaco-editor-vue3";
import { kubeApplyResource, kubeGetResource } from "../api/kube";
import { useEnvStore } from "../stores/env";
import { useYamlMonacoTheme } from "../stores/yamlTheme";
import { useOrchestratorStore, type OrchestratorManifest } from "../stores/orchestrator";

interface ParseIdentity {
  kind: string;
  name: string;
  namespace: string | null;
}

interface DiffRow {
  type: "context" | "added" | "removed" | "modified";
  leftLineNo: number | null;
  rightLineNo: number | null;
  leftText: string;
  rightText: string;
}

interface TokenOp {
  op: "=" | "-" | "+";
  text: string;
}

const APPLY_ORDER: Record<string, number> = {
  Namespace: 1,
  ConfigMap: 10,
  Secret: 11,
  ServiceAccount: 12,
  PersistentVolumeClaim: 13,
  Service: 20,
  Deployment: 30,
  StatefulSet: 31,
  DaemonSet: 32,
  Ingress: 40,
  HorizontalPodAutoscaler: 50,
};

const { environments, currentId } = useEnvStore();
const {
  manifests,
  createManifestDraft,
  saveManifestYaml,
  setManifestIdentity,
  setManifestComponent,
  deleteManifest,
  copyComponentToEnv,
} = useOrchestratorStore();
const { monacoTheme } = useYamlMonacoTheme();

const selectedEnvId = ref<string>("");
const selectedComponent = ref<string>("");
const selectedManifestId = ref<string>("");
const editYaml = ref("");
const validationErrors = ref<string[]>([]);
const validationWarnings = ref<string[]>([]);
const opMessage = ref<string | null>(null);
const opError = ref<string | null>(null);
const applying = ref(false);
const applyDialogVisible = ref(false);
const copyDialogVisible = ref(false);
const deleteArmedId = ref("");
const copyTargetEnvId = ref("");
const copyOverwrite = ref(true);
const copyLoading = ref(false);
const diffLoading = ref(false);
const diffRows = ref<DiffRow[]>([]);

const editorOptions = {
  fontSize: 13,
  minimap: { enabled: false },
  automaticLayout: true,
  wordWrap: "on",
  scrollBeyondLastLine: false,
};

const manifestsByEnv = computed(() =>
  manifests.value.filter((m) => m.env_id === selectedEnvId.value)
);

const components = computed(() => {
  const names = new Set<string>();
  for (const m of manifestsByEnv.value) names.add(m.component);
  return Array.from(names).sort((a, b) => a.localeCompare(b));
});

const manifestsByComponent = computed(() =>
  manifestsByEnv.value.filter((m) => m.component === selectedComponent.value)
);

const selectedManifest = computed<OrchestratorManifest | null>(
  () => manifests.value.find((m) => m.id === selectedManifestId.value) ?? null
);

const selectedHistory = computed(() => selectedManifest.value?.history ?? []);
const canApplyCurrent = computed(() => Boolean(selectedEnvId.value && selectedManifestId.value));
const canApplyComponent = computed(() => Boolean(selectedEnvId.value && selectedComponent.value));
const canOpenApplyDialog = computed(() => canApplyCurrent.value || canApplyComponent.value);
const canOpenCopyDialog = computed(() => Boolean(selectedEnvId.value && selectedComponent.value && environments.value.length > 1));
const diffStats = computed(() => {
  let added = 0;
  let removed = 0;
  let modified = 0;
  for (const r of diffRows.value) {
    if (r.type === "added") added += 1;
    else if (r.type === "removed") removed += 1;
    else if (r.type === "modified") modified += 1;
  }
  return { added, removed, modified };
});

watch(
  () => [environments.value.map((e) => e.id).join(","), currentId.value] as const,
  () => {
    if (!environments.value.length) {
      selectedEnvId.value = "";
      return;
    }
    if (!selectedEnvId.value || !environments.value.some((e) => e.id === selectedEnvId.value)) {
      selectedEnvId.value = currentId.value && environments.value.some((e) => e.id === currentId.value)
        ? currentId.value
        : environments.value[0].id;
    }
  },
  { immediate: true }
);

watch(
  () => [selectedEnvId.value, components.value.join("|")] as const,
  () => {
    if (!components.value.length) {
      selectedComponent.value = "";
      selectedManifestId.value = "";
      return;
    }
    if (!selectedComponent.value || !components.value.includes(selectedComponent.value)) {
      selectedComponent.value = components.value[0];
    }
    diffRows.value = [];
  },
  { immediate: true }
);

watch(
  () => [selectedEnvId.value, environments.value.map((e) => e.id).join(",")] as const,
  () => {
    const candidates = environments.value.filter((e) => e.id !== selectedEnvId.value);
    if (!candidates.length) {
      copyTargetEnvId.value = "";
      return;
    }
    if (!copyTargetEnvId.value || !candidates.some((e) => e.id === copyTargetEnvId.value)) {
      copyTargetEnvId.value = candidates[0].id;
    }
  },
  { immediate: true }
);

watch(
  () => [selectedComponent.value, manifestsByComponent.value.map((m) => m.id).join(",")] as const,
  () => {
    if (!manifestsByComponent.value.length) {
      selectedManifestId.value = "";
      editYaml.value = "";
      return;
    }
    if (!selectedManifestId.value || !manifestsByComponent.value.some((m) => m.id === selectedManifestId.value)) {
      selectedManifestId.value = manifestsByComponent.value[0].id;
    }
    diffRows.value = [];
  },
  { immediate: true }
);

watch(
  () => selectedManifest.value?.id,
  () => {
    editYaml.value = selectedManifest.value?.yaml ?? "";
    validationErrors.value = [];
    validationWarnings.value = [];
    diffRows.value = [];
    opError.value = null;
    opMessage.value = null;
  },
  { immediate: true }
);

function parseIdentity(yaml: string): ParseIdentity | null {
  const parsed = jsYaml.load(yaml);
  if (!parsed || typeof parsed !== "object") return null;
  const obj = parsed as Record<string, unknown>;
  const kind = typeof obj.kind === "string" ? obj.kind.trim() : "";
  const metadata = obj.metadata && typeof obj.metadata === "object" ? (obj.metadata as Record<string, unknown>) : null;
  const name = metadata && typeof metadata.name === "string" ? metadata.name.trim() : "";
  const namespace =
    metadata && typeof metadata.namespace === "string" && metadata.namespace.trim()
      ? metadata.namespace.trim()
      : null;
  if (!kind || !name) return null;
  return { kind, name, namespace };
}

function extractRefCheckWarnings(yaml: string, inventory: OrchestratorManifest[]): string[] {
  const warnings: string[] = [];
  let parsed: unknown;
  try {
    parsed = jsYaml.load(yaml);
  } catch {
    return warnings;
  }
  if (!parsed || typeof parsed !== "object") return warnings;
  const obj = parsed as Record<string, unknown>;
  const kind = typeof obj.kind === "string" ? obj.kind : "";
  if (!["Deployment", "StatefulSet", "DaemonSet"].includes(kind)) return warnings;

  const known = new Set(
    inventory.map((m) => `${m.resource_kind}|${m.resource_namespace ?? ""}|${m.resource_name}`)
  );
  const spec = obj.spec as Record<string, unknown> | undefined;
  const template = spec?.template as Record<string, unknown> | undefined;
  const podSpec = template?.spec as Record<string, unknown> | undefined;
  const metadata = obj.metadata as Record<string, unknown> | undefined;
  const defaultNs =
    metadata && typeof metadata.namespace === "string" && metadata.namespace.trim()
      ? metadata.namespace
      : "";

  const checkRef = (targetKind: "ConfigMap" | "Secret", name: string) => {
    const key = `${targetKind}|${defaultNs}|${name}`;
    if (!known.has(key) && !known.has(`${targetKind}||${name}`)) {
      warnings.push(`未在当前组件中发现引用资源：${targetKind}/${name}`);
    }
  };

  const volumes = Array.isArray(podSpec?.volumes) ? (podSpec?.volumes as Array<Record<string, unknown>>) : [];
  for (const v of volumes) {
    const cm = v.configMap as Record<string, unknown> | undefined;
    if (cm && typeof cm.name === "string" && cm.name) checkRef("ConfigMap", cm.name);
    const sec = v.secret as Record<string, unknown> | undefined;
    if (sec && typeof sec.secretName === "string" && sec.secretName) checkRef("Secret", sec.secretName);
  }

  const containers = [
    ...(Array.isArray(podSpec?.containers) ? (podSpec?.containers as Array<Record<string, unknown>>) : []),
    ...(Array.isArray(podSpec?.initContainers)
      ? (podSpec?.initContainers as Array<Record<string, unknown>>)
      : []),
  ];
  for (const c of containers) {
    const envFrom = Array.isArray(c.envFrom) ? (c.envFrom as Array<Record<string, unknown>>) : [];
    for (const ef of envFrom) {
      const cm = ef.configMapRef as Record<string, unknown> | undefined;
      if (cm && typeof cm.name === "string" && cm.name) checkRef("ConfigMap", cm.name);
      const sec = ef.secretRef as Record<string, unknown> | undefined;
      if (sec && typeof sec.name === "string" && sec.name) checkRef("Secret", sec.name);
    }
    const env = Array.isArray(c.env) ? (c.env as Array<Record<string, unknown>>) : [];
    for (const e of env) {
      const vf = e.valueFrom as Record<string, unknown> | undefined;
      const cm = vf?.configMapKeyRef as Record<string, unknown> | undefined;
      if (cm && typeof cm.name === "string" && cm.name) checkRef("ConfigMap", cm.name);
      const sec = vf?.secretKeyRef as Record<string, unknown> | undefined;
      if (sec && typeof sec.name === "string" && sec.name) checkRef("Secret", sec.name);
    }
  }
  return Array.from(new Set(warnings));
}

function validateCurrent(): boolean {
  validationErrors.value = [];
  validationWarnings.value = [];
  if (!selectedManifest.value) {
    validationErrors.value.push("未选择资源。");
    return false;
  }
  let parsed: unknown;
  try {
    parsed = jsYaml.load(editYaml.value);
  } catch (e) {
    validationErrors.value.push(`YAML 语法错误：${e instanceof Error ? e.message : String(e)}`);
    return false;
  }
  if (!parsed || typeof parsed !== "object") {
    validationErrors.value.push("YAML 必须是对象结构。");
    return false;
  }
  const obj = parsed as Record<string, unknown>;
  if (typeof obj.apiVersion !== "string" || !obj.apiVersion) validationErrors.value.push("缺少 apiVersion。");
  if (typeof obj.kind !== "string" || !obj.kind) validationErrors.value.push("缺少 kind。");
  const metadata = obj.metadata && typeof obj.metadata === "object" ? (obj.metadata as Record<string, unknown>) : null;
  if (!metadata) validationErrors.value.push("缺少 metadata。");
  if (!metadata || typeof metadata.name !== "string" || !metadata.name) {
    validationErrors.value.push("缺少 metadata.name。");
  }
  if (validationErrors.value.length > 0) return false;

  validationWarnings.value = extractRefCheckWarnings(editYaml.value, manifestsByComponent.value);
  return true;
}

function onSaveYaml() {
  if (!selectedManifest.value) return;
  const ok = validateCurrent();
  if (!ok) return;
  const identity = parseIdentity(editYaml.value);
  if (!identity) {
    opError.value = "无法解析资源身份信息（kind/metadata.name）。";
    return;
  }
  setManifestComponent(selectedManifest.value.id, selectedComponent.value);
  setManifestIdentity(selectedManifest.value.id, identity);
  saveManifestYaml(selectedManifest.value.id, editYaml.value, "save");
  opError.value = null;
  opMessage.value = "已保存资源 YAML。";
}

async function onApplyCurrent() {
  if (!selectedManifest.value || !selectedEnvId.value) return;
  const ok = validateCurrent();
  if (!ok) return;
  applying.value = true;
  opError.value = null;
  opMessage.value = null;
  try {
    const identity = parseIdentity(editYaml.value);
    if (!identity) throw new Error("无法解析资源身份信息。");
    await kubeApplyResource(selectedEnvId.value, editYaml.value);
    setManifestComponent(selectedManifest.value.id, selectedComponent.value);
    setManifestIdentity(selectedManifest.value.id, identity);
    saveManifestYaml(selectedManifest.value.id, editYaml.value, "apply");
    opMessage.value = `应用成功：${identity.kind}/${identity.name}`;
  } catch (e) {
    opError.value = e instanceof Error ? e.message : String(e);
  } finally {
    applying.value = false;
  }
}

async function onApplyComponent() {
  if (!selectedEnvId.value || !selectedComponent.value) return;
  const list = [...manifestsByComponent.value].sort((a, b) => {
    const wa = APPLY_ORDER[a.resource_kind] ?? 999;
    const wb = APPLY_ORDER[b.resource_kind] ?? 999;
    if (wa !== wb) return wa - wb;
    return a.resource_name.localeCompare(b.resource_name);
  });
  if (!list.length) return;
  applying.value = true;
  opError.value = null;
  opMessage.value = null;
  const failed: string[] = [];
  for (const item of list) {
    try {
      await kubeApplyResource(selectedEnvId.value, item.yaml);
      saveManifestYaml(item.id, item.yaml, "apply");
    } catch (e) {
      failed.push(`${item.resource_kind}/${item.resource_name}: ${e instanceof Error ? e.message : String(e)}`);
    }
  }
  applying.value = false;
  if (failed.length) {
    opError.value = `组件应用部分失败：${failed.join("；")}`;
  } else {
    opMessage.value = `组件 ${selectedComponent.value} 已完成应用。`;
  }
}

function openApplyDialog() {
  if (!canOpenApplyDialog.value || applying.value) return;
  applyDialogVisible.value = true;
}

function closeApplyDialog() {
  applyDialogVisible.value = false;
}

function openCopyDialog() {
  if (!canOpenCopyDialog.value || copyLoading.value) return;
  copyDialogVisible.value = true;
}

function closeCopyDialog() {
  if (copyLoading.value) return;
  copyDialogVisible.value = false;
}

async function onApplyCurrentFromDialog() {
  if (!canApplyCurrent.value || applying.value) return;
  applyDialogVisible.value = false;
  await onApplyCurrent();
}

async function onApplyComponentFromDialog() {
  if (!canApplyComponent.value || applying.value) return;
  applyDialogVisible.value = false;
  await onApplyComponent();
}

async function loadDiff() {
  if (!selectedManifest.value || !selectedEnvId.value) return;
  diffLoading.value = true;
  opError.value = null;
  try {
    const identity = parseIdentity(editYaml.value);
    if (!identity) throw new Error("请先保证 YAML 包含 kind 与 metadata.name。");
    const remote = await kubeGetResource(
      selectedEnvId.value,
      identity.kind,
      identity.name,
      identity.namespace
    );
    const remoteNormalized = normalizeYamlForDiff(remote);
    const draftNormalized = normalizeYamlForDiff(editYaml.value);
    diffRows.value = buildDiffRows(remoteNormalized, draftNormalized);
  } catch (e) {
    opError.value = e instanceof Error ? e.message : String(e);
  } finally {
    diffLoading.value = false;
  }
}

function closeDiff() {
  diffRows.value = [];
}

function buildDiffRows(oldText: string, newText: string): DiffRow[] {
  const a = oldText.replace(/\r\n/g, "\n").split("\n");
  const b = newText.replace(/\r\n/g, "\n").split("\n");
  const dp: number[][] = Array.from({ length: a.length + 1 }, () => Array(b.length + 1).fill(0));
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const ops: Array<{ op: "=" | "-" | "+"; text: string }> = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      ops.push({ op: "=", text: a[i] });
      i++;
      j++;
      continue;
    }
    if (dp[i + 1][j] >= dp[i][j + 1]) {
      ops.push({ op: "-", text: a[i] });
      i++;
    } else {
      ops.push({ op: "+", text: b[j] });
      j++;
    }
  }
  while (i < a.length) ops.push({ op: "-", text: a[i++] });
  while (j < b.length) ops.push({ op: "+", text: b[j++] });

  const rows: DiffRow[] = [];
  let leftNo = 1;
  let rightNo = 1;
  let p = 0;
  while (p < ops.length) {
    const cur = ops[p];
    if (cur.op === "=") {
      rows.push({
        type: "context",
        leftLineNo: leftNo++,
        rightLineNo: rightNo++,
        leftText: cur.text,
        rightText: cur.text,
      });
      p++;
      continue;
    }

    const delBuf: string[] = [];
    const addBuf: string[] = [];
    while (p < ops.length && ops[p].op !== "=") {
      if (ops[p].op === "-") delBuf.push(ops[p].text);
      if (ops[p].op === "+") addBuf.push(ops[p].text);
      p++;
    }
    const modifiedCount = Math.min(delBuf.length, addBuf.length);
    for (let k = 0; k < modifiedCount; k++) {
      rows.push({
        type: "modified",
        leftLineNo: leftNo++,
        rightLineNo: rightNo++,
        leftText: delBuf[k],
        rightText: addBuf[k],
      });
    }
    for (let k = modifiedCount; k < delBuf.length; k++) {
      rows.push({
        type: "removed",
        leftLineNo: leftNo++,
        rightLineNo: null,
        leftText: delBuf[k],
        rightText: "",
      });
    }
    for (let k = modifiedCount; k < addBuf.length; k++) {
      rows.push({
        type: "added",
        leftLineNo: null,
        rightLineNo: rightNo++,
        leftText: "",
        rightText: addBuf[k],
      });
    }
  }
  return rows;
}

function normalizeYamlForDiff(yaml: string): string {
  try {
    const parsed = jsYaml.load(yaml);
    if (!parsed || typeof parsed !== "object") return yaml;
    const obj = deepClone(parsed as Record<string, unknown>);
    const meta =
      obj.metadata && typeof obj.metadata === "object"
        ? (obj.metadata as Record<string, unknown>)
        : null;
    if (meta) {
      delete meta.uid;
      delete meta.resourceVersion;
      delete meta.managedFields;
      delete meta.creationTimestamp;
    }
    delete obj.status;
    return jsYaml.dump(obj, { lineWidth: -1, sortKeys: true });
  } catch {
    return yaml;
  }
}

function deepClone<T>(value: T): T {
  if (Array.isArray(value)) {
    return value.map((item) => deepClone(item)) as T;
  }
  if (value && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
      out[k] = deepClone(v);
    }
    return out as T;
  }
  return value;
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function tokenizeForInlineDiff(text: string): string[] {
  return text.match(/(\s+|[^\s]+)/g) ?? [];
}

function buildTokenOps(left: string, right: string): TokenOp[] {
  const a = tokenizeForInlineDiff(left);
  const b = tokenizeForInlineDiff(right);
  const dp: number[][] = Array.from({ length: a.length + 1 }, () => Array(b.length + 1).fill(0));
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }

  const ops: TokenOp[] = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      ops.push({ op: "=", text: a[i] });
      i++;
      j++;
      continue;
    }
    if (dp[i + 1][j] >= dp[i][j + 1]) {
      ops.push({ op: "-", text: a[i] });
      i++;
    } else {
      ops.push({ op: "+", text: b[j] });
      j++;
    }
  }
  while (i < a.length) ops.push({ op: "-", text: a[i++] });
  while (j < b.length) ops.push({ op: "+", text: b[j++] });
  return ops;
}

function renderInlineDiff(left: string, right: string, side: "left" | "right"): string {
  const ops = buildTokenOps(left, right);
  const parts: string[] = [];
  for (const op of ops) {
    const text = escapeHtml(op.text);
    if (op.op === "=") {
      parts.push(text);
      continue;
    }
    if (side === "left" && op.op === "-") {
      parts.push(`<span class="inline-removed">${text}</span>`);
      continue;
    }
    if (side === "right" && op.op === "+") {
      parts.push(`<span class="inline-added">${text}</span>`);
      continue;
    }
  }
  return parts.join("");
}

function formatCodeCell(row: DiffRow, side: "left" | "right"): string {
  const raw = side === "left" ? row.leftText : row.rightText;
  if (!raw) return "";
  if (row.type !== "modified") return escapeHtml(raw);
  return renderInlineDiff(row.leftText, row.rightText, side);
}

function onCreateManifest() {
  if (!selectedEnvId.value) return;
  const env = environments.value.find((e) => e.id === selectedEnvId.value);
  if (!env) return;
  const component = selectedComponent.value || "default";
  const item = createManifestDraft(selectedEnvId.value, env.display_name, component, [
    "apiVersion: v1",
    "kind: ConfigMap",
    "metadata:",
    "  name: example-config",
    "  namespace: default",
    "data:",
    "  key: value",
    "",
  ].join("\n"));
  selectedComponent.value = item.component;
  selectedManifestId.value = item.id;
}

function onDeleteManifest() {
  if (!selectedManifest.value) return;
  if (deleteArmedId.value !== selectedManifest.value.id) {
    deleteArmedId.value = selectedManifest.value.id;
    opError.value = null;
    opMessage.value = `再次点击“删除”以确认：${selectedManifest.value.resource_kind}/${selectedManifest.value.resource_name}`;
    return;
  }
  deleteManifest(selectedManifest.value.id);
  deleteArmedId.value = "";
  opError.value = null;
  opMessage.value = "资源已删除。";
}

function onRestoreHistory(yaml: string) {
  editYaml.value = yaml;
  deleteArmedId.value = "";
  opError.value = null;
  opMessage.value = "已加载历史快照到编辑区，保存或应用后生效。";
}

async function onCopyComponentToEnv() {
  if (!selectedEnvId.value || !selectedComponent.value || !copyTargetEnvId.value) return;
  const target = environments.value.find((e) => e.id === copyTargetEnvId.value);
  if (!target) return;
  copyLoading.value = true;
  opError.value = null;
  opMessage.value = null;
  try {
    const result = copyComponentToEnv(
      selectedEnvId.value,
      selectedComponent.value,
      copyTargetEnvId.value,
      target.display_name,
      copyOverwrite.value
    );
    opMessage.value = `组件已复制到 ${target.display_name}：新增 ${result.copied}，更新 ${result.updated}，跳过 ${result.skipped}`;
  } catch (e) {
    opError.value = e instanceof Error ? e.message : String(e);
  } finally {
    copyLoading.value = false;
    copyDialogVisible.value = false;
  }
}
</script>

<template>
  <div class="orchestrator-layout">
    <header class="toolbar">
      <span class="title">资源编排台</span>
      <select v-model="selectedEnvId" class="select">
        <option value="" disabled>选择环境</option>
        <option v-for="env in environments" :key="env.id" :value="env.id">
          {{ env.display_name }}
        </option>
      </select>
      <select v-model="selectedComponent" class="select" :disabled="!components.length">
        <option value="" disabled>选择组件</option>
        <option v-for="name in components" :key="name" :value="name">{{ name }}</option>
      </select>
      <button type="button" class="btn btn-create" :disabled="!selectedEnvId" @click="onCreateManifest">新建资源</button>
      <button type="button" class="btn btn-save" :disabled="!selectedManifestId" @click="onSaveYaml">保存</button>
      <button type="button" class="btn btn-primary" :disabled="!canOpenApplyDialog || applying" @click="openApplyDialog">
        {{ applying ? "应用中…" : "应用" }}
      </button>
      <button type="button" class="btn btn-diff" :disabled="!selectedManifestId || diffLoading" @click="loadDiff">
        {{ diffLoading ? "生成中…" : "查看差异" }}
      </button>
      <button
        type="button"
        class="btn btn-danger"
        :class="{ armed: selectedManifestId && deleteArmedId === selectedManifestId }"
        :disabled="!selectedManifestId"
        @click="onDeleteManifest"
      >
        {{ selectedManifestId && deleteArmedId === selectedManifestId ? "确认删除" : "删除" }}
      </button>
      <button type="button" class="btn btn-copy" :disabled="!canOpenCopyDialog || copyLoading" @click="openCopyDialog">
        {{ copyLoading ? "复制中…" : "复制到环境" }}
      </button>
    </header>

    <div class="body">
      <aside class="list">
        <div v-if="!selectedEnvId" class="empty">请先选择环境</div>
        <template v-else>
          <div
            v-for="m in manifestsByComponent"
            :key="m.id"
            class="item"
            :class="{ active: selectedManifestId === m.id }"
            @click="selectedManifestId = m.id"
          >
            <div class="item-title">{{ m.resource_kind }} / {{ m.resource_name }}</div>
            <div class="item-sub">{{ m.resource_namespace || "default" }}</div>
          </div>
          <div v-if="!manifestsByComponent.length" class="empty">当前组件暂无资源</div>
        </template>
      </aside>

      <section class="editor-panel">
        <div v-if="selectedManifest" class="meta-row">
          <label class="meta-field">
            <span>组件</span>
            <input
              :value="selectedManifest.component"
              type="text"
              @change="setManifestComponent(selectedManifest.id, ($event.target as HTMLInputElement).value)"
            />
          </label>
          <span class="hint">资源：{{ selectedManifest.resource_kind }}/{{ selectedManifest.resource_name }}</span>
        </div>
        <CodeEditor
          v-model:value="editYaml"
          language="yaml"
          :theme="monacoTheme"
          :options="editorOptions"
          class="editor"
        />
        <div v-if="validationErrors.length" class="message message-error">
          {{ validationErrors.join("；") }}
        </div>
        <div v-if="validationWarnings.length" class="message message-warn">
          {{ validationWarnings.join("；") }}
        </div>
        <div v-if="opError" class="message message-error">{{ opError }}</div>
        <div v-if="opMessage" class="message message-ok">{{ opMessage }}</div>
      </section>

      <aside class="history">
        <div class="history-title">历史快照</div>
        <div v-if="selectedHistory.length === 0" class="empty">暂无历史</div>
        <button
          v-for="h in selectedHistory"
          :key="h.id"
          type="button"
          class="history-item"
          :class="`history-${h.action}`"
          @click="onRestoreHistory(h.yaml)"
        >
          <span>{{ h.action }}</span>
          <span>{{ new Date(h.at).toLocaleString() }}</span>
        </button>
      </aside>
    </div>

    <Teleport to="body">
      <div v-if="diffRows.length" class="diff-modal-overlay" @click.self="closeDiff">
        <section class="diff-modal" role="dialog" aria-label="差异详情">
          <div class="diff-head">
            <div class="diff-title">
              集群与草稿差异
              <span class="diff-stat added">+{{ diffStats.added }}</span>
              <span class="diff-stat removed">-{{ diffStats.removed }}</span>
              <span class="diff-stat modified">~{{ diffStats.modified }}</span>
            </div>
            <button type="button" class="btn btn-small" @click="closeDiff">关闭差异</button>
          </div>
          <div class="diff-table-wrap">
            <table class="diff-table">
              <tbody>
                <tr v-for="(row, idx) in diffRows" :key="idx" :class="`row-${row.type}`">
                  <td class="ln">{{ row.leftLineNo ?? "" }}</td>
                  <td class="code left" v-html="formatCodeCell(row, 'left')"></td>
                  <td class="ln">{{ row.rightLineNo ?? "" }}</td>
                  <td class="code right" v-html="formatCodeCell(row, 'right')"></td>
                </tr>
              </tbody>
            </table>
          </div>
        </section>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-if="applyDialogVisible" class="apply-modal-overlay" @click.self="closeApplyDialog">
        <section class="apply-modal" role="dialog" aria-label="选择应用范围">
          <header class="apply-head">
            <h3>选择应用范围</h3>
          </header>
          <div class="apply-body">
            <button
              type="button"
              class="apply-option"
              :disabled="!canApplyCurrent || applying"
              @click="onApplyCurrentFromDialog"
            >
              <span class="apply-option-title">应用当前</span>
              <span class="apply-option-desc">仅应用当前选中的资源 YAML</span>
            </button>
            <button
              type="button"
              class="apply-option"
              :disabled="!canApplyComponent || applying"
              @click="onApplyComponentFromDialog"
            >
              <span class="apply-option-title">应用组件</span>
              <span class="apply-option-desc">按顺序应用当前组件下全部资源</span>
            </button>
          </div>
          <footer class="apply-foot">
            <button type="button" class="btn" @click="closeApplyDialog">取消</button>
          </footer>
        </section>
      </div>
    </Teleport>
    <Teleport to="body">
      <div v-if="copyDialogVisible" class="apply-modal-overlay" @click.self="closeCopyDialog">
        <section class="apply-modal" role="dialog" aria-label="复制组件到环境">
          <header class="apply-head">
            <h3>复制组件到环境</h3>
          </header>
          <div class="apply-body">
            <label class="field-label">
              <span>目标环境</span>
              <select v-model="copyTargetEnvId" class="select copy-select" :disabled="copyLoading">
                <option value="" disabled>选择目标环境</option>
                <option v-for="env in environments.filter((e) => e.id !== selectedEnvId)" :key="env.id" :value="env.id">
                  {{ env.display_name }}
                </option>
              </select>
            </label>
            <label class="field-check">
              <input v-model="copyOverwrite" type="checkbox" :disabled="copyLoading" />
              覆盖同名资源
            </label>
            <div class="copy-tip">将复制当前环境下组件 <strong>{{ selectedComponent }}</strong> 的全部资源 YAML。</div>
          </div>
          <footer class="apply-foot">
            <button type="button" class="btn" :disabled="copyLoading" @click="closeCopyDialog">取消</button>
            <button
              type="button"
              class="btn btn-primary"
              :disabled="!copyTargetEnvId || copyLoading"
              @click="onCopyComponentToEnv"
            >
              {{ copyLoading ? "复制中…" : "开始复制" }}
            </button>
          </footer>
        </section>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.orchestrator-layout {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 0.75rem;
  border-bottom: 1px solid #e2e8f0;
  background: #fff;
}
.title {
  font-size: 0.9375rem;
  font-weight: 600;
  margin-right: 0.5rem;
}
.select {
  height: 2rem;
  border: 1px solid #d1d5db;
  border-radius: 6px;
  padding: 0 0.5rem;
}
.btn {
  height: 2rem;
  border: 1px solid #d1d5db;
  border-radius: 6px;
  background: #fff;
  padding: 0 0.65rem;
  font-size: 0.8125rem;
  cursor: pointer;
}
.btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.btn-primary {
  border-color: #2563eb;
  background: #2563eb;
  color: #fff;
}
.btn-create {
  border-color: #0891b2;
  background: #0891b2;
  color: #fff;
}
.btn-create:hover:not(:disabled) {
  border-color: #0e7490;
  background: #0e7490;
}
.btn-create:disabled {
  border-color: #a5f3fc;
  background: #cffafe;
  color: #ecfeff;
}
.btn-save {
  border-color: #16a34a;
  background: #16a34a;
  color: #fff;
}
.btn-save:hover:not(:disabled) {
  border-color: #15803d;
  background: #15803d;
}
.btn-save:disabled {
  border-color: #86efac;
  background: #dcfce7;
  color: #f0fdf4;
}
.btn-copy {
  border-color: #0f766e;
  background: #0f766e;
  color: #fff;
}
.btn-copy:hover:not(:disabled) {
  border-color: #115e59;
  background: #115e59;
}
.btn-copy:disabled {
  border-color: #99f6e4;
  background: #ccfbf1;
  color: #f0fdfa;
}
.btn-diff {
  border-color: #7c3aed;
  background: #7c3aed;
  color: #fff;
}
.btn-diff:hover:not(:disabled) {
  background: #6d28d9;
  border-color: #6d28d9;
}
.btn-diff:disabled {
  border-color: #c4b5fd;
  background: #ddd6fe;
  color: #f5f3ff;
}
.btn-danger {
  border-color: #dc2626;
  color: #dc2626;
}
.btn-danger.armed {
  background: #dc2626;
  color: #fff;
}
.body {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 260px 1fr 240px;
}
.list,
.history {
  border-right: 1px solid #e2e8f0;
  overflow: auto;
  padding: 0.5rem;
}
.history {
  border-left: 1px solid #e2e8f0;
  border-right: none;
}
.item {
  padding: 0.45rem 0.5rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  margin-bottom: 0.4rem;
  cursor: pointer;
}
.item.active {
  border-color: #2563eb;
  background: #eff6ff;
}
.item-title {
  font-size: 0.8125rem;
  font-weight: 600;
}
.item-sub {
  font-size: 0.75rem;
  color: #64748b;
}
.editor-panel {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
}
.meta-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.5rem;
  border-bottom: 1px solid #e2e8f0;
  background: #f8fafc;
}
.meta-field {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.8125rem;
}
.meta-field input {
  height: 1.8rem;
  border: 1px solid #d1d5db;
  border-radius: 4px;
  padding: 0 0.45rem;
}
.hint {
  font-size: 0.75rem;
  color: #64748b;
}
.editor {
  flex: 1;
  min-height: 260px;
}
.message {
  margin: 0.35rem 0.5rem 0.25rem;
  padding: 0.4rem 0.5rem;
  font-size: 0.8125rem;
  border-radius: 6px;
}
.message-error {
  background: #fef2f2;
  color: #b91c1c;
}
.message-warn {
  background: #fff7ed;
  color: #c2410c;
}
.message-ok {
  background: #ecfdf5;
  color: #047857;
}
.history-title {
  font-size: 0.8125rem;
  font-weight: 600;
  margin-bottom: 0.5rem;
}
.history-item {
  width: 100%;
  text-align: left;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  background: #fff;
  padding: 0.35rem 0.5rem;
  margin-bottom: 0.35rem;
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
  font-size: 0.75rem;
  cursor: pointer;
}
.history-item.history-sync {
  border-color: #0e7490;
  background: #ecfeff;
  box-shadow: inset 2px 0 0 #0891b2;
}
.history-item.history-save {
  border-color: #16a34a;
  background: #f0fdf4;
}
.history-item.history-apply {
  border-color: #6d28d9;
  background: #f5f3ff;
  box-shadow: inset 2px 0 0 #7c3aed;
}
.history-item.history-restore {
  border-color: #d97706;
  background: #fffbeb;
}
.history-item:hover {
  filter: brightness(0.98);
}
.empty {
  color: #94a3b8;
  font-size: 0.8125rem;
  padding: 0.5rem 0.25rem;
}
.diff-modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(15, 23, 42, 0.5);
  z-index: 2000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1rem;
}
.diff-modal {
  width: min(96vw, 1600px);
  height: min(92vh, 980px);
  background: #fff;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  box-shadow: 0 24px 48px rgba(15, 23, 42, 0.28);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.diff-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.6rem 0.75rem;
  border-bottom: 1px solid #e2e8f0;
  background: #f8fafc;
}
.diff-title {
  font-size: 0.8125rem;
  color: #475569;
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
}
.diff-stat {
  display: inline-flex;
  align-items: center;
  padding: 0.05rem 0.4rem;
  border-radius: 999px;
  font-size: 0.6875rem;
  font-weight: 600;
}
.diff-stat.added {
  background: #dcfce7;
  color: #166534;
}
.diff-stat.removed {
  background: #fee2e2;
  color: #991b1b;
}
.diff-stat.modified {
  background: #fef3c7;
  color: #92400e;
}
.diff-table-wrap {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.diff-table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}
.diff-table .ln {
  width: 3rem;
  text-align: right;
  padding: 0.25rem 0.4rem;
  font-size: 0.75rem;
  color: #64748b;
  border-right: 1px solid #e2e8f0;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  vertical-align: top;
}
.diff-table .code {
  padding: 0.25rem 0.45rem;
  font-size: 0.75rem;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  vertical-align: top;
}
.diff-table .left {
  border-right: 1px solid #e2e8f0;
}
.diff-table tr.row-added .right,
.diff-table tr.row-added .ln:last-of-type {
  background: #f0fdf4;
}
.diff-table tr.row-removed .left,
.diff-table tr.row-removed .ln:first-of-type {
  background: #fef2f2;
}
.diff-table tr.row-modified .code,
.diff-table tr.row-modified .ln {
  background: #fffbeb;
}
.diff-table .code :deep(.inline-removed) {
  background: #fecaca;
  color: #7f1d1d;
  border-radius: 3px;
}
.diff-table .code :deep(.inline-added) {
  background: #bbf7d0;
  color: #14532d;
  border-radius: 3px;
}
.apply-modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(15, 23, 42, 0.42);
  z-index: 2100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1rem;
}
.apply-modal {
  width: min(92vw, 560px);
  background: #fff;
  border: 1px solid #cbd5e1;
  border-radius: 10px;
  box-shadow: 0 24px 48px rgba(15, 23, 42, 0.24);
  overflow: hidden;
}
.apply-head {
  padding: 0.75rem 0.9rem;
  border-bottom: 1px solid #e2e8f0;
  background: #f8fafc;
}
.apply-head h3 {
  margin: 0;
  font-size: 0.95rem;
  color: #0f172a;
}
.field-label {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  font-size: 0.8rem;
  color: #334155;
}
.copy-select {
  width: 100%;
}
.field-check {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.8rem;
  color: #334155;
}
.copy-tip {
  font-size: 0.75rem;
  color: #64748b;
}
.apply-body {
  display: grid;
  gap: 0.65rem;
  padding: 0.85rem;
}
.apply-option {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.25rem;
  border: 1px solid #cbd5e1;
  border-radius: 8px;
  background: #f8fafc;
  padding: 0.7rem 0.75rem;
  cursor: pointer;
}
.apply-option:hover:not(:disabled) {
  border-color: #2563eb;
  background: #eff6ff;
}
.apply-option:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
.apply-option-title {
  font-size: 0.86rem;
  font-weight: 600;
  color: #0f172a;
}
.apply-option-desc {
  font-size: 0.78rem;
  color: #64748b;
}
.apply-foot {
  display: flex;
  justify-content: flex-end;
  padding: 0.75rem 0.85rem;
  border-top: 1px solid #e2e8f0;
  background: #f8fafc;
}
.btn-small {
  height: 1.7rem;
  padding: 0 0.5rem;
  font-size: 0.75rem;
}
</style>
