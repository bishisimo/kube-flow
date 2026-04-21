/**
 * 编排中心状态：维护 YAML 资产、组件归类、历史快照与页面切换请求。
 * 应用包管理（Package / PackageVersion / Deployment）已迁移至 orchestratorPackages.ts。
 */
import { ref } from "vue";
import { createStorage } from "../utils/storage";
import {
  uid,
  nowIso,
  batchLabel,
  pushHistory,
  normalizeComponent,
  sanitizeYamlForSync,
} from "./orchestratorUtils";

export type {
  OrchestratorResourceRef,
  OrchestratorImportResourceInput,
  ManifestHistoryItem,
  OrchestratorManifest,
  OrchestratorImportBatch,
  OrchestratorPackageResourceSnapshot,
  OrchestratorPackageVersion,
  OrchestratorPackageDeploymentRecord,
  OrchestratorPackage,
  OrchestratorFocusTarget,
} from "./orchestratorTypes";

import type {
  OrchestratorManifest,
  OrchestratorImportBatch,
  OrchestratorResourceRef,
  OrchestratorImportResourceInput,
  ManifestHistoryItem,
  OrchestratorFocusTarget,
} from "./orchestratorTypes";

// ── Storage ───────────────────────────────────────────────────────────────────

export const manifestStorage = createStorage<OrchestratorManifest[]>({
  key: "kube-flow:orchestrator:manifests",
  version: 1,
  fallback: [],
  migrate: (old) => {
    const arr = Array.isArray(old) ? old : [];
    return arr.filter((m) => m && typeof m === "object") as OrchestratorManifest[];
  },
});

const batchStorage = createStorage<OrchestratorImportBatch[]>({
  key: "kube-flow:orchestrator:import-batches",
  version: 1,
  fallback: [],
  migrate: (old) => {
    const arr = Array.isArray(old) ? old : [];
    return arr.filter((item) => item && typeof item === "object") as OrchestratorImportBatch[];
  },
});

// ── State ─────────────────────────────────────────────────────────────────────

export const manifests = ref<OrchestratorManifest[]>(manifestStorage.read());
const importBatches = ref<OrchestratorImportBatch[]>(batchStorage.read());
const switchToOrchestratorRequested = ref(0);
const orchestratorFocusTarget = ref<OrchestratorFocusTarget | null>(null);

// ── Manifest Map index（O(1) 查找） ───────────────────────────────────────────

export function manifestKey(envId: string, kind: string, name: string, ns: string | null): string {
  return `${envId}|${kind}|${name}|${ns ?? ""}`;
}

export const manifestIndex = new Map<string, OrchestratorManifest>();

export function rebuildManifestIndex() {
  manifestIndex.clear();
  for (const m of manifests.value) {
    manifestIndex.set(manifestKey(m.env_id, m.resource_kind, m.resource_name, m.resource_namespace ?? null), m);
  }
}

rebuildManifestIndex();

// ── Manifest CRUD ─────────────────────────────────────────────────────────────

function upsertFromWorkbenchSync(
  envId: string,
  envName: string,
  resource: OrchestratorResourceRef,
  yaml: string,
  componentHint?: string | null
): OrchestratorManifest {
  const sanitizedYaml = sanitizeYamlForSync(yaml);
  const component = normalizeComponent(componentHint ?? resource.name);
  const now = nowIso();
  const key = manifestKey(envId, resource.kind, resource.name, resource.namespace ?? null);
  const existing = manifestIndex.get(key);
  if (existing) {
    existing.env_name = envName;
    existing.component = component;
    existing.yaml = sanitizedYaml;
    existing.updated_at = now;
    existing.history = pushHistory(existing.history, "sync", sanitizedYaml);
    manifestStorage.write(manifests.value);
    return existing;
  }

  const item: OrchestratorManifest = {
    id: uid("manifest"),
    env_id: envId,
    env_name: envName,
    component,
    resource_kind: resource.kind,
    resource_name: resource.name,
    resource_namespace: resource.namespace ?? null,
    yaml: sanitizedYaml,
    created_at: now,
    updated_at: now,
    history: pushHistory([], "sync", sanitizedYaml),
    source_type: "sync_from_workbench",
    source_batch_id: null,
    source_file_name: null,
    source_doc_index: null,
  };
  manifests.value = [item, ...manifests.value];
  rebuildManifestIndex();
  manifestStorage.write(manifests.value);
  return item;
}

function saveManifestYaml(id: string, yaml: string, action: ManifestHistoryItem["action"] = "save"): boolean {
  const target = manifests.value.find((m) => m.id === id);
  if (!target) return false;
  if (action === "save" && target.yaml === yaml) return true;
  target.yaml = yaml;
  target.updated_at = nowIso();
  target.history = pushHistory(target.history, action, yaml);
  manifestStorage.write(manifests.value);
  return true;
}

function setManifestComponent(id: string, component: string): boolean {
  const target = manifests.value.find((m) => m.id === id);
  if (!target) return false;
  target.component = normalizeComponent(component);
  target.updated_at = nowIso();
  manifestStorage.write(manifests.value);
  return true;
}

function createManifestDraft(envId: string, envName: string, component: string, yaml = ""): OrchestratorManifest {
  const now = nowIso();
  const item: OrchestratorManifest = {
    id: uid("manifest"),
    env_id: envId,
    env_name: envName,
    component: normalizeComponent(component),
    resource_kind: "Unknown",
    resource_name: "unnamed",
    resource_namespace: null,
    yaml,
    created_at: now,
    updated_at: now,
    history: yaml ? [{ id: uid("hist"), at: now, action: "save", yaml }] : [],
    source_type: "manual",
    source_batch_id: null,
    source_file_name: null,
    source_doc_index: null,
  };
  manifests.value = [item, ...manifests.value];
  rebuildManifestIndex();
  manifestStorage.write(manifests.value);
  return item;
}

function setManifestIdentity(
  id: string,
  identity: { kind: string; name: string; namespace: string | null }
): boolean {
  const target = manifests.value.find((m) => m.id === id);
  if (!target) return false;
  target.resource_kind = identity.kind;
  target.resource_name = identity.name;
  target.resource_namespace = identity.namespace;
  target.updated_at = nowIso();
  rebuildManifestIndex();
  manifestStorage.write(manifests.value);
  return true;
}

function deleteManifest(id: string) {
  manifests.value = manifests.value.filter((m) => m.id !== id);
  rebuildManifestIndex();
  manifestStorage.write(manifests.value);
}

function importManifestsToEnv(
  envId: string,
  envName: string,
  resources: OrchestratorImportResourceInput[],
  overwrite = true,
  batchMeta?: {
    name?: string;
    source_kind?: "file" | "text";
    file_count?: number;
    document_count?: number;
    error_count?: number;
    warning_count?: number;
    component?: string;
  }
): { created: number; updated: number; skipped: number; manifestIds: string[]; batchId: string | null } {
  let created = 0;
  let updated = 0;
  let skipped = 0;
  const manifestIds: string[] = [];
  const now = nowIso();
  const batchId = batchMeta ? uid("batch") : null;
  const sourceType: OrchestratorManifest["source_type"] =
    batchMeta?.source_kind === "text" ? "import_text" : "import_file";

  if (batchId) {
    const meta = batchMeta!;
    const fileCount = Math.max(1, meta.file_count ?? 1);
    const documentCount = Math.max(resources.length, meta.document_count ?? resources.length);
    importBatches.value = [
      {
        id: batchId,
        env_id: envId,
        env_name: envName,
        name: meta.name?.trim() || batchLabel(meta.source_kind ?? "file", new Date(now)),
        source_kind: meta.source_kind ?? "file",
        file_count: fileCount,
        document_count: documentCount,
        resource_count: resources.length,
        error_count: Math.max(0, meta.error_count ?? 0),
        warning_count: Math.max(0, meta.warning_count ?? 0),
        created_at: now,
        strategy_snapshot: {
          component: normalizeComponent(meta.component ?? resources[0]?.component ?? "default"),
          overwrite,
        },
        summary: `导入 ${resources.length} 个资源，来源 ${fileCount} 个文件。`,
      },
      ...importBatches.value,
    ].slice(0, 100);
    batchStorage.write(importBatches.value);
  }

  for (const resource of resources) {
    const component = normalizeComponent(resource.component);
    const key = manifestKey(envId, resource.kind, resource.name, resource.namespace ?? null);
    const existing = manifestIndex.get(key);

    if (existing) {
      if (!overwrite) { skipped += 1; continue; }
      existing.env_name = envName;
      existing.component = component;
      existing.resource_kind = resource.kind;
      existing.resource_name = resource.name;
      existing.resource_namespace = resource.namespace ?? null;
      existing.yaml = resource.yaml;
      existing.updated_at = now;
      existing.history = pushHistory(existing.history, "save", resource.yaml);
      existing.source_type = sourceType;
      existing.source_batch_id = batchId;
      existing.source_file_name = resource.source_file_name ?? null;
      existing.source_doc_index = resource.source_doc_index ?? null;
      manifestIds.push(existing.id);
      updated += 1;
      continue;
    }

    const item: OrchestratorManifest = {
      id: uid("manifest"),
      env_id: envId,
      env_name: envName,
      component,
      resource_kind: resource.kind,
      resource_name: resource.name,
      resource_namespace: resource.namespace ?? null,
      yaml: resource.yaml,
      created_at: now,
      updated_at: now,
      history: pushHistory([], "save", resource.yaml),
      source_type: sourceType,
      source_batch_id: batchId,
      source_file_name: resource.source_file_name ?? null,
      source_doc_index: resource.source_doc_index ?? null,
    };
    manifests.value = [item, ...manifests.value];
    manifestIds.push(item.id);
    created += 1;
  }

  rebuildManifestIndex();
  manifestStorage.write(manifests.value);
  return { created, updated, skipped, manifestIds, batchId };
}

function requestSwitchToOrchestrator(target?: OrchestratorFocusTarget | null) {
  orchestratorFocusTarget.value = target ?? null;
  switchToOrchestratorRequested.value += 1;
}

export function useOrchestratorStore() {
  return {
    manifests,
    importBatches,
    switchToOrchestratorRequested,
    orchestratorFocusTarget,
    requestSwitchToOrchestrator,
    upsertFromWorkbenchSync,
    saveManifestYaml,
    setManifestComponent,
    createManifestDraft,
    setManifestIdentity,
    deleteManifest,
    importManifestsToEnv,
  };
}
