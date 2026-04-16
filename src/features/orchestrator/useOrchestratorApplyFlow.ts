import { computed, ref, type ComputedRef, type Ref } from "vue";
import { extractErrorMessage } from "../../utils/errorMessage";

export interface ComponentApplyItem {
  manifestId: string;
  kind: string;
  name: string;
  namespace: string | null;
  yaml: string;
  fileName: string | null;
  status: "pending" | "running" | "success" | "failed";
  error: string | null;
}

interface ComponentApplyPlanItem {
  id: string;
  resource_kind: string;
  resource_name: string;
  resource_namespace: string | null;
  yaml: string;
  source_file_name?: string | null;
}

export function useOrchestratorApplyFlow(params: {
  selectedEnvId: Ref<string>;
  selectedComponent: Ref<string>;
  selectedManifestId: Ref<string>;
  resourceGroupView: Ref<"component" | "file" | "batch">;
  selectedManifest: ComputedRef<{ id: string } | null>;
  componentApplyPlan: ComputedRef<ComponentApplyPlanItem[]>;
  editYaml: Ref<string>;
  validateCurrent: () => boolean;
  parseIdentity: (yaml: string) => { kind: string; name: string; namespace: string | null } | null;
  deployYamlToEnv: (envId: string, yaml: string) => Promise<void>;
  setManifestComponent: (manifestId: string, component: string) => void;
  setManifestIdentity: (
    manifestId: string,
    identity: { kind: string; name: string; namespace: string | null }
  ) => void;
  saveManifestYaml: (manifestId: string, yaml: string, source: "save" | "apply") => void;
  clearManifestDraft: (manifestId: string) => void;
  setOpError: (message: string | null) => void;
  setOpMessage: (message: string | null) => void;
}) {
  const applying = ref(false);
  const applyDialogVisible = ref(false);
  const componentApplyDialogVisible = ref(false);
  const componentApplyItems = ref<ComponentApplyItem[]>([]);
  const componentApplyPhase = ref<"idle" | "applying" | "completed">("idle");

  const canApplyCurrent = computed(() => Boolean(params.selectedEnvId.value && params.selectedManifestId.value));
  const canApplyComponent = computed(
    () =>
      Boolean(
        params.selectedEnvId.value &&
          params.selectedComponent.value &&
          params.resourceGroupView.value === "component"
      )
  );
  const canOpenApplyDialog = computed(() => canApplyCurrent.value || canApplyComponent.value);
  const componentApplySummary = computed(() => {
    const success = componentApplyItems.value.filter((item) => item.status === "success").length;
    const failed = componentApplyItems.value.filter((item) => item.status === "failed").length;
    const running = componentApplyItems.value.filter((item) => item.status === "running").length;
    const pending = componentApplyItems.value.filter((item) => item.status === "pending").length;
    return {
      total: componentApplyItems.value.length,
      success,
      failed,
      running,
      pending,
    };
  });

  function buildComponentApplyItems(): ComponentApplyItem[] {
    return params.componentApplyPlan.value.map((item) => ({
      manifestId: item.id,
      kind: item.resource_kind,
      name: item.resource_name,
      namespace: item.resource_namespace,
      yaml: item.yaml,
      fileName: item.source_file_name ?? null,
      status: "pending",
      error: null,
    }));
  }

  async function onApplyCurrent() {
    if (!params.selectedManifest.value || !params.selectedEnvId.value) return;
    const ok = params.validateCurrent();
    if (!ok) return;
    applying.value = true;
    params.setOpError(null);
    params.setOpMessage(null);
    try {
      const identity = params.parseIdentity(params.editYaml.value);
      if (!identity) throw new Error("无法解析资源身份信息。");
      await params.deployYamlToEnv(params.selectedEnvId.value, params.editYaml.value);
      params.setManifestComponent(params.selectedManifest.value.id, params.selectedComponent.value);
      params.setManifestIdentity(params.selectedManifest.value.id, identity);
      params.saveManifestYaml(params.selectedManifest.value.id, params.editYaml.value, "apply");
      params.clearManifestDraft(params.selectedManifest.value.id);
      params.setOpMessage(`应用成功：${identity.kind}/${identity.name}`);
    } catch (e) {
      params.setOpError(extractErrorMessage(e));
    } finally {
      applying.value = false;
    }
  }

  function openComponentApplyFlow() {
    componentApplyItems.value = buildComponentApplyItems();
    componentApplyPhase.value = "idle";
    componentApplyDialogVisible.value = true;
  }

  function closeComponentApplyDialog() {
    if (componentApplyPhase.value === "applying") return;
    componentApplyDialogVisible.value = false;
  }

  async function onApplyComponent() {
    if (!params.selectedEnvId.value || !params.selectedComponent.value) return;
    if (!componentApplyItems.value.length) {
      componentApplyItems.value = buildComponentApplyItems();
    }
    if (!componentApplyItems.value.length) return;
    applying.value = true;
    componentApplyPhase.value = "applying";
    params.setOpError(null);
    params.setOpMessage(null);
    const failed: string[] = [];
    for (const item of componentApplyItems.value) {
      item.status = "running";
      item.error = null;
      try {
        await params.deployYamlToEnv(params.selectedEnvId.value, item.yaml);
        params.saveManifestYaml(item.manifestId, item.yaml, "apply");
        params.clearManifestDraft(item.manifestId);
        item.status = "success";
      } catch (e) {
        const message = extractErrorMessage(e);
        item.status = "failed";
        item.error = message;
        failed.push(`${item.kind}/${item.name}: ${message}`);
      }
    }
    applying.value = false;
    componentApplyPhase.value = "completed";
    if (failed.length) {
      params.setOpError(`组件应用部分失败：${failed.join("；")}`);
    } else {
      params.setOpMessage(`组件 ${params.selectedComponent.value} 已完成应用。`);
    }
  }

  function openApplyDialog() {
    if (!canOpenApplyDialog.value || applying.value) return;
    applyDialogVisible.value = true;
  }

  function closeApplyDialog() {
    applyDialogVisible.value = false;
  }

  async function onApplyCurrentFromDialog() {
    if (!canApplyCurrent.value || applying.value) return;
    applyDialogVisible.value = false;
    await onApplyCurrent();
  }

  async function onApplyComponentFromDialog() {
    if (!canApplyComponent.value || applying.value) return;
    applyDialogVisible.value = false;
    openComponentApplyFlow();
  }

  async function startComponentApplyFromDialog() {
    if (applying.value || !componentApplyItems.value.length) return;
    componentApplyItems.value = componentApplyItems.value.map((item) => ({
      ...item,
      status: "pending",
      error: null,
    }));
    await onApplyComponent();
  }

  return {
    applying,
    applyDialogVisible,
    componentApplyDialogVisible,
    componentApplyItems,
    componentApplyPhase,
    canApplyCurrent,
    canApplyComponent,
    canOpenApplyDialog,
    componentApplySummary,
    onApplyCurrent,
    openComponentApplyFlow,
    closeComponentApplyDialog,
    onApplyComponent,
    openApplyDialog,
    closeApplyDialog,
    onApplyCurrentFromDialog,
    onApplyComponentFromDialog,
    startComponentApplyFromDialog,
  };
}
