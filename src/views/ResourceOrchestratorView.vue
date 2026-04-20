<script setup lang="ts">
import { computed, ref, watch } from "vue";

defineOptions({ name: "ResourceOrchestratorView" });
import * as jsYaml from "js-yaml";
import { CodeEditor } from "monaco-editor-vue3";
import { kubeDeployResource } from "../api/kube";
import { appSettingsGetResourceDeployStrategy } from "../api/config";
import { createResourceSnapshot, summarizeResourceYaml, type ResourceSnapshotItem } from "../stores/resourceSnapshots";
import { ensureAutoSnapshotSettingLoaded } from "../stores/appSettings";
import { extractErrorMessage } from "../utils/errorMessage";
import { parseKubeObject, type KubeObjectIdentity } from "../utils/yaml";
import {
  buildDiffRows,
  normalizeYamlForDiff,
  type DiffRow,
} from "../features/orchestrator/yamlDiff";
import { kubeGetResource } from "../api/kube";
import {
  useOrchestratorApplyFlow,
} from "../features/orchestrator/useOrchestratorApplyFlow";
import { useOrchestratorImportPreview } from "../features/orchestrator/useOrchestratorImportPreview";
import { useEnvStore } from "../stores/env";
import { useYamlMonacoTheme } from "../stores/yamlTheme";
import {
  useOrchestratorStore,
  type OrchestratorImportBatch,
  type OrchestratorManifest,
} from "../stores/orchestrator";
import OrchestratorPackageView from "../components/orchestrator/OrchestratorPackageView.vue";
import OrchestratorCopyDialog from "../components/orchestrator/OrchestratorCopyDialog.vue";
import OrchestratorDiffModal from "../components/orchestrator/OrchestratorDiffModal.vue";
import ResourceSnapshotViewer from "../components/ResourceSnapshotViewer.vue";
import type { ManifestHistoryItem } from "../stores/orchestrator";

const APPLY_ORDER: Record<string, number> = {
  CustomResourceDefinition: 0,
  Namespace: 1,
  PriorityClass: 2,
  StorageClass: 3,
  IngressClass: 4,
  ServiceAccount: 10,
  Role: 11,
  ClusterRole: 12,
  RoleBinding: 13,
  ClusterRoleBinding: 14,
  ConfigMap: 20,
  Secret: 21,
  PersistentVolume: 22,
  PersistentVolumeClaim: 23,
  ResourceQuota: 24,
  LimitRange: 25,
  MutatingWebhookConfiguration: 26,
  ValidatingWebhookConfiguration: 27,
  NetworkPolicy: 28,
  Service: 30,
  Endpoints: 31,
  EndpointSlice: 32,
  Deployment: 40,
  StatefulSet: 41,
  DaemonSet: 42,
  ReplicaSet: 43,
  Job: 44,
  CronJob: 45,
  Pod: 46,
  Ingress: 50,
  HorizontalPodAutoscaler: 51,
  PodDisruptionBudget: 52,
};

const { environments, currentId } = useEnvStore();
const {
  manifests,
  importBatches,
  orchestratorFocusTarget,
  saveManifestYaml,
  setManifestIdentity,
  setManifestComponent,
  deleteManifest,
  importManifestsToEnv,
} = useOrchestratorStore();
const { monacoTheme } = useYamlMonacoTheme();

const activeView = ref<"resources" | "packages">("resources");
const resourceGroupView = ref<"component" | "file" | "batch">("component");
const selectedEnvId = ref<string>("");
const selectedComponent = ref<string>("");
const selectedSourceFile = ref<string>("");
const selectedBatchId = ref<string>("");
const selectedManifestId = ref<string>("");
const editYaml = ref("");
const componentFilterKeyword = ref("");
const validationErrors = ref<string[]>([]);
const validationWarnings = ref<string[]>([]);
const opMessage = ref<string | null>(null);
const opError = ref<string | null>(null);
const viewingHistoryItem = ref<ManifestHistoryItem | null>(null);
const createYamlActive = ref(false);
const copyDialogVisible = ref(false);
const diffVisible = ref(false);
const diffLoading = ref(false);
const diffNotFound = ref(false);
const diffRows = ref<DiffRow[]>([]);
const selectedManifestByComponent = ref<Record<string, string>>({});
const componentAssignMode = ref<"existing" | "new">("existing");
const componentAssignExisting = ref("");
const componentAssignNew = ref("");
const componentAssignDialogVisible = ref(false);
const listDeleteDialogVisible = ref(false);
const manifestDraftCache = ref<Record<string, string>>({});
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
const selectedEnvironment = computed(() =>
  environments.value.find((env) => env.id === selectedEnvId.value) ?? null
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
const sourceFileItems = computed(() => {
  const fileMap = new Map<string, number>();
  for (const manifest of manifestsByEnv.value) {
    const fileName = manifest.source_file_name?.trim();
    if (!fileName) continue;
    fileMap.set(fileName, (fileMap.get(fileName) ?? 0) + 1);
  }
  return Array.from(fileMap.entries())
    .map(([name, count]) => ({ name, count }))
    .sort((a, b) => a.name.localeCompare(b.name));
});
const batchItems = computed(() =>
  importBatches.value
    .filter((batch) => batch.env_id === selectedEnvId.value)
    .map((batch) => ({
      id: batch.id,
      name: batch.name,
      count: batch.resource_count,
      batch,
    }))
);

const filteredComponentItems = computed(() => {
  const keyword = componentFilterKeyword.value.trim().toLowerCase();
  if (!keyword) return componentItems.value;
  return componentItems.value.filter((item) => item.name.toLowerCase().includes(keyword));
});
const filteredSourceFileItems = computed(() =>
  sourceFileItems.value.filter((entry) =>
    entry.name.toLowerCase().includes(componentFilterKeyword.value.trim().toLowerCase())
  )
);
const filteredBatchItems = computed(() =>
  batchItems.value.filter((entry) =>
    entry.name.toLowerCase().includes(componentFilterKeyword.value.trim().toLowerCase())
  )
);

const manifestsByComponent = computed(() =>
  manifestsByEnv.value.filter((m) => m.component === selectedComponent.value)
);
const manifestsBySourceFile = computed(() =>
  manifestsByEnv.value.filter((m) => (m.source_file_name ?? "") === selectedSourceFile.value)
);
const manifestsByBatch = computed(() =>
  manifestsByEnv.value.filter((m) => (m.source_batch_id ?? "") === selectedBatchId.value)
);
const selectedImportBatch = computed<OrchestratorImportBatch | null>(
  () => importBatches.value.find((batch) => batch.id === selectedBatchId.value) ?? null
);
const activeGroupLabel = computed(() => {
  if (resourceGroupView.value === "file") return selectedSourceFile.value;
  if (resourceGroupView.value === "batch") {
    return selectedImportBatch.value?.name ?? "";
  }
  return selectedComponent.value;
});
const manifestsInActiveGroup = computed(() => {
  if (resourceGroupView.value === "file") return manifestsBySourceFile.value;
  if (resourceGroupView.value === "batch") return manifestsByBatch.value;
  return manifestsByComponent.value;
});
const componentOptionsForAssign = computed(() =>
  components.value.filter((name) => name !== selectedManifest.value?.component)
);
const manifestDraftCount = computed(() =>
  manifestsInActiveGroup.value.filter((m) => manifestDraftCache.value[m.id] && manifestDraftCache.value[m.id] !== m.yaml).length
);
const selectedManifest = computed<OrchestratorManifest | null>(
  () => manifests.value.find((m) => m.id === selectedManifestId.value) ?? null
);

const selectedHistory = computed(() => selectedManifest.value?.history ?? []);

const ACTION_LABELS: Record<string, string> = { sync: "同步", save: "保存", apply: "应用", restore: "恢复" };

const viewingHistorySnapshot = computed<ResourceSnapshotItem | null>(() => {
  const h = viewingHistoryItem.value;
  const m = selectedManifest.value;
  if (!h || !m) return null;
  return {
    id: h.id,
    created_at: h.at,
    env_id: m.env_id,
    resource_kind: m.resource_kind,
    resource_name: m.resource_name,
    resource_namespace: m.resource_namespace ?? null,
    category: "resource",
    source: "manual",
    pinned: false,
    title: `历史快照 · ${ACTION_LABELS[h.action] ?? h.action}`,
    summary: `${m.resource_kind}/${m.resource_name}`,
    yaml: h.yaml,
  };
});
const componentApplyPlan = computed(() => {
  const list = [...manifestsByComponent.value];
  const delayedWebhookKeys = buildDelayedWebhookKeys(list);
  return list.sort((a, b) => {
    const wa = applyWeight(
      a.resource_kind,
      delayedWebhookKeys.has(resourceIdentityKey(a.resource_kind, a.resource_name, a.resource_namespace))
    );
    const wb = applyWeight(
      b.resource_kind,
      delayedWebhookKeys.has(resourceIdentityKey(b.resource_kind, b.resource_name, b.resource_namespace))
    );
    if (wa !== wb) return wa - wb;
    if ((a.resource_namespace || "") !== (b.resource_namespace || "")) {
      return (a.resource_namespace || "").localeCompare(b.resource_namespace || "");
    }
    return a.resource_name.localeCompare(b.resource_name);
  });
});
const canOpenCopyDialog = computed(
  () => Boolean(selectedEnvId.value && selectedComponent.value && environments.value.length > 1 && resourceGroupView.value === "component")
);
const {
  importLoading,
  importComponent,
  importOverwrite,
  importPreviewItems,
  importSummaryMessage,
  importFileInput,
  importTextDraft,
  importValidItems,
  importInvalidItems,
  importWarningItems,
  canConfirmImport,
  refreshImportPreviewChecks,
  onImportFilesSelected,
  triggerImportFileSelect,
  openCreateYamlDialog: openCreateYamlDialogState,
  clearParseTimer,
  onDraftVisibilityOrContentChange,
  buildImportResources,
  extractRefWarningsFromInventory,
} = useOrchestratorImportPreview({
  selectedEnvId,
  manifestsByEnv,
  parseIdentity,
});
const {
  applying,
  applyDialogVisible,
  componentApplyDialogVisible,
  componentApplyItems,
  componentApplyPhase,
  canApplyCurrent,
  canApplyComponent,
  canOpenApplyDialog,
  componentApplySummary,
  closeComponentApplyDialog,
  openApplyDialog,
  closeApplyDialog,
  onApplyCurrentFromDialog,
  onApplyComponentFromDialog,
  startComponentApplyFromDialog,
} = useOrchestratorApplyFlow({
  selectedEnvId,
  selectedComponent,
  selectedManifestId,
  resourceGroupView,
  selectedManifest,
  componentApplyPlan,
  editYaml,
  validateCurrent,
  parseIdentity,
  deployYamlToEnv,
  setManifestComponent,
  setManifestIdentity,
  saveManifestYaml,
  clearManifestDraft: (manifestId) => {
    delete manifestDraftCache.value[manifestId];
  },
  setOpError: (message) => {
    opError.value = message;
  },
  setOpMessage: (message) => {
    opMessage.value = message;
  },
  fetchLiveYaml: async (envId, kind, name, namespace) => {
    try {
      return await kubeGetResource(envId, kind, name, namespace);
    } catch {
      return null;
    }
  },
  createBeforeApplySnapshot: async (envId, kind, name, namespace, liveYaml) => {
    const autoSnapshotEnabled = await ensureAutoSnapshotSettingLoaded();
    if (!autoSnapshotEnabled) return;
    createResourceSnapshot(
      { env_id: envId, resource_kind: kind, resource_name: name, resource_namespace: namespace },
      {
        yaml: liveYaml,
        category: "resource",
        source: "before-apply",
        title: "应用前资源快照",
        summary: summarizeResourceYaml(liveYaml),
      }
    );
  },
});

function onSelectComponent(componentName: string) {
  if (!componentName || componentName === selectedComponent.value) return;
  selectedComponent.value = componentName;
}

function onSelectSourceFile(fileName: string) {
  if (!fileName || fileName === selectedSourceFile.value) return;
  selectedSourceFile.value = fileName;
}

function onSelectBatch(batchId: string) {
  if (!batchId || batchId === selectedBatchId.value) return;
  selectedBatchId.value = batchId;
}

function onSelectManifest(manifestId: string) {
  createYamlActive.value = false;
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

function resourceIdentityKey(kind: string, name: string, namespace: string | null) {
  return `${kind}|${namespace ?? ""}|${name}`;
}

function isWebhookConfigurationKind(kind: string) {
  return kind === "MutatingWebhookConfiguration" || kind === "ValidatingWebhookConfiguration";
}

function isWorkloadKind(kind: string) {
  return ["Deployment", "StatefulSet", "DaemonSet", "ReplicaSet", "Job", "CronJob", "Pod"].includes(kind);
}

function extractWebhookServiceKeys(yaml: string): string[] {
  const obj = parseKubeObject(yaml)?.raw ?? null;
  if (!obj) return [];
  const webhooks = Array.isArray(obj.webhooks) ? (obj.webhooks as Array<Record<string, unknown>>) : [];
  const keys: string[] = [];
  for (const webhook of webhooks) {
    const clientConfig =
      webhook.clientConfig && typeof webhook.clientConfig === "object"
        ? (webhook.clientConfig as Record<string, unknown>)
        : null;
    const service =
      clientConfig?.service && typeof clientConfig.service === "object"
        ? (clientConfig.service as Record<string, unknown>)
        : null;
    if (!service) continue;
    const name = typeof service.name === "string" ? service.name.trim() : "";
    if (!name) continue;
    const namespace = typeof service.namespace === "string" && service.namespace.trim() ? service.namespace.trim() : "default";
    keys.push(resourceIdentityKey("Service", name, namespace));
  }
  return keys;
}

function buildDelayedWebhookKeys<T extends { resource_kind: string; resource_name: string; resource_namespace: string | null; yaml: string }>(
  list: T[]
): Set<string> {
  const serviceKeys = new Set(
    list
      .filter((item) => item.resource_kind === "Service")
      .map((item) => resourceIdentityKey("Service", item.resource_name, item.resource_namespace || "default"))
  );
  const workloadNamespaces = new Set(
    list
      .filter((item) => isWorkloadKind(item.resource_kind))
      .map((item) => item.resource_namespace || "default")
  );
  const delayed = new Set<string>();
  for (const item of list) {
    if (!isWebhookConfigurationKind(item.resource_kind)) continue;
    const targetServiceKeys = extractWebhookServiceKeys(item.yaml);
    const shouldDelay = targetServiceKeys.some((svcKey) => {
      if (!serviceKeys.has(svcKey)) return false;
      const namespace = svcKey.split("|")[1] || "default";
      return workloadNamespaces.has(namespace);
    });
    if (shouldDelay) {
      delayed.add(resourceIdentityKey(item.resource_kind, item.resource_name, item.resource_namespace));
    }
  }
  return delayed;
}

function applyWeight(kind: string, delayedWebhook = false) {
  if (delayedWebhook && isWebhookConfigurationKind(kind)) return 49;
  return APPLY_ORDER[kind] ?? 999;
}

function openCreateYamlDialog() {
  if (!selectedEnvId.value) return;
  if (openCreateYamlDialogState(selectedComponent.value)) {
    createYamlActive.value = true;
  }
}

function closeCreateYamlDialog() {
  if (importLoading.value) return;
  clearParseTimer();
  createYamlActive.value = false;
}

async function onConfirmImport() {
  if (!selectedEnvId.value) return;
  const env = environments.value.find((item) => item.id === selectedEnvId.value);
  if (!env) return;
  const { component, resources } = buildImportResources(selectedComponent.value);
  if (!resources.length) {
    opError.value = "没有可导入的有效资源。";
    return;
  }

  importLoading.value = true;
  opError.value = null;
  opMessage.value = null;
  try {
    const result = importManifestsToEnv(
      selectedEnvId.value,
      env.display_name,
      resources,
      importOverwrite.value,
      {
        source_kind: "text",
        file_count: 1,
        document_count: importPreviewItems.value.length,
        error_count: importInvalidItems.value.length,
        warning_count: importWarningItems.value.length,
        component,
      }
    );
    selectedComponent.value = component;
    if (result.batchId) {
      selectedBatchId.value = result.batchId;
    }
    if (resources[0]?.source_file_name) selectedSourceFile.value = resources[0].source_file_name;
    if (result.manifestIds.length > 0) {
      onSelectManifest(result.manifestIds[0]);
    }
    createYamlActive.value = false;
    opMessage.value = `保存完成：新增 ${result.created}，更新 ${result.updated}，跳过 ${result.skipped}。`;
  } catch (e) {
    opError.value = extractErrorMessage(e);
  } finally {
    importLoading.value = false;
  }
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
  () => [
    selectedEnvId.value,
    components.value.join("|"),
    sourceFileItems.value.map((item) => item.name).join("|"),
    batchItems.value.map((item) => item.id).join("|"),
  ] as const,
  () => {
    if (!components.value.length) selectedComponent.value = "";
    else if (!selectedComponent.value || !components.value.includes(selectedComponent.value)) {
      selectedComponent.value = components.value[0];
    }
    if (!sourceFileItems.value.length) selectedSourceFile.value = "";
    else if (!selectedSourceFile.value || !sourceFileItems.value.some((item) => item.name === selectedSourceFile.value)) {
      selectedSourceFile.value = sourceFileItems.value[0].name;
    }
    if (!batchItems.value.length) selectedBatchId.value = "";
    else if (!selectedBatchId.value || !batchItems.value.some((item) => item.id === selectedBatchId.value)) {
      selectedBatchId.value = batchItems.value[0].id;
    }
    diffRows.value = [];
    diffVisible.value = false;
  },
  { immediate: true }
);


watch(
  () => [importOverwrite.value, selectedEnvId.value, manifestsByEnv.value.length] as const,
  () => {
    refreshImportPreviewChecks();
  }
);

watch(
  () => [createYamlActive.value, importTextDraft.value] as const,
  ([visible, draft]) => {
    onDraftVisibilityOrContentChange(visible, draft);
  }
);

watch(
  () => [
    resourceGroupView.value,
    selectedComponent.value,
    selectedSourceFile.value,
    selectedBatchId.value,
    manifestsInActiveGroup.value.map((m) => m.id).join(","),
  ] as const,
  () => {
    if (!manifestsInActiveGroup.value.length) {
      selectedManifestId.value = "";
      editYaml.value = "";
      return;
    }
    const rememberedId =
      resourceGroupView.value === "component"
        ? selectedManifestByComponent.value[selectedComponent.value]
        : "";
    if (rememberedId && manifestsInActiveGroup.value.some((m) => m.id === rememberedId)) {
      selectedManifestId.value = rememberedId;
    } else if (!selectedManifestId.value || !manifestsInActiveGroup.value.some((m) => m.id === selectedManifestId.value)) {
      selectedManifestId.value = manifestsInActiveGroup.value[0].id;
    }
    if (resourceGroupView.value === "component" && selectedComponent.value && selectedManifestId.value) {
      selectedManifestByComponent.value[selectedComponent.value] = selectedManifestId.value;
    }
    diffRows.value = [];
    diffVisible.value = false;
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
    diffVisible.value = false;
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

function parseIdentity(yaml: string): KubeObjectIdentity | null {
  return parseKubeObject(yaml);
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
    validationErrors.value.push(`YAML 语法错误：${extractErrorMessage(e)}`);
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

  validationWarnings.value = extractRefWarningsFromInventory(editYaml.value, manifestsInActiveGroup.value);
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

async function loadDiff() {
  if (!selectedManifest.value || !selectedEnvId.value) return;
  diffVisible.value = true;
  diffLoading.value = true;
  diffNotFound.value = false;
  diffRows.value = [];
  try {
    const m = selectedManifest.value;
    const live = await kubeGetResource(selectedEnvId.value, m.resource_kind, m.resource_name, m.resource_namespace);
    diffRows.value = buildDiffRows(
      normalizeYamlForDiff(live),
      normalizeYamlForDiff(editYaml.value),
    );
  } catch (e) {
    const msg = extractErrorMessage(e);
    if (/not found|404|NotFound/i.test(msg)) {
      diffNotFound.value = true;
    } else {
      diffVisible.value = false;
      opError.value = msg;
    }
  } finally {
    diffLoading.value = false;
  }
}

function onViewHistory(item: ManifestHistoryItem) {
  viewingHistoryItem.value = item;
}

</script>

<template>
  <div class="orchestrator-layout">
    <header class="toolbar">
      <div class="toolbar-start">
        <div class="toolbar-brand">
          <span class="title">编排中心</span>
          <span v-if="activeView === 'resources'" class="toolbar-subtitle">
            {{
              createYamlActive
                ? "当前正在编辑新建草稿"
                : `${resourceGroupView === "component" ? "组件" : resourceGroupView === "file" ? "文件" : "批次"}：${activeGroupLabel || "-"}（${manifestsInActiveGroup.length} 资源）`
            }}
          </span>
        </div>
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
        <button
          v-if="activeView === 'resources'"
          type="button"
          class="btn btn-create"
          :disabled="!selectedEnvId"
          @click="openCreateYamlDialog"
        >
          新建资源
        </button>
      </div>
    </header>

    <div v-if="activeView === 'resources'" class="body">
      <aside class="list">
        <div class="component-switcher">
          <label class="env-select-card env-select-card-sidebar">
            <span class="env-select-label">当前环境</span>
            <div class="env-select-main">
              <select v-model="selectedEnvId" class="select env-select">
                <option value="" disabled>选择环境</option>
                <option v-for="env in environments" :key="env.id" :value="env.id">
                  {{ env.display_name }}
                </option>
              </select>
              <div class="env-current-chip" :class="{ empty: !selectedEnvironment }">
                <span class="env-current-dot" />
                <span class="env-current-text">{{ selectedEnvironment?.display_name || "未选择环境" }}</span>
              </div>
            </div>
          </label>
          <div v-if="!selectedEnvId" class="empty">请先选择环境</div>
          <template v-else>
            <div class="component-switcher-head">
              <span>资源视图</span>
              <small v-if="manifestDraftCount > 0">未保存草稿 {{ manifestDraftCount }}</small>
            </div>
            <div class="group-view-switch">
              <button
                type="button"
                class="group-view-btn"
                :class="{ active: resourceGroupView === 'component' }"
                @click="resourceGroupView = 'component'"
              >
                组件
              </button>
              <button
                type="button"
                class="group-view-btn"
                :class="{ active: resourceGroupView === 'file' }"
                @click="resourceGroupView = 'file'"
              >
                文件
              </button>
              <button
                type="button"
                class="group-view-btn"
                :class="{ active: resourceGroupView === 'batch' }"
                @click="resourceGroupView = 'batch'"
              >
                批次
              </button>
            </div>
            <input
              v-model="componentFilterKeyword"
              type="text"
              class="component-search"
              :placeholder="resourceGroupView === 'component' ? '搜索组件名称' : resourceGroupView === 'file' ? '搜索文件名' : '搜索批次名称'"
            />
            <div class="component-list">
              <button
                v-if="resourceGroupView === 'component'"
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
              <button
                v-else-if="resourceGroupView === 'file'"
                v-for="item in filteredSourceFileItems"
                :key="item.name"
                type="button"
                class="component-item"
                :class="{ active: selectedSourceFile === item.name }"
                @click="onSelectSourceFile(item.name)"
              >
                <span class="component-item-name">{{ item.name }}</span>
                <div class="component-item-actions">
                  <small>{{ item.count }}</small>
                </div>
              </button>
              <button
                v-else
                v-for="item in filteredBatchItems"
                :key="item.id"
                type="button"
                class="component-item"
                :class="{ active: selectedBatchId === item.id }"
                @click="onSelectBatch(item.id)"
              >
                <span class="component-item-name">{{ item.name }}</span>
                <div class="component-item-actions">
                  <small>{{ item.count }}</small>
                </div>
              </button>
              <div
                v-if="
                  (resourceGroupView === 'component' && !filteredComponentItems.length) ||
                  (resourceGroupView === 'file' && !filteredSourceFileItems.length) ||
                  (resourceGroupView === 'batch' && !filteredBatchItems.length)
                "
                class="empty"
              >
                {{ resourceGroupView === "component" ? "没有匹配的组件" : resourceGroupView === "file" ? "没有匹配的文件" : "没有匹配的批次" }}
              </div>
            </div>

          <div class="list-title">
            <span>资源列表</span>
            <small>{{ activeGroupLabel || "-" }}</small>
          </div>
          <div class="resource-list-panel">
            <div
              v-for="m in manifestsInActiveGroup"
              :key="m.id"
              class="item"
              :class="{ active: selectedManifestId === m.id }"
              @click="onSelectManifest(m.id)"
            >
              <div class="item-title">
                <span class="item-kind">{{ m.resource_kind }}</span>
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
              <div class="item-name-row">
                <strong class="item-name">{{ m.resource_name }}</strong>
                <strong v-if="hasManifestDraft(m.id)" class="draft-tag">草稿</strong>
              </div>
              <div class="item-sub">
                <span>命名空间：{{ m.resource_namespace || "default" }}</span>
              </div>
              <div v-if="m.source_file_name" class="item-meta">
                <span>{{ m.source_file_name }}#{{ m.source_doc_index ?? 1 }}</span>
              </div>
            </div>
            <div v-if="!manifestsInActiveGroup.length" class="empty">
              {{ resourceGroupView === "component" ? "当前组件暂无资源" : resourceGroupView === "file" ? "当前文件暂无资源" : "当前批次暂无资源" }}
            </div>
          </div>
          </template>
        </div>
      </aside>

      <section class="editor-panel">
        <input
          ref="importFileInput"
          type="file"
          accept=".yaml,.yml"
          multiple
          class="import-file-input"
          @change="onImportFilesSelected"
        />
        <div v-if="createYamlActive" class="meta-row">
          <div class="meta-component-editor">
            <div class="meta-field">
              <span>新建资源</span>
              <strong class="meta-component-name">{{ importComponent || selectedComponent || "default" }}</strong>
            </div>
          </div>
          <div class="meta-actions">
            <button type="button" class="btn btn-import" :disabled="importLoading" @click="triggerImportFileSelect">
              {{ importLoading ? "导入中…" : "导入文件" }}
            </button>
            <button
              type="button"
              class="btn"
              :disabled="importLoading"
              @click="importTextDraft = ''; importPreviewItems = []; importSummaryMessage = null"
            >
              清空
            </button>
            <button type="button" class="btn" :disabled="importLoading" @click="closeCreateYamlDialog">关闭</button>
          </div>
        </div>
        <div v-else-if="selectedManifest" class="meta-row">
          <div class="meta-component-editor">
            <div class="meta-field">
              <span>当前所属组件</span>
              <strong class="meta-component-name">{{ selectedManifest.component }}</strong>
            </div>
          </div>
          <div class="meta-actions">
            <button type="button" class="btn btn-move-component" @click="openComponentAssignDialog">变更组件</button>
            <button
              type="button"
              class="btn btn-diff"
              :disabled="!selectedManifestId || diffLoading || createYamlActive"
              @click="loadDiff"
            >
              {{ diffLoading ? "生成中…" : "查看差异" }}
            </button>
            <button
              type="button"
              class="btn btn-save"
              :disabled="!selectedManifestId || createYamlActive"
              @click="onSaveYaml"
            >
              保存
            </button>
            <button
              type="button"
              class="btn btn-copy"
              :disabled="!canOpenCopyDialog || createYamlActive"
              @click="copyDialogVisible = true"
            >
              复制到其他环境
            </button>
            <button
              type="button"
              class="btn btn-primary"
              :disabled="!canOpenApplyDialog || applying || createYamlActive"
              @click="openApplyDialog"
            >
              {{ applying ? "应用中…" : "应用到当前环境" }}
            </button>
            <span class="hint">资源：{{ selectedManifest.resource_kind }}/{{ selectedManifest.resource_name }}</span>
            <span v-if="selectedManifest.source_file_name" class="hint">
              来源：{{ selectedManifest.source_file_name }}#{{ selectedManifest.source_doc_index ?? 1 }}
            </span>
          </div>
        </div>
        <template v-if="createYamlActive">
          <div class="create-toolbar">
            <label class="field-label create-field">
              <span>保存到组件</span>
              <input v-model="importComponent" type="text" class="assign-input import-input" placeholder="输入组件名称" />
            </label>
            <label class="field-check">
              <input v-model="importOverwrite" type="checkbox" :disabled="importLoading" />
              遇到同名资源时覆盖已有编排资产
            </label>
            <div class="create-toolbar-actions">
              <button type="button" class="btn btn-primary" :disabled="!canConfirmImport" @click="onConfirmImport">
                {{
                  importLoading
                    ? "保存中…"
                    : importValidItems.length === 1
                      ? "保存资源"
                      : `保存 ${importValidItems.length} 个资源`
                }}
              </button>
            </div>
          </div>
          <CodeEditor
            v-model:value="importTextDraft"
            language="yaml"
            :theme="monacoTheme"
            :options="editorOptions"
            class="editor"
          />
          <div v-if="importSummaryMessage" class="message message-ok">{{ importSummaryMessage }}</div>
          <div v-if="opError" class="message message-error">{{ opError }}</div>
          <div v-if="opMessage" class="message message-ok">{{ opMessage }}</div>
        </template>
        <template v-else-if="selectedManifest">
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
        </template>
        <div v-else class="editor-empty-state">
          <div class="editor-empty-icon">YAML</div>
          <div class="editor-empty-title">还没有打开任何资源</div>
          <div class="editor-empty-desc">
            可以从左侧选择一个已有资源继续编辑，或者直接新建一份 YAML 草稿。
          </div>
          <div class="editor-empty-actions">
            <button type="button" class="btn btn-primary" @click="openCreateYamlDialog">新建资源</button>
            <button
              v-if="manifestsInActiveGroup.length"
              type="button"
              class="btn"
              @click="onSelectManifest(manifestsInActiveGroup[0].id)"
            >
              打开第一个资源
            </button>
          </div>
        </div>
      </section>

      <aside class="history">
        <template v-if="createYamlActive">
          <div class="history-title">解析预览</div>
          <div class="copy-tip preview-tip">可以直接编写 YAML，也可以先导入文件到当前草稿。</div>
          <div v-if="!importPreviewItems.length" class="empty">草稿为空或尚未识别到资源。</div>
          <div v-else class="create-preview-list">
            <div
              v-for="item in importPreviewItems"
              :key="item.id"
              class="import-preview-item"
              :class="{
                valid: item.valid && !item.conflict && !item.duplicate && item.warnings.length === 0,
                conflict: item.valid && (item.conflict || item.duplicate || item.warnings.length > 0),
                invalid: !item.valid,
              }"
            >
              <div class="import-preview-title-row">
                <span class="import-preview-type">{{ item.valid ? item.kind : "未识别资源" }}</span>
                <span class="import-preview-doc">文档 #{{ item.docIndex }}</span>
              </div>
              <div class="import-preview-name-row">
                <strong class="import-preview-name">{{ item.valid ? item.name : "请检查 YAML 结构" }}</strong>
              </div>
              <div class="import-preview-main">
                <span>{{ item.valid ? `命名空间：${item.namespace || "default"}` : "当前文档未识别出有效资源" }}</span>
              </div>
              <div v-if="item.fileName !== '当前草稿'" class="import-preview-meta">
                <small class="import-preview-source">来源文件：{{ item.fileName }}</small>
              </div>
              <div v-for="(msg, idx) in item.errors" :key="`err-side-${item.id}-${idx}`" class="import-preview-tip error-tip">{{ msg }}</div>
              <div v-for="(msg, idx) in item.warnings" :key="`warn-side-${item.id}-${idx}`" class="import-preview-tip">
                {{ msg }}
              </div>
            </div>
          </div>
        </template>
        <template v-else>
          <div class="history-title">历史快照</div>
          <div v-if="!selectedManifest" class="empty">先选择一个资源，或新建 YAML 后再查看这里的内容。</div>
          <div v-else-if="selectedHistory.length === 0" class="empty">暂无历史</div>
          <button
            v-for="h in selectedHistory"
            :key="h.id"
            type="button"
            class="history-item"
            :class="`history-${h.action}`"
            @click="onViewHistory(h)"
          >
            <span>{{ h.action }}</span>
            <span>{{ new Date(h.at).toLocaleString() }}</span>
          </button>
        </template>
      </aside>
    </div>

    <OrchestratorPackageView
      v-else
      :selected-env-id="selectedEnvId"
      :environments="environments"
      :components="components"
      :manifests-by-env="manifestsByEnv"
      @op-message="opMessage = $event"
      @op-error="opError = $event"
    />

    <OrchestratorDiffModal
      :visible="diffVisible"
      :loading="diffLoading"
      :not-found="diffNotFound"
      :diff-rows="diffRows"
      @close="diffVisible = false; diffRows = []; diffNotFound = false"
    />

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
      <div v-if="componentApplyDialogVisible" class="apply-modal-overlay" @click.self="closeComponentApplyDialog">
        <section class="apply-modal apply-flow-modal" role="dialog" aria-label="应用组件流程">
          <header class="apply-head">
            <h3>应用组件</h3>
            <div class="apply-flow-subtitle">
              环境：{{ environments.find((env) => env.id === selectedEnvId)?.display_name || "-" }} · 组件：{{ selectedComponent || "-" }}
            </div>
          </header>
          <div class="apply-body apply-flow-body">
            <div class="apply-flow-summary">
              <span class="risk-pill notice">总数 {{ componentApplySummary.total }}</span>
              <span class="risk-pill notice">未开始 {{ componentApplySummary.pending }}</span>
              <span class="risk-pill notice" v-if="componentApplySummary.running">进行中 {{ componentApplySummary.running }}</span>
              <span class="risk-pill" :class="componentApplySummary.failed ? 'warning' : 'notice'">成功 {{ componentApplySummary.success }}</span>
              <span class="risk-pill error" v-if="componentApplySummary.failed">失败 {{ componentApplySummary.failed }}</span>
            </div>
            <div class="copy-tip">
              {{ componentApplyPhase === "idle" ? "系统会按资源依赖顺序逐条应用当前组件。" : componentApplyPhase === "applying" ? "正在按顺序应用资源，请留意每条状态变化。" : "本次组件应用已经完成，可在下方查看成功与失败详情。" }}
            </div>
            <div class="apply-flow-list">
              <div
                v-for="item in componentApplyItems"
                :key="item.manifestId"
                class="apply-flow-item"
                :class="`status-${item.status}`"
              >
                <div class="apply-flow-item-head">
                  <span class="apply-flow-kind">{{ item.kind }}</span>
                  <span class="apply-flow-status">{{ item.status === "pending" ? "未开始" : item.status === "running" ? "应用中" : item.status === "success" ? "成功" : "失败" }}</span>
                </div>
                <div class="apply-flow-name">{{ item.name }}</div>
                <div class="apply-flow-meta">
                  <span>命名空间：{{ item.namespace || "default" }}</span>
                  <span v-if="item.fileName">来源文件：{{ item.fileName }}</span>
                </div>
                <div v-if="item.error" class="apply-flow-error">{{ item.error }}</div>
              </div>
            </div>
          </div>
          <footer class="apply-foot">
            <button type="button" class="btn" :disabled="componentApplyPhase === 'applying'" @click="closeComponentApplyDialog">
              {{ componentApplyPhase === "completed" ? "关闭" : "取消" }}
            </button>
            <button
              type="button"
              class="btn btn-primary"
              :disabled="componentApplyPhase === 'applying' || !componentApplyItems.length"
              @click="startComponentApplyFromDialog"
            >
              {{
                componentApplyPhase === "idle"
                  ? "开始应用"
                  : componentApplySummary.failed
                    ? "重新应用"
                    : "再次应用"
              }}
            </button>
          </footer>
        </section>
      </div>
    </Teleport>
    <OrchestratorCopyDialog
      :visible="copyDialogVisible"
      :selected-env-id="selectedEnvId"
      :selected-component="selectedComponent"
      :environments="environments"
      @close="copyDialogVisible = false"
      @op-message="opMessage = $event"
      @op-error="opError = $event"
    />
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
  <ResourceSnapshotViewer
    :visible="!!viewingHistoryItem"
    :snapshot="viewingHistorySnapshot"
    :env-id="selectedEnvId"
    @close="viewingHistoryItem = null"
  />
</template>


<style src="./orchestrator-view.css" scoped></style>
