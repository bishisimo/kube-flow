import { computed, ref, type ComputedRef, type Ref } from "vue";
import * as jsYaml from "js-yaml";
import { extractErrorMessage } from "../../utils/errorMessage";
import type { OrchestratorManifest } from "../../stores/orchestrator";
import type { KubeObjectIdentity } from "../../utils/yaml";

export interface ImportPreviewItem {
  id: string;
  fileName: string;
  docIndex: number;
  kind: string;
  name: string;
  namespace: string | null;
  yaml: string;
  valid: boolean;
  errors: string[];
  warnings: string[];
  conflict: boolean;
  duplicate: boolean;
}

interface KnownResource {
  kind: string;
  name: string;
  namespace: string | null;
}

export function useOrchestratorImportPreview(params: {
  selectedEnvId: Ref<string>;
  manifestsByEnv: ComputedRef<OrchestratorManifest[]>;
  parseIdentity: (yaml: string) => KubeObjectIdentity | null;
}) {
  const importLoading = ref(false);
  const importComponent = ref("");
  const importOverwrite = ref(true);
  const importPreviewItems = ref<ImportPreviewItem[]>([]);
  const importSummaryMessage = ref<string | null>(null);
  const importFileInput = ref<HTMLInputElement | null>(null);
  const importTextDraft = ref("");
  const importParseTimer = ref<number | null>(null);

  const importValidItems = computed(() => importPreviewItems.value.filter((item) => item.valid));
  const importInvalidItems = computed(() => importPreviewItems.value.filter((item) => !item.valid));
  const importWarningItems = computed(() => importValidItems.value.filter((item) => item.warnings.length > 0));
  const canConfirmImport = computed(
    () => Boolean(params.selectedEnvId.value && importValidItems.value.length && !importLoading.value)
  );

  function resourceIdentityKey(kind: string, name: string, namespace: string | null) {
    return `${kind}|${namespace ?? ""}|${name}`;
  }

  function buildImportYaml(doc: unknown): string {
    return jsYaml.dump(doc, { lineWidth: -1 });
  }

  function makeImportPreviewId(fileName: string, docIndex: number) {
    return `${fileName}:${docIndex}`;
  }

  function existingManifestConflict(kind: string, name: string, namespace: string | null) {
    return params.manifestsByEnv.value.some(
      (m) =>
        m.resource_kind === kind && m.resource_name === name && (m.resource_namespace ?? null) === (namespace ?? null)
    );
  }

  function buildKnownResourceKeySet(items: KnownResource[]) {
    return new Set(items.map((item) => resourceIdentityKey(item.kind, item.name, item.namespace)));
  }

  function extractImportRefWarnings(yaml: string, knownResources: Set<string>): string[] {
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
    const metadata = obj.metadata as Record<string, unknown> | undefined;
    const namespace =
      metadata && typeof metadata.namespace === "string" && metadata.namespace.trim() ? metadata.namespace.trim() : null;

    const checkRef = (
      targetKind: "ConfigMap" | "Secret" | "ServiceAccount" | "PersistentVolumeClaim",
      name: string
    ) => {
      const exact = resourceIdentityKey(targetKind, name, namespace);
      const fallback = resourceIdentityKey(targetKind, name, null);
      if (!knownResources.has(exact) && !knownResources.has(fallback)) {
        warnings.push(`引用资源缺失：${targetKind}/${name}`);
      }
    };

    if (kind === "Deployment" || kind === "StatefulSet" || kind === "DaemonSet") {
      const spec = obj.spec as Record<string, unknown> | undefined;
      const template = spec?.template as Record<string, unknown> | undefined;
      const podSpec = template?.spec as Record<string, unknown> | undefined;
      if (podSpec && typeof podSpec.serviceAccountName === "string" && podSpec.serviceAccountName) {
        checkRef("ServiceAccount", podSpec.serviceAccountName);
      }
      const volumes = Array.isArray(podSpec?.volumes) ? (podSpec?.volumes as Array<Record<string, unknown>>) : [];
      for (const v of volumes) {
        const cm = v.configMap as Record<string, unknown> | undefined;
        if (cm && typeof cm.name === "string" && cm.name) checkRef("ConfigMap", cm.name);
        const sec = v.secret as Record<string, unknown> | undefined;
        if (sec && typeof sec.secretName === "string" && sec.secretName) checkRef("Secret", sec.secretName);
        const pvc = v.persistentVolumeClaim as Record<string, unknown> | undefined;
        if (pvc && typeof pvc.claimName === "string" && pvc.claimName) checkRef("PersistentVolumeClaim", pvc.claimName);
      }
      const containers = [
        ...(Array.isArray(podSpec?.containers) ? (podSpec.containers as Array<Record<string, unknown>>) : []),
        ...(Array.isArray(podSpec?.initContainers) ? (podSpec.initContainers as Array<Record<string, unknown>>) : []),
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
    }

    return Array.from(new Set(warnings));
  }

  function applyImportBatchPrecheck(items: ImportPreviewItem[]): ImportPreviewItem[] {
    const duplicateCounts = new Map<string, number>();
    for (const item of items) {
      if (!item.valid) continue;
      const key = resourceIdentityKey(item.kind, item.name, item.namespace);
      duplicateCounts.set(key, (duplicateCounts.get(key) ?? 0) + 1);
    }

    const knownResources = buildKnownResourceKeySet([
      ...params.manifestsByEnv.value.map((m) => ({
        kind: m.resource_kind,
        name: m.resource_name,
        namespace: m.resource_namespace,
      })),
      ...items
        .filter((item) => item.valid)
        .map((item) => ({
          kind: item.kind,
          name: item.name,
          namespace: item.namespace,
        })),
    ]);

    return items.map((item) => {
      if (!item.valid) return item;
      const key = resourceIdentityKey(item.kind, item.name, item.namespace);
      const duplicate = (duplicateCounts.get(key) ?? 0) > 1;
      const warnings = [...item.warnings];
      if (duplicate) warnings.push("批次内存在重复资源身份，导入后可能相互覆盖。");
      if (item.conflict) warnings.push(`环境内已存在同名资源，导入时将${importOverwrite.value ? "覆盖" : "跳过"}。`);
      warnings.push(...extractImportRefWarnings(item.yaml, knownResources));
      return {
        ...item,
        duplicate,
        warnings: Array.from(new Set(warnings)),
      };
    });
  }

  function refreshImportPreviewChecks() {
    if (!importPreviewItems.value.length) return;
    importPreviewItems.value = applyImportBatchPrecheck(
      importPreviewItems.value.map((item) => ({
        ...item,
        warnings: [],
        conflict: item.valid ? existingManifestConflict(item.kind, item.name, item.namespace) : false,
        duplicate: false,
      }))
    );
  }

  function buildImportPreviewFromText(fileName: string, content: string): ImportPreviewItem[] {
    const docs: unknown[] = [];
    try {
      jsYaml.loadAll(content, (doc) => {
        docs.push(doc);
      });
    } catch (e) {
      return [
        {
          id: makeImportPreviewId(fileName, 1),
          fileName,
          docIndex: 1,
          kind: "-",
          name: "-",
          namespace: null,
          yaml: content,
          valid: false,
          errors: [`YAML 解析失败：${extractErrorMessage(e)}`],
          warnings: [],
          conflict: false,
          duplicate: false,
        },
      ];
    }

    const items: ImportPreviewItem[] = [];
    let docIndex = 0;
    for (const doc of docs) {
      if (doc == null) continue;
      docIndex += 1;
      if (typeof doc !== "object") {
        items.push({
          id: makeImportPreviewId(fileName, docIndex),
          fileName,
          docIndex,
          kind: "-",
          name: "-",
          namespace: null,
          yaml: String(doc),
          valid: false,
          errors: ["该 document 不是对象结构，无法作为 Kubernetes 资源导入。"],
          warnings: [],
          conflict: false,
          duplicate: false,
        });
        continue;
      }
      const yaml = buildImportYaml(doc);
      const identity = params.parseIdentity(yaml);
      if (!identity) {
        items.push({
          id: makeImportPreviewId(fileName, docIndex),
          fileName,
          docIndex,
          kind: "-",
          name: "-",
          namespace: null,
          yaml,
          valid: false,
          errors: ["缺少 kind 或 metadata.name，无法识别资源身份。"],
          warnings: [],
          conflict: false,
          duplicate: false,
        });
        continue;
      }
      const errors: string[] = [];
      const parsed = doc as Record<string, unknown>;
      if (typeof parsed.apiVersion !== "string" || !parsed.apiVersion.trim()) {
        errors.push("缺少 apiVersion。");
      }
      items.push({
        id: makeImportPreviewId(fileName, docIndex),
        fileName,
        docIndex,
        kind: identity.kind,
        name: identity.name,
        namespace: identity.namespace,
        yaml,
        valid: errors.length === 0,
        errors,
        warnings: [],
        conflict: existingManifestConflict(identity.kind, identity.name, identity.namespace),
        duplicate: false,
      });
    }

    if (items.length > 0) return items;
    return [
      {
        id: makeImportPreviewId(fileName, 1),
        fileName,
        docIndex: 1,
        kind: "-",
        name: "-",
        namespace: null,
        yaml: content,
        valid: false,
        errors: ["文件中未解析出可导入的 YAML document。"],
        warnings: [],
        conflict: false,
        duplicate: false,
      },
    ];
  }

  function parseImportTextDraft() {
    const name = "当前草稿";
    const content = importTextDraft.value.trim();
    if (!content) {
      importPreviewItems.value = [];
      importSummaryMessage.value = "请输入 YAML 文本后再解析。";
      return;
    }
    const rows = applyImportBatchPrecheck(buildImportPreviewFromText(name, content));
    importPreviewItems.value = rows;
    importSummaryMessage.value = `已解析当前草稿，共 ${rows.length} 个条目；有效 ${rows.filter((r) => r.valid).length}，无效 ${rows.filter((r) => !r.valid).length}，批次内重复 ${rows.filter((r) => r.duplicate).length}。`;
  }

  async function onImportFilesSelected(event: Event) {
    const input = event.target as HTMLInputElement | null;
    const files = Array.from(input?.files ?? []);
    if (!files.length) return;
    importLoading.value = true;
    try {
      const chunks: string[] = [];
      for (const file of files) {
        const text = await file.text();
        chunks.push(
          [
            "# ========================================",
            `# 导入文件: ${file.name}`,
            "# ========================================",
            text.trim(),
          ].join("\n")
        );
      }
      const merged = chunks.join("\n\n---\n\n");
      importTextDraft.value = importTextDraft.value.trim()
        ? `${importTextDraft.value.trim()}\n\n---\n\n${merged}`
        : merged;
      parseImportTextDraft();
    } finally {
      importLoading.value = false;
      if (input) input.value = "";
    }
  }

  function triggerImportFileSelect() {
    importFileInput.value?.click();
  }

  function openCreateYamlDialog(defaultComponent: string) {
    if (!params.selectedEnvId.value) return false;
    importComponent.value = defaultComponent || "default";
    importOverwrite.value = true;
    importPreviewItems.value = [];
    importSummaryMessage.value = null;
    importTextDraft.value = "";
    return true;
  }

  function clearParseTimer() {
    if (importParseTimer.value) {
      window.clearTimeout(importParseTimer.value);
      importParseTimer.value = null;
    }
  }

  function onDraftVisibilityOrContentChange(visible: boolean, draft: string) {
    clearParseTimer();
    if (!visible) return;
    if (!draft.trim()) {
      importPreviewItems.value = [];
      importSummaryMessage.value = "可以直接编写 YAML，也可以先导入文件到当前草稿。";
      return;
    }
    importSummaryMessage.value = "正在解析当前草稿…";
    importParseTimer.value = window.setTimeout(() => {
      parseImportTextDraft();
      importParseTimer.value = null;
    }, 350);
  }

  function buildImportResources(componentFallback: string) {
    const component = importComponent.value.trim() || componentFallback || "default";
    return {
      component,
      resources: importValidItems.value.map((item) => ({
        component,
        kind: item.kind,
        name: item.name,
        namespace: item.namespace,
        yaml: item.yaml,
        source_file_name: item.fileName,
        source_doc_index: item.docIndex,
      })),
    };
  }

  return {
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
    parseImportTextDraft,
    onImportFilesSelected,
    triggerImportFileSelect,
    openCreateYamlDialog,
    clearParseTimer,
    onDraftVisibilityOrContentChange,
    buildImportResources,
    extractRefWarningsFromInventory: (yaml: string, inventory: OrchestratorManifest[]) => {
      const known = buildKnownResourceKeySet(
        inventory.map((m) => ({
          kind: m.resource_kind,
          name: m.resource_name,
          namespace: m.resource_namespace,
        }))
      );
      return extractImportRefWarnings(yaml, known);
    },
  };
}
