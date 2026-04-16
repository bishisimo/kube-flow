<script setup lang="ts">
import { computed, ref, watch } from "vue";

defineOptions({ name: "ResourceOrchestratorView" });
import * as jsYaml from "js-yaml";
import { CodeEditor } from "monaco-editor-vue3";
import { kubeDeployResource, kubeGetResource } from "../api/kube";
import { appSettingsGetResourceDeployStrategy } from "../api/config";
import { extractErrorMessage } from "../utils/errorMessage";
import { formatDateTime } from "../utils/dateFormat";
import { parseKubeObject, type KubeObjectIdentity } from "../utils/yaml";
import {
  buildDiffRows,
  formatCodeCell,
  normalizeYamlForDiff,
  type DiffRow,
} from "../features/orchestrator/yamlDiff";
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
  type OrchestratorPackageResourceSnapshot,
} from "../stores/orchestrator";
import {
  useOrchestratorPackagesStore,
  type OrchestratorPackage,
  type OrchestratorPackageVersion,
} from "../stores/orchestratorPackages";

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
const {
  packages,
  createPackage,
  deletePackage,
  createPackageVersion,
  setPackageVersionTag,
  deletePackageVersion,
  syncPackageVersionToEnv,
  recordPackageDeployment,
  copyComponentToEnv,
} = useOrchestratorPackagesStore();
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
const createYamlActive = ref(false);
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
    opError.value = extractErrorMessage(e);
  } finally {
    diffLoading.value = false;
  }
}

function closeDiff() {
  diffRows.value = [];
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
    opError.value = extractErrorMessage(e);
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
    opError.value = extractErrorMessage(e);
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
    opError.value = extractErrorMessage(e);
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
  const delayedWebhookKeys = buildDelayedWebhookKeys(
    list.map((item) => ({
      resource_kind: item.resource_kind,
      resource_name: item.resource_name,
      resource_namespace: item.resource_namespace,
      yaml: item.yaml,
    }))
  );
  return [...list].sort((a, b) => {
    const wa = applyWeight(
      a.resource_kind,
      delayedWebhookKeys.has(resourceIdentityKey(a.resource_kind, a.resource_name, a.resource_namespace))
    );
    const wb = applyWeight(
      b.resource_kind,
      delayedWebhookKeys.has(resourceIdentityKey(b.resource_kind, b.resource_name, b.resource_namespace))
    );
    if (wa !== wb) return wa - wb;
    if (a.component !== b.component) return a.component.localeCompare(b.component);
    if ((a.resource_namespace || "") !== (b.resource_namespace || "")) {
      return (a.resource_namespace || "").localeCompare(b.resource_namespace || "");
    }
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
    opError.value = extractErrorMessage(e);
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
        errors.push(`${r.component}/${r.resource_kind}/${r.resource_name}: ${extractErrorMessage(e)}`);
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
    opError.value = extractErrorMessage(e);
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
              :disabled="!canOpenCopyDialog || copyLoading || createYamlActive"
              @click="openCopyDialog"
            >
              {{ copyLoading ? "复制中…" : "复制到其他环境" }}
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
            @click="onRestoreHistory(h.yaml)"
          >
            <span>{{ h.action }}</span>
            <span>{{ new Date(h.at).toLocaleString() }}</span>
          </button>
        </template>
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
  gap: 0.75rem;
  flex-wrap: wrap;
  justify-content: space-between;
  padding: 0.75rem 0.9rem;
  border-bottom: 1px solid #e2e8f0;
  background: linear-gradient(180deg, #ffffff 0%, #f8fafc 100%);
}
.toolbar-start,
.toolbar-brand {
  display: flex;
  align-items: center;
}
.toolbar-start {
  gap: 0.75rem;
  flex-wrap: wrap;
  width: 100%;
}
.toolbar-brand {
  flex-direction: column;
  align-items: flex-start;
  gap: 0.1rem;
  margin-right: 0.25rem;
}
.title {
  font-size: 1rem;
  font-weight: 700;
  color: #0f172a;
}
.toolbar-subtitle {
  font-size: 0.75rem;
  color: #64748b;
}
.view-switch {
  display: inline-flex;
  padding: 0.15rem;
  border: 1px solid #dbe3ef;
  border-radius: 10px;
  overflow: hidden;
  background: linear-gradient(180deg, #f8fbff 0%, #eef2ff 100%);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.6);
}
.switch-btn {
  height: 1.9rem;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: #475569;
  font-size: 0.78rem;
  padding: 0 0.8rem;
  cursor: pointer;
  transition: background 0.18s ease, color 0.18s ease, box-shadow 0.18s ease;
}
.switch-btn:hover:not(.active) {
  background: rgba(148, 163, 184, 0.16);
  color: #334155;
}
.switch-btn.active {
  background: #2563eb;
  color: #fff;
  box-shadow: 0 4px 12px rgba(37, 99, 235, 0.25);
}
.env-select-card {
  display: inline-flex;
  align-items: flex-start;
  gap: 0.55rem;
  min-height: 2.6rem;
  padding: 0.45rem 0.55rem 0.45rem 0.8rem;
  border: 1px solid #dbe3ef;
  border-radius: 12px;
  background: linear-gradient(180deg, #ffffff 0%, #f8fafc 100%);
  box-shadow: 0 8px 18px rgba(148, 163, 184, 0.12);
}
.env-select-label {
  font-size: 0.76rem;
  font-weight: 600;
  color: #475569;
  white-space: nowrap;
  line-height: 2rem;
}
.env-select-main {
  display: flex;
  align-items: center;
  gap: 0.55rem;
  flex-wrap: wrap;
}
.select {
  height: 2rem;
  min-width: 12rem;
  border: 1px solid #cbd5e1;
  border-radius: 8px;
  padding: 0 0.7rem;
  background: #fff;
  color: #0f172a;
}
.env-select {
  min-width: 13.5rem;
  border-color: #bfdbfe;
  background:
    linear-gradient(180deg, #ffffff 0%, #eff6ff 100%);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.8);
}
.env-current-chip {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  min-height: 2rem;
  max-width: 16rem;
  padding: 0 0.75rem;
  border: 1px solid #93c5fd;
  border-radius: 999px;
  background: linear-gradient(180deg, #eff6ff 0%, #dbeafe 100%);
  color: #1d4ed8;
  font-size: 0.8rem;
  font-weight: 700;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.7);
}
.env-current-chip.empty {
  border-color: #cbd5e1;
  background: linear-gradient(180deg, #f8fafc 0%, #f1f5f9 100%);
  color: #64748b;
  font-weight: 600;
}
.env-current-dot {
  width: 0.55rem;
  height: 0.55rem;
  border-radius: 999px;
  background: #2563eb;
  box-shadow: 0 0 0 4px rgba(37, 99, 235, 0.12);
  flex: 0 0 auto;
}
.env-current-chip.empty .env-current-dot {
  background: #94a3b8;
  box-shadow: 0 0 0 4px rgba(148, 163, 184, 0.12);
}
.env-current-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
  opacity: 1;
  cursor: not-allowed;
}
.btn-primary {
  border-color: #2563eb;
  background: #2563eb;
  color: #fff;
}
.btn-primary:hover:not(:disabled) {
  border-color: #1d4ed8;
  background: #1d4ed8;
}
.btn-primary:disabled {
  border-color: #93c5fd;
  background: #eff6ff;
  color: #1e3a8a;
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
  border-color: #67e8f9;
  background: #ecfeff;
  color: #155e75;
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
  background: #f0fdf4;
  color: #166534;
}
.btn-import {
  border-color: #d97706;
  background: #d97706;
  color: #fff;
}
.btn-import:hover:not(:disabled) {
  border-color: #b45309;
  background: #b45309;
}
.btn-import:disabled {
  border-color: #fcd34d;
  background: #fffbeb;
  color: #92400e;
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
  background: #f0fdfa;
  color: #115e59;
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
  background: #f0f9ff;
  color: #0c4a6e;
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
  background: #f5f3ff;
  color: #5b21b6;
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
  background: #f8fafc;
}
.list,
.history {
  border-right: 1px solid #e2e8f0;
  overflow: auto;
  padding: 0.75rem;
}
.history {
  border-left: 1px solid #e2e8f0;
  border-right: none;
  background: linear-gradient(180deg, #fbfdff 0%, #f8fafc 100%);
}
.list {
  background: linear-gradient(180deg, #fbfdff 0%, #f8fafc 100%);
}
.component-switcher {
  border: 1px solid #dbe3ef;
  border-radius: 14px;
  background: linear-gradient(180deg, #f8fbff 0%, #eff6ff 100%);
  padding: 0.65rem;
  margin-bottom: 0.7rem;
  box-shadow: 0 10px 24px rgba(148, 163, 184, 0.12);
}
.env-select-card-sidebar {
  width: 100%;
  margin-bottom: 0.7rem;
}
.env-select-card-sidebar .env-select-main {
  width: 100%;
}
.env-select-card-sidebar .env-select {
  width: 100%;
  min-width: 0;
}
.env-select-card-sidebar .env-current-chip {
  width: 100%;
  max-width: none;
  justify-content: flex-start;
}
.component-switcher-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.55rem;
  font-size: 0.78rem;
  font-weight: 700;
  color: #1e293b;
}
.component-switcher-head small {
  color: #0f766e;
  font-weight: 600;
}
.group-view-switch {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 0.35rem;
  margin-bottom: 0.55rem;
}
.group-view-btn {
  height: 1.95rem;
  border: 1px solid #d5deea;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.88);
  color: #475569;
  font-size: 0.74rem;
  cursor: pointer;
  transition: all 0.18s ease;
}
.group-view-btn:hover:not(.active) {
  border-color: #bfdbfe;
  background: #f8fbff;
  color: #334155;
}
.group-view-btn.active {
  border-color: #2563eb;
  background: linear-gradient(180deg, #dbeafe 0%, #bfdbfe 100%);
  color: #1d4ed8;
  box-shadow: 0 8px 18px rgba(37, 99, 235, 0.16);
}
.component-search {
  width: 100%;
  height: 2rem;
  border: 1px solid #cbd5e1;
  border-radius: 8px;
  padding: 0 0.6rem;
  font-size: 0.75rem;
  margin-bottom: 0.55rem;
  background: rgba(255, 255, 255, 0.92);
}
.component-list {
  display: grid;
  gap: 0.35rem;
  max-height: 220px;
  overflow: auto;
}
.component-item {
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.92);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.42rem 0.55rem;
  font-size: 0.74rem;
  color: #0f172a;
  cursor: pointer;
  transition: all 0.18s ease;
}
.component-item:hover {
  border-color: #bfdbfe;
  background: #f8fbff;
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
  background: linear-gradient(180deg, #eff6ff 0%, #dbeafe 100%);
  box-shadow: 0 10px 20px rgba(37, 99, 235, 0.12);
}
.list-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 0.8rem;
  font-weight: 700;
  color: #1e293b;
  margin: 0 0 0.45rem;
  padding: 0 0.1rem;
}
.list-title small {
  color: #64748b;
  font-weight: 600;
}
.resource-list-panel {
  border: 1px solid #dbe3ef;
  border-radius: 14px;
  background: linear-gradient(180deg, #ffffff 0%, #f8fafc 100%);
  padding: 0.45rem;
  box-shadow: 0 10px 24px rgba(148, 163, 184, 0.1);
}
.item {
  padding: 0.5rem 0.6rem;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  margin-bottom: 0.4rem;
  cursor: pointer;
  background: #fff;
  transition: all 0.18s ease;
}
.item:hover {
  border-color: #cbd5e1;
  background: #fbfdff;
}
.item.active {
  border-color: #2563eb;
  background: linear-gradient(180deg, #eff6ff 0%, #dbeafe 100%);
  box-shadow: 0 10px 20px rgba(37, 99, 235, 0.12);
}
.item-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.35rem;
}
.item-kind {
  color: #334155;
  font-size: 0.72rem;
  font-weight: 600;
}
.item-name-row {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  min-width: 0;
  margin-top: 0.1rem;
}
.item-name {
  display: block;
  min-width: 0;
  flex: 1;
  font-size: 0.82rem;
  font-weight: 600;
  color: #0f172a;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
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
  border: 1px solid #f59e0b;
  background: #fffbeb;
  color: #92400e;
  border-radius: 999px;
  font-size: 0.66rem;
  padding: 0.05rem 0.35rem;
  flex-shrink: 0;
}
.item-sub {
  font-size: 0.75rem;
  color: #64748b;
  margin-top: 0.12rem;
}
.item-meta {
  font-size: 0.7rem;
  color: #94a3b8;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-top: 0.08rem;
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
  padding: 0.75rem 0.9rem;
  border-bottom: 1px solid #e2e8f0;
  background: linear-gradient(180deg, #f8fafc 0%, #f1f5f9 100%);
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
  flex-wrap: wrap;
  justify-content: flex-end;
}
.create-toolbar {
  display: flex;
  align-items: end;
  gap: 0.75rem;
  padding: 0.65rem 0.75rem;
  border-bottom: 1px solid #e2e8f0;
  background: #f8fafc;
  flex-wrap: wrap;
}
.create-field {
  min-width: 220px;
  flex: 1;
}
.create-toolbar-actions {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
}
.editor {
  flex: 1;
  min-height: 260px;
}
.editor-empty-state {
  flex: 1;
  min-height: 260px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.55rem;
  padding: 1.5rem;
  text-align: center;
  background: linear-gradient(180deg, #f8fafc 0%, #ffffff 100%);
}
.editor-empty-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 3.2rem;
  height: 3.2rem;
  padding: 0 0.9rem;
  border-radius: 999px;
  background: #dbeafe;
  color: #1d4ed8;
  font-size: 0.92rem;
  font-weight: 700;
  letter-spacing: 0.04em;
}
.editor-empty-title {
  font-size: 0.98rem;
  font-weight: 600;
  color: #0f172a;
}
.editor-empty-desc {
  max-width: 360px;
  font-size: 0.8rem;
  line-height: 1.6;
  color: #64748b;
}
.editor-empty-actions {
  display: inline-flex;
  align-items: center;
  gap: 0.6rem;
  flex-wrap: wrap;
  justify-content: center;
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
.preview-tip {
  padding: 0 0.25rem 0.5rem;
}
.create-preview-list {
  display: grid;
  gap: 0.45rem;
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
.apply-flow-modal {
  width: min(92vw, 760px);
}
.import-modal {
  width: min(92vw, 760px);
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
.import-file-input {
  display: none;
}
.import-upload-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  border: 1px dashed #cbd5e1;
  border-radius: 10px;
  background: #fff7ed;
  padding: 0.8rem;
}
.import-upload-copy {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  font-size: 0.8rem;
  color: #9a3412;
}
.import-text-panel {
  display: grid;
  gap: 0.65rem;
}
.import-textarea {
  min-height: 220px;
  resize: vertical;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  padding: 0.65rem 0.75rem;
  font-size: 0.8rem;
  line-height: 1.5;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  background: #fff;
}
.import-text-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}
.import-input {
  width: 100%;
}
.import-preview {
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  overflow: hidden;
}
.import-preview-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.65rem 0.8rem;
  border-bottom: 1px solid #e2e8f0;
  background: #f8fafc;
  font-size: 0.8rem;
  color: #334155;
}
.import-preview-head small {
  color: #64748b;
}
.import-risk-summary {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
  padding: 0.55rem 0.8rem 0;
  background: #fff;
}
.risk-pill {
  display: inline-flex;
  align-items: center;
  padding: 0.08rem 0.45rem;
  border-radius: 999px;
  font-size: 0.7rem;
  font-weight: 600;
}
.risk-pill.error {
  background: #fee2e2;
  color: #991b1b;
}
.risk-pill.warning {
  background: #fef3c7;
  color: #92400e;
}
.risk-pill.notice {
  background: #dbeafe;
  color: #1d4ed8;
}
.import-preview-list {
  max-height: 320px;
  overflow: auto;
  padding: 0.6rem;
  display: grid;
  gap: 0.45rem;
  background: #fff;
}
.import-preview-item {
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  padding: 0.55rem 0.65rem;
  display: grid;
  gap: 0.28rem;
}
.import-preview-item.valid {
  border-color: #86efac;
  background: #f0fdf4;
}
.import-preview-item.conflict {
  border-color: #fbbf24;
  background: #fffbeb;
}
.import-preview-item.invalid {
  border-color: #fecaca;
  background: #fef2f2;
}
.import-preview-title-row,
.import-preview-main {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  font-size: 0.78rem;
  min-width: 0;
}
.import-preview-name-row,
.import-preview-meta {
  display: flex;
  align-items: center;
  min-width: 0;
}
.import-preview-type {
  color: #334155;
  font-size: 0.72rem;
  font-weight: 600;
}
.import-preview-name {
  display: block;
  width: 100%;
  font-size: 0.82rem;
  font-weight: 600;
  color: #0f172a;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.import-preview-doc,
.import-preview-main span,
.import-preview-main small {
  color: #64748b;
}
.import-preview-main {
  justify-content: flex-start;
  gap: 0.4rem;
}
.import-preview-main span,
.import-preview-main small {
  font-size: 0.74rem;
}
.import-preview-meta {
  justify-content: flex-start;
}
.import-preview-source {
  color: #94a3b8;
  font-size: 0.7rem;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.import-preview-tip {
  font-size: 0.75rem;
  color: #92400e;
}
.import-preview-tip.error-tip {
  color: #b91c1c;
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
.apply-flow-subtitle {
  margin-top: 0.3rem;
  font-size: 0.76rem;
  color: #64748b;
}
.apply-flow-body {
  gap: 0.75rem;
}
.apply-flow-summary {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
}
.apply-flow-list {
  max-height: 420px;
  overflow: auto;
  display: grid;
  gap: 0.55rem;
}
.apply-flow-item {
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  padding: 0.65rem 0.75rem;
  background: #fff;
  display: grid;
  gap: 0.28rem;
}
.apply-flow-item.status-running {
  border-color: #93c5fd;
  background: #eff6ff;
}
.apply-flow-item.status-success {
  border-color: #86efac;
  background: #f0fdf4;
}
.apply-flow-item.status-failed {
  border-color: #fecaca;
  background: #fef2f2;
}
.apply-flow-item-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.6rem;
  min-width: 0;
}
.apply-flow-kind {
  color: #334155;
  font-size: 0.72rem;
  font-weight: 600;
}
.apply-flow-status {
  flex-shrink: 0;
  font-size: 0.72rem;
  font-weight: 600;
  color: #475569;
}
.apply-flow-name {
  font-size: 0.84rem;
  font-weight: 600;
  color: #0f172a;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.apply-flow-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 0.45rem 0.75rem;
  font-size: 0.74rem;
  color: #64748b;
}
.apply-flow-error {
  font-size: 0.74rem;
  color: #b91c1c;
  line-height: 1.45;
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
  background: linear-gradient(180deg, #fbfdff 0%, #f8fafc 100%);
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
  border: 1px solid #dbe3ef;
  border-radius: 14px;
  background: linear-gradient(180deg, #ffffff 0%, #f8fafc 100%);
  padding: 0.8rem;
  box-shadow: 0 10px 24px rgba(148, 163, 184, 0.1);
}
.pkg-side-title,
.pkg-block-title {
  font-size: 0.84rem;
  font-weight: 700;
  color: #1e293b;
  margin-bottom: 0.55rem;
}
.pkg-side-subtitle {
  margin-top: 0.6rem;
  font-size: 0.75rem;
  color: #64748b;
}
.pkg-create-panel {
  margin-bottom: 0.8rem;
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
  border-radius: 8px;
  padding: 0 0.6rem;
  background: rgba(255, 255, 255, 0.96);
}
.pkg-item {
  border: 1px solid #e2e8f0;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.96);
  padding: 0.55rem 0.65rem;
  margin-bottom: 0.45rem;
  cursor: pointer;
  transition: all 0.18s ease;
}
.pkg-item:hover {
  border-color: #bfdbfe;
  background: #f8fbff;
}
.pkg-item.active {
  border-color: #2563eb;
  background: linear-gradient(180deg, #eff6ff 0%, #dbeafe 100%);
  box-shadow: 0 10px 20px rgba(37, 99, 235, 0.12);
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
  border: 1px solid #dbe3ef;
  background: linear-gradient(180deg, #ffffff 0%, #f8fafc 100%);
  border-radius: 999px;
  padding: 0.16rem 0.5rem;
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
  border-radius: 12px;
  background: linear-gradient(180deg, #ffffff 0%, #f8fafc 100%);
  text-align: left;
  padding: 0.55rem 0.7rem;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  transition: all 0.18s ease;
}
.pkg-version-item:hover {
  border-color: #bfdbfe;
  background: #f8fbff;
}
.pkg-version-item.active {
  border-color: #2563eb;
  background: linear-gradient(180deg, #eff6ff 0%, #dbeafe 100%);
  box-shadow: 0 10px 20px rgba(37, 99, 235, 0.12);
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
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.96);
  padding: 0.5rem 0.6rem;
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
  border-radius: 14px;
  background: linear-gradient(180deg, #ffffff 0%, #f8fafc 100%);
  padding: 1rem;
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
  .toolbar-start,
  .toolbar-start {
    width: 100%;
    flex-wrap: wrap;
  }
  .env-select-card {
    order: 4;
  }
  .btn-create {
    order: 5;
  }
}
</style>
