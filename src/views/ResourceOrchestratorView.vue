<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  NButton,
  NCheckbox,
  NDropdown,
  NInput,
  NRadio,
  NRadioGroup,
  NSelect,
  NSpace,
  type DropdownOption,
} from "naive-ui";
import { kfSpace } from "../kf";
import BaseModal from "../components/base/BaseModal.vue";

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
import { useOrchestratorStore, type OrchestratorManifest } from "../stores/orchestrator";
import { isClusterScopedKind, rewriteYamlNamespace } from "../stores/orchestratorUtils";
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
  componentMetas,
  orchestratorFocusTarget,
  saveManifestYaml,
  setManifestIdentity,
  setManifestComponent,
  deleteManifest,
  importManifestsToEnv,
  getComponentNamespace,
  setComponentNamespace,
  deleteComponentMeta,
} = useOrchestratorStore();
const { monacoTheme } = useYamlMonacoTheme();

const activeView = ref<"resources" | "packages">("resources");
const selectedEnvId = ref<string>("");
const selectedComponent = ref<string>("");
const selectedManifestId = ref<string>("");
const componentNamespaceDraft = ref("default");
const editYaml = ref("");
const validationErrors = ref<string[]>([]);
const validationWarnings = ref<string[]>([]);
const opMessage = ref<string | null>(null);
const opError = ref<string | null>(null);
const viewingHistoryItem = ref<ManifestHistoryItem | null>(null);
const createYamlActive = ref(false);
const historyPanelOpen = ref(false);
const nsEditOpen = ref(false);
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

const envSelectOptions = computed(() =>
  environments.value.map((e) => ({ label: e.display_name, value: e.id }))
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

const componentSelectOptions = computed(() =>
  componentItems.value.map((item) => ({
    label: `${item.name}（${item.count}）`,
    value: item.name,
  }))
);
const selectedComponentResourceCount = computed(
  () => componentItems.value.find((c) => c.name === selectedComponent.value)?.count ?? 0
);

const selectedComponentNamespace = computed(() => {
  if (!selectedEnvId.value || !selectedComponent.value) return "default";
  // 依赖 componentMetas 以保持响应式
  void componentMetas.value.length;
  return getComponentNamespace(selectedEnvId.value, selectedComponent.value);
});

const manifestsByComponent = computed(() =>
  manifestsByEnv.value.filter((m) => m.component === selectedComponent.value)
);
const componentOptionsForAssign = computed(() =>
  components.value.filter((name) => name !== selectedManifest.value?.component)
);
const componentAssignSelectOptions = computed(() =>
  componentOptionsForAssign.value.map((name) => ({ label: name, value: name }))
);
const manifestDraftCount = computed(() => {
  const draftIds = new Set<string>();
  for (const m of manifestsByComponent.value) {
    const draft = manifestDraftCache.value[m.id];
    if (draft && draft !== m.yaml) draftIds.add(m.id);
  }
  const current = selectedManifest.value;
  if (current && editYaml.value !== current.yaml) draftIds.add(current.id);
  return draftIds.size;
});
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
const canPushComponent = computed(
  () => Boolean(selectedEnvId.value && selectedComponent.value && environments.value.length > 1)
);
const moreMenuOptions = computed<DropdownOption[]>(() => {
  const options: DropdownOption[] = [
    { label: "新建资源", key: "create", disabled: !selectedEnvId.value },
    { label: "应用包", key: "packages" },
  ];
  if (selectedManifest.value && !createYamlActive.value) {
    options.push({
      label: diffLoading.value ? "查看差异（生成中…）" : "查看差异",
      key: "diff",
      disabled: !selectedManifestId.value || diffLoading.value,
    });
  }
  if (selectedComponent.value && selectedComponentResourceCount.value > 0) {
    options.push({ label: "删除组件", key: "delete-component" });
  }
  return options;
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
  componentApplyDialogVisible,
  componentApplyItems,
  componentApplyPhase,
  canApplyCurrent,
  canApplyComponent,
  componentApplySummary,
  closeComponentApplyDialog,
  onApplyCurrent,
  openComponentApplyFlow,
  startComponentApplyFromDialog,
} = useOrchestratorApplyFlow({
  selectedEnvId,
  selectedComponent,
  selectedManifestId,
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
  deleteComponentMeta(selectedEnvId.value, target.component);
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
  const manifest = manifests.value.find((m) => m.id === manifestId);
  if (!manifest) return false;
  const draft = manifestDraftCache.value[manifestId];
  if (draft && draft !== manifest.yaml) return true;
  if (manifestId === selectedManifestId.value && editYaml.value !== manifest.yaml) return true;
  return false;
}

function saveManifestDraftById(manifestId: string): boolean {
  const manifest = manifests.value.find((m) => m.id === manifestId);
  if (!manifest || !selectedEnvId.value) return false;
  const yaml =
    manifestId === selectedManifestId.value
      ? editYaml.value
      : manifestDraftCache.value[manifestId];
  if (!yaml || yaml === manifest.yaml) return false;

  let parsed: unknown;
  try {
    parsed = jsYaml.load(yaml);
  } catch {
    return false;
  }
  if (!parsed || typeof parsed !== "object") return false;
  const obj = parsed as Record<string, unknown>;
  if (typeof obj.apiVersion !== "string" || !obj.apiVersion) return false;
  if (typeof obj.kind !== "string" || !obj.kind) return false;
  const metadata =
    obj.metadata && typeof obj.metadata === "object"
      ? (obj.metadata as Record<string, unknown>)
      : null;
  if (!metadata || typeof metadata.name !== "string" || !metadata.name) return false;

  const identity = parseIdentity(yaml);
  if (!identity) return false;
  const ns = getComponentNamespace(selectedEnvId.value, manifest.component);
  const preparedYaml = rewriteYamlNamespace(yaml, ns, identity.kind);
  const preparedIdentity = {
    kind: identity.kind,
    name: identity.name,
    namespace: isClusterScopedKind(identity.kind) ? null : ns,
  };
  setManifestComponent(manifestId, manifest.component);
  setManifestIdentity(manifestId, preparedIdentity);
  saveManifestYaml(manifestId, preparedYaml, "save");
  delete manifestDraftCache.value[manifestId];
  if (manifestId === selectedManifestId.value) {
    editYaml.value = preparedYaml;
  }
  return true;
}

function flushComponentDrafts(): number {
  let saved = 0;
  for (const m of manifestsByComponent.value) {
    if (saveManifestDraftById(m.id)) saved += 1;
  }
  return saved;
}

function onMoreMenuSelect(key: string | number) {
  switch (key) {
    case "create":
      openCreateYamlDialog();
      break;
    case "packages":
      activeView.value = "packages";
      break;
    case "diff":
      loadDiff();
      break;
    case "delete-component":
      if (selectedComponent.value) {
        openDeleteComponentDialog(selectedComponent.value, selectedComponentResourceCount.value);
      }
      break;
  }
}

function backToResourcesView() {
  activeView.value = "resources";
}

function openCopyComponentDialog() {
  if (!canPushComponent.value) return;
  copyDialogVisible.value = true;
}

function openApplyComponent() {
  openComponentApplyFlow();
}

function commitComponentNamespace() {
  if (!selectedEnvId.value || !selectedComponent.value) return;
  const next = componentNamespaceDraft.value.trim() || "default";
  componentNamespaceDraft.value = next;
  if (next === selectedComponentNamespace.value) return;
  const ns = setComponentNamespace(selectedEnvId.value, selectedComponent.value, next);
  componentNamespaceDraft.value = ns;
  // 命名空间变更会改写组件内 YAML，同步当前编辑区
  const current = selectedManifest.value;
  if (current) {
    editYaml.value = manifestDraftCache.value[current.id] ?? current.yaml;
  }
  opError.value = null;
  opMessage.value = `组件「${selectedComponent.value}」安装命名空间已设为 ${ns}。`;
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
  () =>
    [
      orchestratorFocusTarget.value?.env_id ?? "",
      orchestratorFocusTarget.value?.manifest_id ?? "",
      orchestratorFocusTarget.value?.component ?? "",
      manifests.value.map((m) => `${m.id}:${m.component}:${m.updated_at}`).join("|"),
    ] as const,
  () => {
    applyPendingOrchestratorFocus();
  },
  { immediate: true }
);

watch(
  () => [selectedEnvId.value, components.value.join("|")] as const,
  () => {
    if (orchestratorFocusTarget.value) return;
    if (!components.value.length) selectedComponent.value = "";
    else if (!selectedComponent.value || !components.value.includes(selectedComponent.value)) {
      selectedComponent.value = components.value[0];
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
  () => [selectedEnvId.value, selectedComponent.value, selectedComponentNamespace.value] as const,
  () => {
    componentNamespaceDraft.value = selectedComponentNamespace.value;
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
    } else if (
      !selectedManifestId.value ||
      !manifestsByComponent.value.some((m) => m.id === selectedManifestId.value)
    ) {
      selectedManifestId.value = manifestsByComponent.value[0].id;
    }
    if (selectedComponent.value && selectedManifestId.value) {
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

  validationWarnings.value = extractRefWarningsFromInventory(editYaml.value, manifestsByComponent.value);
  return true;
}

function onSaveYaml() {
  if (!selectedManifest.value || !selectedEnvId.value) return;
  const ok = validateCurrent();
  if (!ok) return;
  const identity = parseIdentity(editYaml.value);
  if (!identity) {
    opError.value = "无法解析资源身份信息（kind/metadata.name）。";
    return;
  }
  const ns = getComponentNamespace(selectedEnvId.value, selectedComponent.value);
  const preparedYaml = rewriteYamlNamespace(editYaml.value, ns, identity.kind);
  const preparedIdentity = {
    kind: identity.kind,
    name: identity.name,
    namespace: isClusterScopedKind(identity.kind) ? null : ns,
  };
  editYaml.value = preparedYaml;
  setManifestComponent(selectedManifest.value.id, selectedComponent.value);
  setManifestIdentity(selectedManifest.value.id, preparedIdentity);
  saveManifestYaml(selectedManifest.value.id, preparedYaml, "save");
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

defineExpose({ openComponentAssignDialog });

</script>

<template>
  <div class="orchestrator-layout">
    <header class="toolbar">
      <NSpace v-bind="kfSpace.orchestratorToolbar" class="toolbar-row">
        <div class="toolbar-brand">
          <span class="title">编排中心</span>
        </div>
        <template v-if="activeView === 'resources'">
          <div class="toolbar-env">
            <span class="toolbar-env-label">环境</span>
            <NSelect
              v-model:value="selectedEnvId"
              :options="envSelectOptions"
              placeholder="选择环境"
              filterable
              class="toolbar-env-select env-select-naive"
            />
          </div>
          <div class="toolbar-trailing">
            <NDropdown trigger="click" :options="moreMenuOptions" @select="onMoreMenuSelect">
              <NButton size="small" secondary>更多</NButton>
            </NDropdown>
          </div>
        </template>
        <template v-else>
          <span class="toolbar-subtitle">应用包</span>
          <div class="toolbar-trailing">
            <NButton size="small" secondary @click="backToResourcesView">返回资源</NButton>
          </div>
        </template>
      </NSpace>
    </header>

    <div v-if="activeView === 'resources'" class="body-stack">
      <div v-if="selectedEnvId" class="component-bar">
        <NSpace v-bind="kfSpace.orchestratorToolbar" class="component-bar-row">
          <div class="component-bar-field">
            <span class="component-bar-label">组件</span>
            <NSelect
              v-model:value="selectedComponent"
              :options="componentSelectOptions"
              placeholder="选择组件"
              filterable
              :disabled="!componentSelectOptions.length"
              class="component-bar-select env-select-naive"
            />
          </div>
          <span v-if="manifestDraftCount > 0" class="draft-pill">未保存 {{ manifestDraftCount }}</span>
          <template v-if="selectedComponent">
            <NButton
              v-if="!nsEditOpen"
              quaternary
              size="tiny"
              class="ns-chip"
              @click="nsEditOpen = true"
            >
              ns: {{ selectedComponentNamespace }}
            </NButton>
            <div v-else class="ns-edit-row">
              <NInput
                v-model:value="componentNamespaceDraft"
                placeholder="例如 default / prod"
                size="small"
                class="ns-edit-input"
                @keydown.enter.prevent="commitComponentNamespace(); nsEditOpen = false"
              />
              <NButton size="small" @click="commitComponentNamespace(); nsEditOpen = false">确定</NButton>
              <NButton
                size="small"
                secondary
                @click="componentNamespaceDraft = selectedComponentNamespace; nsEditOpen = false"
              >取消</NButton>
            </div>
          </template>
          <NSpace v-bind="kfSpace.metaActions" class="component-bar-actions">
            <NButton
              type="primary"
              size="small"
              :disabled="!canPushComponent || createYamlActive"
              @click="openCopyComponentDialog"
            >跨环境拷贝…</NButton>
            <NButton
              size="small"
              secondary
              :disabled="!canApplyComponent || applying || createYamlActive"
              :loading="applying"
              @click="openApplyComponent"
            >应用此组件</NButton>
          </NSpace>
        </NSpace>
      </div>

      <div
        class="body"
        :class="{ 'body--history-open': historyPanelOpen || createYamlActive }"
      >
        <aside class="list">
          <div v-if="!selectedEnvId" class="empty orch-env-empty">请先选择环境</div>
          <template v-else-if="!selectedComponent">
            <div class="empty">当前环境暂无组件，请先新建或导入资源。</div>
          </template>
          <template v-else>
            <div class="orch-sidebar-tier-head">
              <div class="orch-sidebar-tier-head-main">
                <span class="orch-sidebar-tier-label">资源</span>
                <span class="orch-sidebar-tier-count">{{ manifestsByComponent.length }}</span>
              </div>
            </div>
            <div class="resource-list-panel">
              <div
                v-for="m in manifestsByComponent"
                :key="m.id"
                class="item"
                :class="{ active: selectedManifestId === m.id }"
              >
                <NButton
                  quaternary
                  class="resource-item-main"
                  @click="onSelectManifest(m.id)"
                >
                  <div class="item-title">
                    <span class="item-kind">{{ m.resource_kind }}</span>
                  </div>
                  <div class="item-name-row">
                    <strong class="item-name">{{ m.resource_name }}</strong>
                    <strong v-if="hasManifestDraft(m.id)" class="draft-tag">草稿</strong>
                  </div>
                  <div class="item-sub">
                    <span v-if="m.resource_namespace === null">集群资源</span>
                    <span v-else>命名空间：{{ selectedComponentNamespace }}</span>
                  </div>
                  <div v-if="m.source_file_name" class="item-meta">
                    <span>{{ m.source_file_name }}#{{ m.source_doc_index ?? 1 }}</span>
                  </div>
                </NButton>
                <NButton
                  text
                  type="error"
                  class="resource-item-close"
                  title="删除资源"
                  aria-label="删除资源"
                  @click="openDeleteResourceDialog(m)"
                >
                  ×
                </NButton>
              </div>
              <div v-if="!manifestsByComponent.length" class="empty empty--tier-muted">当前组件暂无资源</div>
            </div>
          </template>
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
            <NSpace v-bind="kfSpace.metaActions" class="meta-actions">
              <NButton type="primary" size="small" :loading="importLoading" @click="triggerImportFileSelect">
                {{ importLoading ? "导入中…" : "导入文件" }}
              </NButton>
              <NButton
                size="small"
                :disabled="importLoading"
                @click="importTextDraft = ''; importPreviewItems = []; importSummaryMessage = null"
              >清空</NButton>
              <NButton size="small" :disabled="importLoading" @click="closeCreateYamlDialog">关闭</NButton>
            </NSpace>
          </div>
          <div v-else-if="selectedManifest" class="meta-row">
            <div class="meta-component-editor">
              <div class="meta-field">
                <span>{{ selectedManifest.resource_kind }}/{{ selectedManifest.resource_name }}</span>
                <span v-if="selectedManifest.source_file_name" class="hint">
                  来源：{{ selectedManifest.source_file_name }}#{{ selectedManifest.source_doc_index ?? 1 }}
                </span>
              </div>
            </div>
            <NSpace v-bind="kfSpace.metaActions" class="meta-actions">
              <NButton
                size="small"
                :type="historyPanelOpen ? 'primary' : 'default'"
                :secondary="!historyPanelOpen"
                :disabled="createYamlActive"
                @click="historyPanelOpen = !historyPanelOpen"
              >历史</NButton>
              <NButton
                size="small"
                secondary
                :disabled="!selectedManifestId || diffLoading || createYamlActive"
                :loading="diffLoading"
                @click="loadDiff"
              >{{ diffLoading ? "生成中…" : "差异" }}</NButton>
              <NButton
                size="small"
                type="primary"
                :disabled="!selectedManifestId || createYamlActive"
                @click="onSaveYaml"
              >保存</NButton>
              <NButton
                size="small"
                type="primary"
                :disabled="!canApplyCurrent || applying || createYamlActive"
                :loading="applying"
                @click="onApplyCurrent"
              >{{ applying ? "应用中…" : "应用此资源" }}</NButton>
            </NSpace>
          </div>
          <template v-if="createYamlActive">
            <div class="create-toolbar">
              <label class="field-label create-field">
                <span>保存到组件</span>
                <NInput
                  v-model:value="importComponent"
                  type="text"
                  size="small"
                  class="import-component-naive"
                  placeholder="输入组件名称"
                />
              </label>
              <NCheckbox v-model:checked="importOverwrite" :disabled="importLoading" class="create-overwrite-naive">
                遇到同名资源时覆盖已有编排资产
              </NCheckbox>
              <div class="create-toolbar-actions">
                <NButton
                  type="primary"
                  size="small"
                  :disabled="!canConfirmImport"
                  :loading="importLoading"
                  @click="onConfirmImport"
                >
                  {{
                    importLoading
                      ? "保存中…"
                      : importValidItems.length === 1
                        ? "保存资源"
                        : `保存 ${importValidItems.length} 个资源`
                  }}
                </NButton>
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
            <NSpace v-bind="kfSpace.editorEmptyActions" class="editor-empty-actions">
              <NButton type="primary" @click="openCreateYamlDialog">新建资源</NButton>
              <NButton
                v-if="manifestsByComponent.length"
                secondary
                @click="onSelectManifest(manifestsByComponent[0].id)"
              >打开第一个资源</NButton>
            </NSpace>
          </div>
        </section>

        <aside v-if="historyPanelOpen || createYamlActive" class="history">
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
            <div v-else class="history-list">
              <NButton
                v-for="h in selectedHistory"
                :key="h.id"
                quaternary
                block
                class="history-item"
                :class="`history-${h.action}`"
                @click="onViewHistory(h)"
              >
                <span>{{ ACTION_LABELS[h.action] ?? h.action }}</span>
                <span>{{ new Date(h.at).toLocaleString() }}</span>
              </NButton>
            </div>
          </template>
        </aside>
      </div>
    </div>

    <div v-else class="pkg-view-host">
      <OrchestratorPackageView
        :selected-env-id="selectedEnvId"
        :environments="environments"
        :components="components"
        :manifests-by-env="manifestsByEnv"
        @op-message="opMessage = $event"
        @op-error="opError = $event"
      />
    </div>

    <OrchestratorDiffModal
      :visible="diffVisible"
      :loading="diffLoading"
      :not-found="diffNotFound"
      :diff-rows="diffRows"
      @close="diffVisible = false; diffRows = []; diffNotFound = false"
    />

    <BaseModal
      :visible="componentApplyDialogVisible"
      title="应用组件"
      width="760px"
      @close="closeComponentApplyDialog"
    >
      <div class="apply-flow-subtitle">
        环境：{{ environments.find((env) => env.id === selectedEnvId)?.display_name || "-" }} ·
        组件：{{ selectedComponent || "-" }}
      </div>
      <div class="apply-body apply-flow-body">
        <div class="apply-flow-summary">
          <span class="risk-pill notice">总数 {{ componentApplySummary.total }}</span>
          <span class="risk-pill notice">未开始 {{ componentApplySummary.pending }}</span>
          <span v-if="componentApplySummary.running" class="risk-pill notice"
            >进行中 {{ componentApplySummary.running }}</span
          >
          <span class="risk-pill" :class="componentApplySummary.failed ? 'warning' : 'notice'"
            >成功 {{ componentApplySummary.success }}</span
          >
          <span v-if="componentApplySummary.failed" class="risk-pill error"
            >失败 {{ componentApplySummary.failed }}</span
          >
        </div>
        <div class="copy-tip">
          {{
            componentApplyPhase === "idle"
              ? "系统会按资源依赖顺序逐条应用当前组件。"
              : componentApplyPhase === "applying"
                ? "正在按顺序应用资源，请留意每条状态变化。"
                : "本次组件应用已经完成，可在下方查看成功与失败详情。"
          }}
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
              <span class="apply-flow-status">{{
                item.status === "pending"
                  ? "未开始"
                  : item.status === "running"
                    ? "应用中"
                    : item.status === "success"
                      ? "成功"
                      : "失败"
              }}</span>
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
      <template #footer>
        <NButton
          secondary
          :disabled="componentApplyPhase === 'applying'"
          @click="closeComponentApplyDialog"
        >
          {{ componentApplyPhase === "completed" ? "关闭" : "取消" }}
        </NButton>
        <NButton
          type="primary"
          :disabled="componentApplyPhase === 'applying' || !componentApplyItems.length"
          :loading="applying"
          @click="startComponentApplyFromDialog"
        >
          {{
            componentApplyPhase === "idle"
              ? "开始应用"
              : componentApplySummary.failed
                ? "重新应用"
                : "再次应用"
          }}
        </NButton>
      </template>
    </BaseModal>

    <OrchestratorCopyDialog
      :visible="copyDialogVisible"
      :selected-env-id="selectedEnvId"
      :selected-component="selectedComponent"
      :environments="environments"
      :draft-count="manifestDraftCount"
      :flush-drafts="flushComponentDrafts"
      @close="copyDialogVisible = false"
      @op-message="opMessage = $event"
      @op-error="opError = $event"
    />

    <BaseModal
      :visible="componentAssignDialogVisible"
      title="变更组件归属"
      width="480px"
      @close="closeComponentAssignDialog"
    >
      <div class="apply-body">
        <div class="copy-tip">
          当前资源：<strong>{{ selectedManifest?.resource_kind }}/{{ selectedManifest?.resource_name }}</strong>
        </div>
        <NRadioGroup v-model:value="componentAssignMode" name="orchestrator-component-assign" class="assign-radio-group">
          <NRadio value="existing" :disabled="!componentOptionsForAssign.length">加入已有组件</NRadio>
          <NRadio value="new">新建组件</NRadio>
        </NRadioGroup>
        <NSelect
          v-if="componentAssignMode === 'existing'"
          v-model:value="componentAssignExisting"
          :options="componentAssignSelectOptions"
          :disabled="!componentAssignSelectOptions.length"
          filterable
          class="assign-select-naive"
          placeholder="选择目标组件"
        />
        <NInput
          v-else
          v-model:value="componentAssignNew"
          type="text"
          class="assign-input-naive"
          placeholder="输入新组件名称"
        />
      </div>
      <template #footer>
        <NButton secondary @click="closeComponentAssignDialog">取消</NButton>
        <NButton
          type="primary"
          :disabled="
            componentAssignMode === 'existing' ? !componentAssignExisting : !componentAssignNew.trim()
          "
          @click="applyManifestComponentAssignment"
        >
          确认变更
        </NButton>
      </template>
    </BaseModal>

    <BaseModal
      :visible="listDeleteDialogVisible"
      :title="listContextTarget?.type === 'component' ? '确认删除应用组件' : '确认删除资源'"
      width="480px"
      @close="closeListDeleteDialog"
    >
      <div class="apply-body">
        <div v-if="listContextTarget?.type === 'component'" class="copy-tip">
          将删除应用组件
          <strong>{{ listContextTarget.component }}</strong>，以及该组件下全部资源（{{
            listContextTarget.count
          }}
          个）。
        </div>
        <div v-else class="copy-tip">
          将删除资源
          <strong>{{ listContextTarget?.type === 'resource' ? listContextTarget.label : '' }}</strong>。
        </div>
      </div>
      <template #footer>
        <NButton secondary @click="closeListDeleteDialog">取消</NButton>
        <NButton type="error" @click="confirmDeleteFromContextMenu">确认删除</NButton>
      </template>
    </BaseModal>
  </div>

  <ResourceSnapshotViewer
    :visible="!!viewingHistoryItem"
    :snapshot="viewingHistorySnapshot"
    :env-id="selectedEnvId"
    @close="viewingHistoryItem = null"
  />
</template>


<style src="./orchestrator-view.css" scoped></style>
