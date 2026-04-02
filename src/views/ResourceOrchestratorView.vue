<script setup lang="ts">
import { computed, ref, watch } from "vue";
import * as jsYaml from "js-yaml";
import { CodeEditor } from "monaco-editor-vue3";
import { kubeDeployResource, kubeGetResource } from "../api/kube";
import { appSettingsGetResourceDeployStrategy } from "../api/config";
import { useEnvStore } from "../stores/env";
import { useYamlMonacoTheme } from "../stores/yamlTheme";
import {
  useOrchestratorStore,
  type OrchestratorManifest,
  type OrchestratorPackage,
  type OrchestratorPackageVersion,
  type OrchestratorPackageResourceSnapshot,
} from "../stores/orchestrator";

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
  packages,
  orchestratorFocusTarget,
  createManifestDraft,
  saveManifestYaml,
  setManifestIdentity,
  setManifestComponent,
  deleteManifest,
  copyComponentToEnv,
  createPackage,
  deletePackage,
  createPackageVersion,
  setPackageVersionTag,
  deletePackageVersion,
  syncPackageVersionToEnv,
  recordPackageDeployment,
} = useOrchestratorStore();
const { monacoTheme } = useYamlMonacoTheme();

const activeView = ref<"resources" | "packages">("resources");
const selectedEnvId = ref<string>("");
const selectedComponent = ref<string>("");
const selectedManifestId = ref<string>("");
const editYaml = ref("");
const componentFilterKeyword = ref("");
const validationErrors = ref<string[]>([]);
const validationWarnings = ref<string[]>([]);
const opMessage = ref<string | null>(null);
const opError = ref<string | null>(null);
const applying = ref(false);
const applyDialogVisible = ref(false);
const copyDialogVisible = ref(false);
const copyTargetEnvId = ref("");
const copyOverwrite = ref(true);
const copyLoading = ref(false);
const diffLoading = ref(false);
const diffRows = ref<DiffRow[]>([]);
const selectedPackageId = ref("");
const selectedPackageVersionId = ref("");
const packageNameInput = ref("");
const packageDescriptionInput = ref("");
const packageComponentDraft = ref<string[]>([]);
const packageTargetEnvId = ref("");
const packageOverwrite = ref(true);
const packageWorking = ref(false);
const packageActionDialogVisible = ref(false);
const packageActionMode = ref<"sync" | "apply">("sync");
const packageDeleteDialogVisible = ref(false);
const versionDeleteDialogVisible = ref(false);
const editingVersionTagId = ref("");
const editingVersionTagValue = ref("");
const manifestDraftCache = ref<Record<string, string>>({});
const selectedManifestByComponent = ref<Record<string, string>>({});
const componentAssignMode = ref<"existing" | "new">("existing");
const componentAssignExisting = ref("");
const componentAssignNew = ref("");
const componentAssignDialogVisible = ref(false);
const listDeleteDialogVisible = ref(false);
const listContextTarget = ref<
  | { type: "resource"; manifestId: string; label: string }
  | { type: "component"; component: string; count: number }
  | null
>(null);

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

const componentItems = computed(() =>
  components.value.map((name) => ({
    name,
    count: manifestsByEnv.value.filter((m) => m.component === name).length,
  }))
);

const filteredComponentItems = computed(() => {
  const keyword = componentFilterKeyword.value.trim().toLowerCase();
  if (!keyword) return componentItems.value;
  return componentItems.value.filter((item) => item.name.toLowerCase().includes(keyword));
});

const manifestsByComponent = computed(() =>
  manifestsByEnv.value.filter((m) => m.component === selectedComponent.value)
);
const componentOptionsForAssign = computed(() =>
  components.value.filter((name) => name !== selectedManifest.value?.component)
);
const manifestDraftCount = computed(() =>
  manifestsByComponent.value.filter((m) => manifestDraftCache.value[m.id] && manifestDraftCache.value[m.id] !== m.yaml).length
);
const selectedPackage = computed<OrchestratorPackage | null>(
  () => packages.value.find((p) => p.id === selectedPackageId.value) ?? null
);
const selectedPackageVersion = computed<OrchestratorPackageVersion | null>(
  () => selectedPackage.value?.versions.find((v) => v.id === selectedPackageVersionId.value) ?? null
);
const packageDeployments = computed(() => selectedPackage.value?.deployments ?? []);
const packageDraftComponents = computed(() =>
  components.value.map((name) => ({
    name,
    checked: packageComponentDraft.value.includes(name),
    count: manifestsByEnv.value.filter((m) => m.component === name).length,
  }))
);
const canCreatePackageVersion = computed(
  () => Boolean(selectedPackage.value && selectedEnvId.value && packageComponentDraft.value.length)
);
const canOpenPackageActionDialog = computed(
  () => Boolean(selectedPackageVersion.value && environments.value.length)
);
const canOperatePackageDeploy = computed(
  () => Boolean(selectedPackageVersion.value && packageTargetEnvId.value)
);
const selectedPackageStats = computed(() => {
  const pkg = selectedPackage.value;
  if (!pkg) return { versions: 0, resources: 0 };
  const resources = pkg.versions.reduce((sum, v) => sum + v.resources.length, 0);
  return { versions: pkg.versions.length, resources };
});

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

function onSelectComponent(componentName: string) {
  if (!componentName || componentName === selectedComponent.value) return;
  selectedComponent.value = componentName;
}

function onSelectManifest(manifestId: string) {
  selectedManifestId.value = manifestId;
  if (selectedComponent.value) {
    selectedManifestByComponent.value[selectedComponent.value] = manifestId;
  }
}

function applyPendingOrchestratorFocus() {
  const target = orchestratorFocusTarget.value;
  if (!target) return false;

  selectedEnvId.value = target.env_id;

  const envManifests = manifests.value.filter((m) => m.env_id === target.env_id);
  const componentManifests = envManifests.filter((m) => m.component === target.component);
  if (!componentManifests.length) return false;

  selectedComponent.value = target.component;

  const matchedManifest =
    (target.manifest_id
      ? componentManifests.find((m) => m.id === target.manifest_id)
      : undefined) ??
    componentManifests.find(
      (m) =>
        (!target.resource_kind || m.resource_kind === target.resource_kind) &&
        (!target.resource_name || m.resource_name === target.resource_name) &&
        (m.resource_namespace ?? null) === (target.resource_namespace ?? null)
    ) ??
    componentManifests[0];

  if (!matchedManifest) return false;

  selectedManifestId.value = matchedManifest.id;
  selectedManifestByComponent.value[target.component] = matchedManifest.id;
  orchestratorFocusTarget.value = null;
  return true;
}

function openDeleteResourceDialog(manifest: OrchestratorManifest) {
  onSelectManifest(manifest.id);
  selectedComponent.value = manifest.component;
  listContextTarget.value = {
    type: "resource",
    manifestId: manifest.id,
    label: `${manifest.resource_kind}/${manifest.resource_name}`,
  };
  listDeleteDialogVisible.value = true;
}

function openDeleteComponentDialog(componentName: string, count: number) {
  onSelectComponent(componentName);
  listContextTarget.value = {
    type: "component",
    component: componentName,
    count,
  };
  listDeleteDialogVisible.value = true;
}

function closeListDeleteDialog() {
  listDeleteDialogVisible.value = false;
}

function removeManifestById(manifestId: string) {
  const target = manifests.value.find((m) => m.id === manifestId);
  if (!target) return;
  delete manifestDraftCache.value[manifestId];
  if (selectedManifestByComponent.value[target.component] === manifestId) {
    delete selectedManifestByComponent.value[target.component];
  }
  deleteManifest(manifestId);
}

function confirmDeleteFromContextMenu() {
  const target = listContextTarget.value;
  if (!target) return;
  if (target.type === "resource") {
    removeManifestById(target.manifestId);
    opError.value = null;
    opMessage.value = "资源已删除。";
    listDeleteDialogVisible.value = false;
    listContextTarget.value = null;
    return;
  }

  const ids = manifests.value
    .filter((m) => m.env_id === selectedEnvId.value && m.component === target.component)
    .map((m) => m.id);
  for (const id of ids) removeManifestById(id);
  if (selectedComponent.value === target.component) {
    const next = components.value.find((name) => name !== target.component) ?? "";
    selectedComponent.value = next;
    if (!next) selectedManifestId.value = "";
  }
  opError.value = null;
  opMessage.value = `应用组件 ${target.component} 已删除（共 ${ids.length} 个资源）。`;
  listDeleteDialogVisible.value = false;
  listContextTarget.value = null;
}

function hasManifestDraft(manifestId: string): boolean {
  const draft = manifestDraftCache.value[manifestId];
  const manifest = manifests.value.find((m) => m.id === manifestId);
  if (!draft || !manifest) return false;
  return draft !== manifest.yaml;
}

function applyManifestComponentAssignment() {
  if (!selectedManifest.value) return;
  const current = selectedManifest.value.component;
  const target =
    componentAssignMode.value === "existing"
      ? componentAssignExisting.value.trim()
      : componentAssignNew.value.trim();
  if (!target) {
    opError.value = componentAssignMode.value === "existing" ? "请选择目标组件。" : "请输入新组件名称。";
    return;
  }
  if (target === current) {
    opError.value = null;
    opMessage.value = "组件未变化。";
    return;
  }
  setManifestComponent(selectedManifest.value.id, target);
  selectedComponent.value = target;
  selectedManifestByComponent.value[target] = selectedManifest.value.id;
  opError.value = null;
  opMessage.value = `已将资源移动到组件：${target}`;
  componentAssignDialogVisible.value = false;
  componentAssignExisting.value = "";
  componentAssignNew.value = "";
}

function openComponentAssignDialog() {
  if (!selectedManifest.value) return;
  const hasExisting = componentOptionsForAssign.value.length > 0;
  componentAssignMode.value = hasExisting ? "existing" : "new";
  componentAssignExisting.value = hasExisting ? componentOptionsForAssign.value[0] : "";
  componentAssignNew.value = "";
  componentAssignDialogVisible.value = true;
}

function closeComponentAssignDialog() {
  componentAssignDialogVisible.value = false;
}

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
  () => [orchestratorFocusTarget.value?.env_id ?? "", orchestratorFocusTarget.value?.manifest_id ?? "", manifests.value.length] as const,
  () => {
    applyPendingOrchestratorFocus();
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
    const rememberedId = selectedManifestByComponent.value[selectedComponent.value];
    if (rememberedId && manifestsByComponent.value.some((m) => m.id === rememberedId)) {
      selectedManifestId.value = rememberedId;
    } else if (!selectedManifestId.value || !manifestsByComponent.value.some((m) => m.id === selectedManifestId.value)) {
      selectedManifestId.value = manifestsByComponent.value[0].id;
    }
    if (selectedComponent.value && selectedManifestId.value) {
      selectedManifestByComponent.value[selectedComponent.value] = selectedManifestId.value;
    }
    diffRows.value = [];
  },
  { immediate: true }
);

watch(
  () => [packages.value.map((p) => p.id).join(","), selectedPackageId.value] as const,
  () => {
    if (!packages.value.length) {
      selectedPackageId.value = "";
      selectedPackageVersionId.value = "";
      return;
    }
    if (!selectedPackageId.value || !packages.value.some((p) => p.id === selectedPackageId.value)) {
      selectedPackageId.value = packages.value[0].id;
    }
  },
  { immediate: true }
);

watch(
  () => [selectedPackage.value?.id ?? "", selectedPackage.value?.versions.map((v) => v.id).join(",") ?? ""] as const,
  () => {
    const versions = selectedPackage.value?.versions ?? [];
    if (!versions.length) {
      selectedPackageVersionId.value = "";
      return;
    }
    if (!selectedPackageVersionId.value || !versions.some((v) => v.id === selectedPackageVersionId.value)) {
      selectedPackageVersionId.value = versions[0].id;
    }
  },
  { immediate: true }
);

watch(
  () => [selectedEnvId.value, environments.value.map((e) => e.id).join(",")] as const,
  () => {
    const candidates = environments.value;
    if (!candidates.length) {
      packageTargetEnvId.value = "";
      return;
    }
    if (!packageTargetEnvId.value || !candidates.some((e) => e.id === packageTargetEnvId.value)) {
      packageTargetEnvId.value = candidates[0].id;
    }
  },
  { immediate: true }
);

watch(
  () => selectedManifest.value?.id,
  (nextId, prevId) => {
    if (prevId) {
      const prev = manifests.value.find((m) => m.id === prevId);
      if (prev && editYaml.value !== prev.yaml) {
        manifestDraftCache.value[prevId] = editYaml.value;
      } else {
        delete manifestDraftCache.value[prevId];
      }
    }
    if (nextId) {
      const next = manifests.value.find((m) => m.id === nextId);
      editYaml.value = manifestDraftCache.value[nextId] ?? next?.yaml ?? "";
    } else {
      editYaml.value = "";
    }
    validationErrors.value = [];
    validationWarnings.value = [];
    diffRows.value = [];
    opError.value = null;
    opMessage.value = null;
    if (nextId) {
      componentAssignMode.value = "existing";
      componentAssignExisting.value = "";
      componentAssignNew.value = "";
    }
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
  delete manifestDraftCache.value[selectedManifest.value.id];
  opError.value = null;
  opMessage.value = "已保存资源 YAML。";
}

async function deployYamlToEnv(envId: string, yaml: string) {
  const strategy = await appSettingsGetResourceDeployStrategy();
  await kubeDeployResource(envId, yaml, strategy);
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
    await deployYamlToEnv(selectedEnvId.value, editYaml.value);
    setManifestComponent(selectedManifest.value.id, selectedComponent.value);
    setManifestIdentity(selectedManifest.value.id, identity);
    saveManifestYaml(selectedManifest.value.id, editYaml.value, "apply");
    delete manifestDraftCache.value[selectedManifest.value.id];
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
      await deployYamlToEnv(selectedEnvId.value, item.yaml);
      saveManifestYaml(item.id, item.yaml, "apply");
      delete manifestDraftCache.value[item.id];
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

function openPackageActionDialog(mode: "sync" | "apply") {
  if (!canOpenPackageActionDialog.value || packageWorking.value) return;
  packageActionMode.value = mode;
  packageActionDialogVisible.value = true;
}

function closePackageActionDialog() {
  if (packageWorking.value) return;
  packageActionDialogVisible.value = false;
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
  onSelectManifest(item.id);
}

function onRestoreHistory(yaml: string) {
  editYaml.value = yaml;
  if (selectedManifest.value) {
    manifestDraftCache.value[selectedManifest.value.id] = yaml;
  }
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

function onCreatePackage() {
  try {
    const pkg = createPackage(packageNameInput.value, packageDescriptionInput.value);
    selectedPackageId.value = pkg.id;
    packageNameInput.value = "";
    packageDescriptionInput.value = "";
    opError.value = null;
    opMessage.value = `已创建应用包：${pkg.name}`;
  } catch (e) {
    opError.value = e instanceof Error ? e.message : String(e);
  }
}

function openDeletePackageDialog() {
  if (!selectedPackage.value) return;
  packageDeleteDialogVisible.value = true;
}

function closeDeletePackageDialog() {
  if (packageWorking.value) return;
  packageDeleteDialogVisible.value = false;
}

function onDeletePackage() {
  if (!selectedPackage.value) return;
  const ok = deletePackage(selectedPackage.value.id);
  packageDeleteDialogVisible.value = false;
  if (ok) {
    opError.value = null;
    opMessage.value = "应用包已删除。";
    return;
  }
  opError.value = "删除应用包失败。";
}

function openDeleteVersionDialog() {
  if (!selectedPackage.value || !selectedPackageVersion.value) return;
  versionDeleteDialogVisible.value = true;
}

function closeDeleteVersionDialog() {
  if (packageWorking.value) return;
  versionDeleteDialogVisible.value = false;
}

function onDeletePackageVersion() {
  if (!selectedPackage.value || !selectedPackageVersion.value) return;
  const ok = deletePackageVersion(selectedPackage.value.id, selectedPackageVersion.value.id);
  versionDeleteDialogVisible.value = false;
  if (ok) {
    opError.value = null;
    opMessage.value = "版本已删除。";
    return;
  }
  opError.value = "删除版本失败。";
}

function startEditVersionTag(version: OrchestratorPackageVersion) {
  editingVersionTagId.value = version.id;
  editingVersionTagValue.value = version.tag ?? "";
}

function cancelEditVersionTag() {
  editingVersionTagId.value = "";
  editingVersionTagValue.value = "";
}

function onSaveVersionTag(versionId: string) {
  if (!selectedPackage.value) return;
  const ok = setPackageVersionTag(selectedPackage.value.id, versionId, editingVersionTagValue.value);
  if (!ok) {
    opError.value = "保存版本 Tag 失败。";
    return;
  }
  cancelEditVersionTag();
  opError.value = null;
  opMessage.value = "版本 Tag 已更新。";
}

function formatDateTime(iso: string): string {
  if (!iso) return "-";
  return new Date(iso).toLocaleString();
}

function onCreatePackageVersion() {
  if (!selectedPackage.value || !selectedEnvId.value) return;
  const env = environments.value.find((e) => e.id === selectedEnvId.value);
  if (!env) return;
  try {
    const version = createPackageVersion(
      selectedPackage.value.id,
      selectedEnvId.value,
      env.display_name,
      packageComponentDraft.value
    );
    selectedPackageVersionId.value = version.id;
    opError.value = null;
    opMessage.value = `已生成版本 ${version.label}（${version.resources.length} 个资源）。`;
  } catch (e) {
    opError.value = e instanceof Error ? e.message : String(e);
  }
}

function validatePackageVersion(version: OrchestratorPackageVersion): string[] {
  const errs: string[] = [];
  const keys = new Set<string>();
  for (const r of version.resources) {
    const key = `${r.component}|${r.resource_kind}|${r.resource_namespace ?? ""}|${r.resource_name}`;
    if (keys.has(key)) errs.push(`存在重复资源：${r.component} / ${r.resource_kind}/${r.resource_name}`);
    keys.add(key);
  }
  return errs;
}

function sortPackageResources(list: OrchestratorPackageResourceSnapshot[]): OrchestratorPackageResourceSnapshot[] {
  return [...list].sort((a, b) => {
    const wa = APPLY_ORDER[a.resource_kind] ?? 999;
    const wb = APPLY_ORDER[b.resource_kind] ?? 999;
    if (wa !== wb) return wa - wb;
    if (a.component !== b.component) return a.component.localeCompare(b.component);
    return a.resource_name.localeCompare(b.resource_name);
  });
}

function findManifestIdForPackageResource(
  targetEnvId: string,
  resource: OrchestratorPackageResourceSnapshot
): string | null {
  const found = manifests.value.find(
    (m) =>
      m.env_id === targetEnvId &&
      m.component === resource.component &&
      m.resource_kind === resource.resource_kind &&
      m.resource_name === resource.resource_name &&
      (m.resource_namespace ?? null) === (resource.resource_namespace ?? null)
  );
  return found?.id ?? null;
}

async function onSyncPackageToEnv() {
  if (!selectedPackage.value || !selectedPackageVersion.value || !canOperatePackageDeploy.value) return;
  const target = environments.value.find((e) => e.id === packageTargetEnvId.value);
  if (!target) return;
  const precheckErrors = validatePackageVersion(selectedPackageVersion.value);
  if (precheckErrors.length) {
    opError.value = `预检失败：${precheckErrors.join("；")}`;
    return;
  }
  packageWorking.value = true;
  opError.value = null;
  opMessage.value = null;
  try {
    const result = syncPackageVersionToEnv(
      selectedPackage.value.id,
      selectedPackageVersion.value.id,
      target.id,
      target.display_name,
      packageOverwrite.value
    );
    recordPackageDeployment(
      selectedPackage.value.id,
      selectedPackageVersion.value.id,
      target.id,
      target.display_name,
      "sync",
      result.copied + result.updated,
      0,
      []
    );
    opMessage.value = `已同步到 ${target.display_name}：新增 ${result.copied}，更新 ${result.updated}，跳过 ${result.skipped}`;
  } catch (e) {
    opError.value = e instanceof Error ? e.message : String(e);
  } finally {
    packageWorking.value = false;
  }
}

async function onApplyPackageToEnv() {
  if (!selectedPackage.value || !selectedPackageVersion.value || !canOperatePackageDeploy.value) return;
  const target = environments.value.find((e) => e.id === packageTargetEnvId.value);
  if (!target) return;
  const precheckErrors = validatePackageVersion(selectedPackageVersion.value);
  if (precheckErrors.length) {
    opError.value = `预检失败：${precheckErrors.join("；")}`;
    return;
  }
  packageWorking.value = true;
  opError.value = null;
  opMessage.value = null;
  const version = selectedPackageVersion.value;
  try {
    syncPackageVersionToEnv(
      selectedPackage.value.id,
      version.id,
      target.id,
      target.display_name,
      packageOverwrite.value
    );
    const resources = sortPackageResources(version.resources);
    const errors: string[] = [];
    let success = 0;
    for (const r of resources) {
      try {
        await deployYamlToEnv(target.id, r.yaml);
        const mid = findManifestIdForPackageResource(target.id, r);
        if (mid) saveManifestYaml(mid, r.yaml, "apply");
        success += 1;
      } catch (e) {
        errors.push(`${r.component}/${r.resource_kind}/${r.resource_name}: ${e instanceof Error ? e.message : String(e)}`);
      }
    }
    recordPackageDeployment(
      selectedPackage.value.id,
      version.id,
      target.id,
      target.display_name,
      "apply",
      success,
      errors.length,
      errors
    );
    if (errors.length) {
      opError.value = `应用包部分失败：${errors.join("；")}`;
    } else {
      opMessage.value = `应用包 ${selectedPackage.value.name}@${version.label} 已发布到 ${target.display_name}。`;
    }
  } catch (e) {
    opError.value = e instanceof Error ? e.message : String(e);
  } finally {
    packageWorking.value = false;
  }
}

async function onConfirmPackageAction() {
  if (!canOperatePackageDeploy.value || packageWorking.value) return;
  packageActionDialogVisible.value = false;
  if (packageActionMode.value === "sync") {
    await onSyncPackageToEnv();
    return;
  }
  await onApplyPackageToEnv();
}
</script>

<template>
  <div class="orchestrator-layout">
    <header class="toolbar">
      <span class="title">编排中心</span>
      <div class="view-switch">
        <button
          type="button"
          class="switch-btn"
          :class="{ active: activeView === 'resources' }"
          @click="activeView = 'resources'"
        >
          资源
        </button>
        <button
          type="button"
          class="switch-btn"
          :class="{ active: activeView === 'packages' }"
          @click="activeView = 'packages'"
        >
          应用包
        </button>
      </div>
      <select v-model="selectedEnvId" class="select">
        <option value="" disabled>选择环境</option>
        <option v-for="env in environments" :key="env.id" :value="env.id">
          {{ env.display_name }}
        </option>
      </select>
      <span v-if="activeView === 'resources'" class="hint">
        组件：{{ selectedComponent || "-" }}（{{ manifestsByComponent.length }} 资源）
      </span>
      <button
        v-if="activeView === 'resources'"
        type="button"
        class="btn btn-create"
        :disabled="!selectedEnvId"
        @click="onCreateManifest"
      >
        新建资源
      </button>
      <button v-if="activeView === 'resources'" type="button" class="btn btn-save" :disabled="!selectedManifestId" @click="onSaveYaml">保存</button>
      <button
        v-if="activeView === 'resources'"
        type="button"
        class="btn btn-primary"
        :disabled="!canOpenApplyDialog || applying"
        @click="openApplyDialog"
      >
        {{ applying ? "应用中…" : "应用" }}
      </button>
      <button
        v-if="activeView === 'resources'"
        type="button"
        class="btn btn-diff"
        :disabled="!selectedManifestId || diffLoading"
        @click="loadDiff"
      >
        {{ diffLoading ? "生成中…" : "查看差异" }}
      </button>
      <button
        v-if="activeView === 'resources'"
        type="button"
        class="btn btn-copy"
        :disabled="!canOpenCopyDialog || copyLoading"
        @click="openCopyDialog"
      >
        {{ copyLoading ? "复制中…" : "复制到环境" }}
      </button>
    </header>

    <div v-if="activeView === 'resources'" class="body">
      <aside class="list">
        <div v-if="!selectedEnvId" class="empty">请先选择环境</div>
        <template v-else>
          <div class="component-switcher">
            <div class="component-switcher-head">
              <span>组件切换</span>
              <small v-if="manifestDraftCount > 0">未保存草稿 {{ manifestDraftCount }}</small>
            </div>
            <input
              v-model="componentFilterKeyword"
              type="text"
              class="component-search"
              placeholder="搜索组件名称"
            />
            <div class="component-list">
              <button
                v-for="item in filteredComponentItems"
                :key="item.name"
                type="button"
                class="component-item"
                :class="{ active: selectedComponent === item.name }"
                @click="onSelectComponent(item.name)"
              >
                <span class="component-item-name">{{ item.name }}</span>
                <div class="component-item-actions">
                  <small>{{ item.count }}</small>
                  <button
                    type="button"
                    class="card-delete-btn"
                    title="删除应用组件"
                    aria-label="删除应用组件"
                    @click.stop="openDeleteComponentDialog(item.name, item.count)"
                  >
                    ❌
                  </button>
                </div>
              </button>
              <div v-if="!filteredComponentItems.length" class="empty">没有匹配的组件</div>
            </div>
          </div>

          <div class="list-title">
            <span>资源列表</span>
            <small>{{ selectedComponent || "-" }}</small>
          </div>
          <div class="resource-list-panel">
            <div
              v-for="m in manifestsByComponent"
              :key="m.id"
              class="item"
              :class="{ active: selectedManifestId === m.id }"
              @click="onSelectManifest(m.id)"
            >
              <div class="item-title">
                <span>
                  {{ m.resource_kind }} / {{ m.resource_name }}
                  <strong v-if="hasManifestDraft(m.id)" class="draft-tag">草稿</strong>
                </span>
                <button
                  type="button"
                  class="card-delete-btn"
                  title="删除资源"
                  aria-label="删除资源"
                  @click.stop="openDeleteResourceDialog(m)"
                >
                  ❌
                </button>
              </div>
              <div class="item-sub">{{ m.resource_namespace || "default" }}</div>
            </div>
            <div v-if="!manifestsByComponent.length" class="empty">当前组件暂无资源</div>
          </div>
        </template>
      </aside>

      <section class="editor-panel">
        <div v-if="selectedManifest" class="meta-row">
          <div class="meta-component-editor">
            <div class="meta-field">
              <span>当前所属组件</span>
              <strong class="meta-component-name">{{ selectedManifest.component }}</strong>
              <button type="button" class="btn btn-move-component" @click="openComponentAssignDialog">变更组件</button>
            </div>
          </div>
          <div class="meta-actions">
            <span class="hint">资源：{{ selectedManifest.resource_kind }}/{{ selectedManifest.resource_name }}</span>
          </div>
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

    <div v-else class="pkg-layout">
      <aside class="pkg-side">
        <div class="pkg-panel pkg-create-panel">
          <div class="pkg-side-title">应用包管理</div>
          <div class="pkg-create">
            <input v-model="packageNameInput" type="text" class="pkg-input" placeholder="新应用包名称" />
            <input v-model="packageDescriptionInput" type="text" class="pkg-input" placeholder="描述（可选）" />
            <button
              type="button"
              class="btn btn-package-create"
              :disabled="!packageNameInput.trim()"
              @click="onCreatePackage"
            >
              创建应用包
            </button>
          </div>
          <div class="pkg-side-subtitle">共 {{ packages.length }} 个应用包</div>
        </div>
        <div class="pkg-panel pkg-list-panel">
          <div class="pkg-side-title">应用包列表</div>
          <div
            v-for="pkg in packages"
            :key="pkg.id"
            class="pkg-item"
            :class="{ active: selectedPackageId === pkg.id }"
            @click="selectedPackageId = pkg.id"
          >
            <div class="pkg-item-title">{{ pkg.name }}</div>
            <div class="pkg-item-sub">{{ pkg.description || "无描述" }}</div>
            <div class="pkg-item-meta">版本 {{ pkg.versions.length }} · 更新于 {{ formatDateTime(pkg.updated_at) }}</div>
          </div>
          <div v-if="!packages.length" class="empty">暂无应用包</div>
        </div>
      </aside>

      <section class="pkg-main">
        <template v-if="selectedPackage">
          <div class="pkg-head pkg-panel">
            <div>
              <div class="pkg-name">{{ selectedPackage.name }}</div>
              <div class="pkg-desc">{{ selectedPackage.description || "用于将多个组件打包后按版本发布到环境。" }}</div>
              <div class="pkg-summary-row">
                <span class="pkg-summary-pill">版本 {{ selectedPackageStats.versions }}</span>
                <span class="pkg-summary-pill">资源快照 {{ selectedPackageStats.resources }}</span>
                <span class="pkg-summary-pill">更新于 {{ formatDateTime(selectedPackage.updated_at) }}</span>
              </div>
            </div>
            <button type="button" class="btn btn-danger" @click="openDeletePackageDialog">删除应用包</button>
          </div>

          <div class="pkg-main-grid">
            <div class="pkg-compose pkg-panel">
              <div class="pkg-block-title">版本构建（来源：当前环境）</div>
              <div class="pkg-comp-list">
                <label v-for="item in packageDraftComponents" :key="item.name" class="pkg-check">
                  <input v-model="packageComponentDraft" type="checkbox" :value="item.name" />
                  <span>{{ item.name }}</span>
                  <small>{{ item.count }} 资源</small>
                </label>
              </div>
              <button
                type="button"
                class="btn btn-package-version"
                :disabled="!canCreatePackageVersion"
                @click="onCreatePackageVersion"
              >
                生成新版本
              </button>
            </div>

            <div class="pkg-versions pkg-panel">
              <div class="pkg-block-title">版本列表</div>
              <div class="pkg-version-list">
                <div
                  v-for="v in selectedPackage.versions"
                  :key="v.id"
                  class="pkg-version-item"
                  :class="{ active: selectedPackageVersionId === v.id }"
                  @click="selectedPackageVersionId = v.id"
                >
                  <div class="pkg-version-title-row">
                    <span>{{ v.label }}</span>
                    <button
                      v-if="editingVersionTagId !== v.id"
                      type="button"
                      class="version-tag-edit-btn"
                      :title="v.tag ? '编辑 Tag' : '设置 Tag'"
                      :aria-label="v.tag ? '编辑 Tag' : '设置 Tag'"
                      @click.stop="startEditVersionTag(v)"
                    >
                      <span aria-hidden="true">🏷</span>
                    </button>
                  </div>
                  <div v-if="editingVersionTagId === v.id" class="version-inline-edit-row" @click.stop>
                    <input
                      v-model="editingVersionTagValue"
                      type="text"
                      class="pkg-input version-inline-input"
                      placeholder="输入正式 Tag，例如 prod-20260318"
                    />
                    <button type="button" class="btn btn-save version-inline-btn" @click.stop="onSaveVersionTag(v.id)">保存</button>
                    <button type="button" class="btn version-inline-btn" @click.stop="cancelEditVersionTag">取消</button>
                  </div>
                  <div v-else class="version-tag-display">
                    <strong v-if="v.tag" class="version-tag">#{{ v.tag }}</strong>
                    <span v-else class="version-tag-empty">未设置 Tag</span>
                  </div>
                  <small>组件数 {{ v.component_names.length }} · 资源数 {{ v.resources.length }}</small>
                  <small>{{ v.component_names.join(" / ") }}</small>
                  <small>创建于 {{ formatDateTime(v.created_at) }}</small>
                </div>
                <div v-if="!selectedPackage.versions.length" class="empty">还没有版本，先选择组件生成一个版本。</div>
              </div>
            </div>
          </div>
        </template>
        <div v-else class="empty pkg-empty-card">请先创建或选择应用包</div>
      </section>

      <aside class="pkg-deploy">
        <template v-if="selectedPackageVersion">
          <div class="pkg-panel pkg-action-panel">
            <div class="pkg-block-title">发布操作</div>
            <div class="pkg-version-meta">
              <strong>{{ selectedPackage?.name }} @ {{ selectedPackageVersion.label }}</strong>
              <span v-if="selectedPackageVersion.tag">正式 Tag：#{{ selectedPackageVersion.tag }}</span>
              <span>来源环境：{{ selectedPackageVersion.source_env_name }}</span>
              <span>组件数：{{ selectedPackageVersion.component_names.length }}</span>
              <span>资源数：{{ selectedPackageVersion.resources.length }}</span>
              <span>组件：{{ selectedPackageVersion.component_names.join(" / ") }}</span>
            </div>
            <div class="pkg-action-row">
              <button
                type="button"
                class="btn"
                :disabled="!canOpenPackageActionDialog || packageWorking"
                @click="openPackageActionDialog('sync')"
              >
                {{ packageWorking ? "处理中…" : "同步到环境…" }}
              </button>
              <button
                type="button"
                class="btn btn-primary"
                :disabled="!canOpenPackageActionDialog || packageWorking"
                @click="openPackageActionDialog('apply')"
              >
                {{ packageWorking ? "发布中…" : "发布到环境…" }}
              </button>
            </div>
            <div class="copy-tip">点击按钮后会弹出确认窗口，选择目标环境后再执行。</div>
            <button type="button" class="btn btn-danger" :disabled="packageWorking" @click="openDeleteVersionDialog">
              删除当前版本
            </button>
          </div>

          <div class="pkg-panel">
            <div class="pkg-block-title">资源清单</div>
            <div class="pkg-resource-list">
              <div v-for="r in selectedPackageVersion.resources" :key="r.id" class="pkg-resource-item">
                <span>{{ r.component }}</span>
                <span>{{ r.resource_kind }}/{{ r.resource_name }}</span>
                <small>{{ r.resource_namespace || "default" }}</small>
              </div>
            </div>
          </div>

          <div class="pkg-panel">
            <div class="pkg-block-title">发布记录</div>
            <div class="pkg-deploy-history">
              <div v-for="d in packageDeployments" :key="d.id" class="pkg-deploy-item">
                <span>{{ d.mode === "apply" ? "发布" : "同步" }} · {{ d.version_label }}</span>
                <small>{{ d.target_env_name }} · 成功 {{ d.success }} / 失败 {{ d.failed }}</small>
                <small>{{ formatDateTime(d.at) }}</small>
              </div>
              <div v-if="!packageDeployments.length" class="empty">暂无发布记录</div>
            </div>
          </div>
        </template>
        <div v-else class="empty pkg-empty-card">请选择应用包版本</div>
      </aside>
    </div>

    <Teleport to="body">
      <div v-if="packageActionDialogVisible" class="apply-modal-overlay" @click.self="closePackageActionDialog">
        <section class="apply-modal" role="dialog" aria-label="应用包发布确认">
          <header class="apply-head">
            <h3>{{ packageActionMode === "apply" ? "发布到环境" : "同步到环境" }}</h3>
          </header>
          <div class="apply-body">
            <div class="pkg-version-meta">
              <strong>{{ selectedPackage?.name }} @ {{ selectedPackageVersion?.label }}</strong>
              <span>组件数：{{ selectedPackageVersion?.component_names.length ?? 0 }}</span>
              <span>资源数：{{ selectedPackageVersion?.resources.length ?? 0 }}</span>
            </div>
            <label class="field-label">
              <span>目标环境</span>
              <select v-model="packageTargetEnvId" class="select copy-select" :disabled="packageWorking">
                <option value="" disabled>选择目标环境</option>
                <option v-for="env in environments" :key="env.id" :value="env.id">
                  {{ env.display_name }}
                </option>
              </select>
            </label>
            <label class="field-check">
              <input v-model="packageOverwrite" type="checkbox" :disabled="packageWorking" />
              覆盖同名资源
            </label>
            <div class="copy-tip">
              {{ packageActionMode === "apply" ? "将先同步编排资产，再按顺序发布到集群。" : "仅同步到编排资产，不会直接写入集群。" }}
            </div>
          </div>
          <footer class="apply-foot">
            <button type="button" class="btn" :disabled="packageWorking" @click="closePackageActionDialog">取消</button>
            <button
              type="button"
              class="btn btn-primary"
              :disabled="!canOperatePackageDeploy || packageWorking"
              @click="onConfirmPackageAction"
            >
              {{ packageWorking ? "处理中…" : packageActionMode === "apply" ? "确认发布" : "确认同步" }}
            </button>
          </footer>
        </section>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-if="packageDeleteDialogVisible" class="apply-modal-overlay" @click.self="closeDeletePackageDialog">
        <section class="apply-modal" role="dialog" aria-label="删除应用包确认">
          <header class="apply-head">
            <h3>确认删除应用包</h3>
          </header>
          <div class="apply-body">
            <div class="copy-tip">
              将删除应用包 <strong>{{ selectedPackage?.name }}</strong>，以及该应用包下所有版本和发布记录。
            </div>
          </div>
          <footer class="apply-foot">
            <button type="button" class="btn" :disabled="packageWorking" @click="closeDeletePackageDialog">取消</button>
            <button type="button" class="btn btn-danger" :disabled="packageWorking" @click="onDeletePackage">确认删除</button>
          </footer>
        </section>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-if="versionDeleteDialogVisible" class="apply-modal-overlay" @click.self="closeDeleteVersionDialog">
        <section class="apply-modal" role="dialog" aria-label="删除版本确认">
          <header class="apply-head">
            <h3>确认删除版本</h3>
          </header>
          <div class="apply-body">
            <div class="copy-tip">
              将删除版本 <strong>{{ selectedPackageVersion?.label }}</strong>
              <span v-if="selectedPackageVersion?.tag">（#{{ selectedPackageVersion?.tag }}）</span>
              及其发布记录。
            </div>
          </div>
          <footer class="apply-foot">
            <button type="button" class="btn" :disabled="packageWorking" @click="closeDeleteVersionDialog">取消</button>
            <button type="button" class="btn btn-danger" :disabled="packageWorking" @click="onDeletePackageVersion">确认删除</button>
          </footer>
        </section>
      </div>
    </Teleport>

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
    <Teleport to="body">
      <div v-if="componentAssignDialogVisible" class="apply-modal-overlay" @click.self="closeComponentAssignDialog">
        <section class="apply-modal" role="dialog" aria-label="变更组件归属">
          <header class="apply-head">
            <h3>变更组件归属</h3>
          </header>
          <div class="apply-body">
            <div class="copy-tip">
              当前资源：<strong>{{ selectedManifest?.resource_kind }}/{{ selectedManifest?.resource_name }}</strong>
            </div>
            <label class="assign-mode">
              <input
                v-model="componentAssignMode"
                type="radio"
                value="existing"
                :disabled="componentOptionsForAssign.length === 0"
              />
              加入已有组件
            </label>
            <label class="assign-mode">
              <input v-model="componentAssignMode" type="radio" value="new" />
              新建组件
            </label>
            <select v-if="componentAssignMode === 'existing'" v-model="componentAssignExisting" class="select assign-select">
              <option value="" disabled>选择目标组件</option>
              <option v-for="name in componentOptionsForAssign" :key="name" :value="name">{{ name }}</option>
            </select>
            <input
              v-else
              v-model="componentAssignNew"
              type="text"
              class="assign-input"
              placeholder="输入新组件名称"
            />
          </div>
          <footer class="apply-foot">
            <button type="button" class="btn" @click="closeComponentAssignDialog">取消</button>
            <button
              type="button"
              class="btn btn-primary"
              :disabled="componentAssignMode === 'existing' ? !componentAssignExisting : !componentAssignNew.trim()"
              @click="applyManifestComponentAssignment"
            >
              确认变更
            </button>
          </footer>
        </section>
      </div>
    </Teleport>
    <Teleport to="body">
      <div v-if="listDeleteDialogVisible" class="apply-modal-overlay" @click.self="closeListDeleteDialog">
        <section class="apply-modal" role="dialog" aria-label="删除确认">
          <header class="apply-head">
            <h3>{{ listContextTarget?.type === "component" ? "确认删除应用组件" : "确认删除资源" }}</h3>
          </header>
          <div class="apply-body">
            <div v-if="listContextTarget?.type === 'component'" class="copy-tip">
              将删除应用组件 <strong>{{ listContextTarget.component }}</strong>，以及该组件下全部资源（{{ listContextTarget.count }} 个）。
            </div>
            <div v-else class="copy-tip">
              将删除资源 <strong>{{ listContextTarget?.type === "resource" ? listContextTarget.label : "" }}</strong>。
            </div>
          </div>
          <footer class="apply-foot">
            <button type="button" class="btn" @click="closeListDeleteDialog">取消</button>
            <button type="button" class="btn btn-danger" @click="confirmDeleteFromContextMenu">确认删除</button>
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
.view-switch {
  display: inline-flex;
  border: 1px solid #cbd5e1;
  border-radius: 8px;
  overflow: hidden;
}
.switch-btn {
  height: 1.9rem;
  border: none;
  border-right: 1px solid #cbd5e1;
  background: #f8fafc;
  color: #475569;
  font-size: 0.78rem;
  padding: 0 0.6rem;
  cursor: pointer;
}
.switch-btn:last-child {
  border-right: none;
}
.switch-btn.active {
  background: #2563eb;
  color: #fff;
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
.btn-move-component {
  border-color: #0284c7;
  background: #0284c7;
  color: #fff;
}
.btn-move-component:hover:not(:disabled) {
  border-color: #0369a1;
  background: #0369a1;
}
.btn-move-component:disabled {
  border-color: #bae6fd;
  background: #e0f2fe;
  color: #f0f9ff;
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
.component-switcher {
  border: 1px solid #bfdbfe;
  border-radius: 8px;
  background: #eff6ff;
  padding: 0.45rem;
  margin-bottom: 0.5rem;
}
.component-switcher-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.35rem;
  font-size: 0.76rem;
  color: #334155;
}
.component-switcher-head small {
  color: #0f766e;
}
.component-search {
  width: 100%;
  height: 1.8rem;
  border: 1px solid #cbd5e1;
  border-radius: 6px;
  padding: 0 0.45rem;
  font-size: 0.75rem;
  margin-bottom: 0.4rem;
}
.component-list {
  display: grid;
  gap: 0.25rem;
  max-height: 180px;
  overflow: auto;
}
.component-item {
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  background: #fff;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.25rem 0.4rem;
  font-size: 0.74rem;
  color: #0f172a;
  cursor: pointer;
}
.component-item-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.component-item-actions {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
}
.component-item small {
  color: #64748b;
}
.component-item.active {
  border-color: #2563eb;
  background: #eff6ff;
}
.list-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 0.76rem;
  color: #475569;
  margin: 0.3rem 0 0.4rem;
}
.list-title small {
  color: #64748b;
}
.resource-list-panel {
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  background: #ffffff;
  padding: 0.35rem;
}
.item {
  padding: 0.45rem 0.5rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  margin-bottom: 0.4rem;
  cursor: pointer;
  background: #fff;
}
.item.active {
  border-color: #2563eb;
  background: #eff6ff;
}
.item-title {
  font-size: 0.8125rem;
  font-weight: 600;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.35rem;
}
.card-delete-btn {
  width: 1.2rem;
  height: 1.2rem;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: #ef4444;
  font-size: 0.62rem;
  line-height: 1;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  opacity: 0.65;
}
.card-delete-btn:hover {
  border-color: #fecaca;
  background: #fef2f2;
  opacity: 0.95;
}
.draft-tag {
  margin-left: 0.35rem;
  border: 1px solid #f59e0b;
  background: #fffbeb;
  color: #92400e;
  border-radius: 999px;
  font-size: 0.66rem;
  padding: 0.05rem 0.35rem;
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
.meta-component-editor {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
}
.meta-component-name {
  color: #0f766e;
  font-size: 0.82rem;
}
.component-assign-row {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  flex-wrap: wrap;
}
.assign-mode {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.75rem;
  color: #334155;
}
.assign-select,
.assign-input {
  height: 1.8rem;
  border: 1px solid #d1d5db;
  border-radius: 6px;
  padding: 0 0.45rem;
  font-size: 0.76rem;
}
.assign-select {
  min-width: 160px;
}
.assign-input {
  min-width: 180px;
}
.hint {
  font-size: 0.75rem;
  color: #64748b;
}
.meta-actions {
  display: inline-flex;
  align-items: center;
  gap: 0.55rem;
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
.btn-package-create {
  border-color: #67e8f9;
  background: #cffafe;
  color: #0f172a;
}
.btn-package-create:hover:not(:disabled) {
  border-color: #22d3ee;
  background: #a5f3fc;
}
.btn-package-create:disabled {
  border-color: #e2e8f0;
  background: #f8fafc;
  color: #94a3b8;
}
.btn-package-version {
  border-color: #86efac;
  background: #dcfce7;
  color: #0f172a;
}
.btn-package-version:hover:not(:disabled) {
  border-color: #4ade80;
  background: #bbf7d0;
}
.btn-package-version:disabled {
  border-color: #dcfce7;
  background: #f0fdf4;
  color: #94a3b8;
}
.pkg-layout {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 300px 1fr 360px;
  background: #f8fafc;
}
.pkg-side,
.pkg-main,
.pkg-deploy {
  min-height: 0;
  overflow: auto;
  padding: 0.75rem;
}
.pkg-side {
  border-right: 1px solid #e2e8f0;
}
.pkg-main {
  border-right: 1px solid #e2e8f0;
}
.pkg-panel {
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  background: #fff;
  padding: 0.7rem;
}
.pkg-side-title,
.pkg-block-title {
  font-size: 0.83rem;
  font-weight: 600;
  color: #1e293b;
  margin-bottom: 0.45rem;
}
.pkg-side-subtitle {
  margin-top: 0.5rem;
  font-size: 0.75rem;
  color: #64748b;
}
.pkg-create-panel {
  margin-bottom: 0.65rem;
}
.pkg-list-panel {
  min-height: 0;
}
.pkg-create {
  display: grid;
  gap: 0.45rem;
}
.pkg-input {
  height: 2rem;
  border: 1px solid #d1d5db;
  border-radius: 6px;
  padding: 0 0.5rem;
}
.pkg-item {
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  background: #fff;
  padding: 0.45rem 0.55rem;
  margin-bottom: 0.45rem;
  cursor: pointer;
  transition: border-color 0.15s ease, background 0.15s ease;
}
.pkg-item:hover {
  border-color: #cbd5e1;
  background: #f8fafc;
}
.pkg-item.active {
  border-color: #2563eb;
  background: #eff6ff;
}
.pkg-item-title {
  font-size: 0.82rem;
  font-weight: 600;
}
.pkg-item-sub {
  font-size: 0.74rem;
  color: #64748b;
}
.pkg-item-meta {
  margin-top: 0.25rem;
  font-size: 0.7rem;
  color: #94a3b8;
}
.pkg-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.75rem;
  margin-bottom: 0.65rem;
}
.pkg-name {
  font-size: 0.95rem;
  font-weight: 700;
  color: #0f172a;
}
.pkg-desc {
  font-size: 0.78rem;
  color: #64748b;
  margin-top: 0.25rem;
}
.pkg-summary-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
  margin-top: 0.5rem;
}
.pkg-summary-pill {
  font-size: 0.72rem;
  color: #0f172a;
  border: 1px solid #cbd5e1;
  background: #f8fafc;
  border-radius: 999px;
  padding: 0.12rem 0.45rem;
}
.pkg-main-grid {
  display: grid;
  grid-template-columns: minmax(260px, 360px) 1fr;
  gap: 0.65rem;
}
.pkg-compose,
.pkg-versions {
  min-height: 0;
}
.pkg-comp-list {
  display: grid;
  gap: 0.35rem;
  margin-bottom: 0.5rem;
}
.pkg-check {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.8rem;
}
.pkg-check small {
  color: #64748b;
}
.pkg-version-list {
  display: grid;
  gap: 0.4rem;
  max-height: 360px;
  overflow: auto;
}
.pkg-version-item {
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  background: #f8fafc;
  text-align: left;
  padding: 0.5rem 0.6rem;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}
.pkg-version-item.active {
  border-color: #2563eb;
  background: #eff6ff;
}
.pkg-version-item small {
  color: #64748b;
}
.pkg-version-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}
.version-tag-edit-btn {
  border: 1px solid #cbd5e1;
  border-radius: 999px;
  background: #fff;
  color: #334155;
  width: 1.65rem;
  height: 1.65rem;
  font-size: 0.78rem;
  padding: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  line-height: 1;
  cursor: pointer;
}
.version-tag-edit-btn:hover {
  border-color: #94a3b8;
  background: #f8fafc;
}
.version-inline-edit-row {
  display: flex;
  align-items: center;
  gap: 0.35rem;
}
.version-inline-input {
  flex: 1;
  min-width: 180px;
}
.version-inline-btn {
  height: 1.8rem;
  padding: 0 0.5rem;
  font-size: 0.72rem;
}
.version-tag-display {
  min-height: 1.1rem;
}
.version-tag {
  display: inline-flex;
  align-items: center;
  color: #0f766e;
  background: #ecfeff;
  border: 1px solid #99f6e4;
  border-radius: 999px;
  font-size: 0.7rem;
  padding: 0.05rem 0.4rem;
}
.version-tag-empty {
  color: #94a3b8;
  font-size: 0.72rem;
}
.pkg-version-meta {
  display: grid;
  gap: 0.2rem;
  font-size: 0.78rem;
  color: #334155;
  margin-bottom: 0.65rem;
}
.pkg-action-panel {
  position: sticky;
  top: 0;
  z-index: 1;
  margin-bottom: 0.65rem;
}
.pkg-action-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.45rem;
  margin-bottom: 0.45rem;
}
.pkg-resource-list,
.pkg-deploy-history {
  max-height: 280px;
  overflow: auto;
  display: grid;
  gap: 0.35rem;
}
.pkg-resource-item,
.pkg-deploy-item {
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  background: #fff;
  padding: 0.4rem 0.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
  font-size: 0.75rem;
}
.pkg-resource-item small,
.pkg-deploy-item small {
  color: #64748b;
}
.pkg-empty-card {
  border: 1px dashed #cbd5e1;
  border-radius: 10px;
  background: #fff;
  padding: 0.9rem;
}
@media (max-width: 1480px) {
  .pkg-layout {
    grid-template-columns: 280px 1fr 320px;
  }
}
@media (max-width: 1280px) {
  .pkg-layout {
    grid-template-columns: 280px 1fr;
  }
  .pkg-deploy {
    border-top: 1px solid #e2e8f0;
    border-left: none;
    grid-column: 1 / -1;
  }
  .pkg-main-grid {
    grid-template-columns: 1fr;
  }
}
</style>
