/**
 * 编排中心状态：维护 YAML 资产、组件归类、历史快照与页面切换请求。
 * 应用包管理（Package / PackageVersion / Deployment）已迁移至 orchestratorPackages.ts。
 */
import { ref } from "vue";
import {
  queueOrchestratorPersist,
  type OrchestratorPersistedData,
} from "./orchestratorPersistence";
import {
  uid,
  nowIso,
  batchLabel,
  pushHistory,
  normalizeComponent,
  sanitizeYamlForSync,
  isClusterScopedKind,
  normalizeInstallNamespace,
  rewriteYamlNamespace,
  inferNamespaceFromResources,
} from "./orchestratorUtils";

export type {
  OrchestratorComponentMeta,
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
  OrchestratorComponentMeta,
  OrchestratorManifest,
  OrchestratorImportBatch,
  OrchestratorResourceRef,
  OrchestratorImportResourceInput,
  ManifestHistoryItem,
  OrchestratorFocusTarget,
} from "./orchestratorTypes";

// ── State ─────────────────────────────────────────────────────────────────────

export const manifests = ref<OrchestratorManifest[]>([]);
export const importBatches = ref<OrchestratorImportBatch[]>([]);
export const componentMetas = ref<OrchestratorComponentMeta[]>([]);
const switchToOrchestratorRequested = ref(0);
const orchestratorFocusTarget = ref<OrchestratorFocusTarget | null>(null);

// ── Manifest Map index（O(1) 查找） ───────────────────────────────────────────

export function manifestKey(envId: string, kind: string, name: string, ns: string | null): string {
  return `${envId}|${kind}|${name}|${ns ?? ""}`;
}

export function componentMetaKey(envId: string, name: string): string {
  return `${envId}|${normalizeComponent(name)}`;
}

export const manifestIndex = new Map<string, OrchestratorManifest>();

export function rebuildManifestIndex() {
  manifestIndex.clear();
  for (const m of manifests.value) {
    manifestIndex.set(manifestKey(m.env_id, m.resource_kind, m.resource_name, m.resource_namespace ?? null), m);
  }
}

rebuildManifestIndex();

function persistOrchestratorSnapshot() {
  queueOrchestratorPersist();
}

function findComponentMeta(envId: string, name: string): OrchestratorComponentMeta | undefined {
  const key = componentMetaKey(envId, name);
  return componentMetas.value.find((c) => componentMetaKey(c.env_id, c.name) === key);
}

/** 读取组件安装命名空间；无元数据时回退 default。 */
export function getComponentNamespace(envId: string, name: string): string {
  return findComponentMeta(envId, name)?.namespace ?? "default";
}

function upsertComponentMetaRecord(envId: string, name: string, namespace: string): OrchestratorComponentMeta {
  const normalizedName = normalizeComponent(name);
  const ns = normalizeInstallNamespace(namespace);
  const existing = findComponentMeta(envId, normalizedName);
  if (existing) {
    existing.namespace = ns;
    existing.name = normalizedName;
    return existing;
  }
  const created: OrchestratorComponentMeta = {
    env_id: envId,
    name: normalizedName,
    namespace: ns,
  };
  componentMetas.value = [created, ...componentMetas.value];
  return created;
}

/**
 * 确保组件元数据存在。
 * 已有元数据时保留其命名空间；新建时使用 namespaceHint（或 default）。
 */
export function ensureComponentMeta(
  envId: string,
  name: string,
  namespaceHint?: string | null
): OrchestratorComponentMeta {
  const existing = findComponentMeta(envId, name);
  if (existing) return existing;
  return upsertComponentMetaRecord(envId, name, namespaceHint ?? "default");
}

function applyNamespaceToManifest(manifest: OrchestratorManifest, namespace: string) {
  if (isClusterScopedKind(manifest.resource_kind)) {
    manifest.resource_namespace = null;
    if (manifest.yaml.trim()) {
      manifest.yaml = rewriteYamlNamespace(manifest.yaml, namespace, manifest.resource_kind);
    }
    return;
  }
  const ns = normalizeInstallNamespace(namespace);
  manifest.resource_namespace = ns;
  if (manifest.yaml.trim()) {
    manifest.yaml = rewriteYamlNamespace(manifest.yaml, ns, manifest.resource_kind);
  }
}

/** 设置组件安装命名空间，并改写该组件下全部 namespaced 资源的 YAML / 身份。 */
export function setComponentNamespace(envId: string, name: string, namespace: string): string {
  const ns = normalizeInstallNamespace(namespace);
  const component = normalizeComponent(name);
  upsertComponentMetaRecord(envId, component, ns);
  const now = nowIso();
  for (const m of manifests.value) {
    if (m.env_id !== envId || m.component !== component) continue;
    applyNamespaceToManifest(m, ns);
    m.updated_at = now;
  }
  rebuildManifestIndex();
  persistOrchestratorSnapshot();
  return ns;
}

export function deleteComponentMeta(envId: string, name: string) {
  const key = componentMetaKey(envId, name);
  const before = componentMetas.value.length;
  componentMetas.value = componentMetas.value.filter((c) => componentMetaKey(c.env_id, c.name) !== key);
  if (componentMetas.value.length !== before) persistOrchestratorSnapshot();
}

/** 复制组件元数据到目标环境（目标已有则保留，除非 overwrite）。 */
export function copyComponentMetaToEnv(
  sourceEnvId: string,
  component: string,
  targetEnvId: string,
  overwrite = true
) {
  const source = findComponentMeta(sourceEnvId, component);
  if (!source) return;
  const existing = findComponentMeta(targetEnvId, component);
  if (existing && !overwrite) return;
  upsertComponentMetaRecord(targetEnvId, component, source.namespace);
}

function migrateComponentMetasFromManifests(existing: OrchestratorComponentMeta[]): OrchestratorComponentMeta[] {
  const byKey = new Map<string, OrchestratorComponentMeta>();
  for (const c of existing) {
    if (!c || typeof c !== "object") continue;
    const envId = typeof c.env_id === "string" ? c.env_id : "";
    const name = typeof c.name === "string" ? normalizeComponent(c.name) : "";
    if (!envId || !name) continue;
    byKey.set(componentMetaKey(envId, name), {
      env_id: envId,
      name,
      namespace: normalizeInstallNamespace(c.namespace),
    });
  }
  const groups = new Map<string, OrchestratorManifest[]>();
  for (const m of manifests.value) {
    const key = componentMetaKey(m.env_id, m.component);
    const list = groups.get(key) ?? [];
    list.push(m);
    groups.set(key, list);
  }
  for (const [key, list] of groups) {
    if (byKey.has(key)) continue;
    const sample = list[0];
    const namespace = inferNamespaceFromResources(
      list.map((m) => ({ kind: m.resource_kind, namespace: m.resource_namespace }))
    );
    byKey.set(key, {
      env_id: sample.env_id,
      name: normalizeComponent(sample.component),
      namespace,
    });
  }
  return Array.from(byKey.values());
}

function prepareNamespacedYaml(
  envId: string,
  component: string,
  kind: string,
  yaml: string,
  namespaceHint?: string | null
): { yaml: string; namespace: string | null } {
  const meta = ensureComponentMeta(envId, component, namespaceHint);
  if (isClusterScopedKind(kind)) {
    return {
      yaml: rewriteYamlNamespace(yaml, meta.namespace, kind),
      namespace: null,
    };
  }
  return {
    yaml: rewriteYamlNamespace(yaml, meta.namespace, kind),
    namespace: meta.namespace,
  };
}

export function applyOrchestratorHydration(
  data: Pick<OrchestratorPersistedData, "manifests" | "importBatches" | "components">
) {
  manifests.value = data.manifests;
  importBatches.value = data.importBatches;
  componentMetas.value = migrateComponentMetasFromManifests(data.components ?? []);
  rebuildManifestIndex();
}

export function getOrchestratorPersistSnapshot(): Pick<
  OrchestratorPersistedData,
  "manifests" | "importBatches" | "components"
> {
  return {
    manifests: manifests.value,
    importBatches: importBatches.value,
    components: componentMetas.value,
  };
}

// ── Manifest CRUD ─────────────────────────────────────────────────────────────

function upsertFromWorkbenchSync(
  envId: string,
  envName: string,
  resource: OrchestratorResourceRef,
  yaml: string,
  componentHint?: string | null
): OrchestratorManifest {
  const component = normalizeComponent(componentHint ?? resource.name);
  const sanitizedYaml = sanitizeYamlForSync(yaml);
  const prepared = prepareNamespacedYaml(
    envId,
    component,
    resource.kind,
    sanitizedYaml,
    resource.namespace
  );
  const now = nowIso();
  const key = manifestKey(envId, resource.kind, resource.name, prepared.namespace);
  const existing = manifestIndex.get(key);
  if (existing) {
    existing.env_name = envName;
    existing.component = component;
    existing.resource_namespace = prepared.namespace;
    existing.yaml = prepared.yaml;
    existing.updated_at = now;
    existing.history = pushHistory(existing.history, "sync", prepared.yaml);
    persistOrchestratorSnapshot();
    rebuildManifestIndex();
    return existing;
  }

  // 同 kind/name 可能仍挂在旧命名空间键上（组件 ns 变更前），优先合并。
  const legacy = manifests.value.find(
    (m) =>
      m.env_id === envId &&
      m.resource_kind === resource.kind &&
      m.resource_name === resource.name &&
      m.component === component
  );
  if (legacy) {
    legacy.env_name = envName;
    legacy.resource_namespace = prepared.namespace;
    legacy.yaml = prepared.yaml;
    legacy.updated_at = now;
    legacy.history = pushHistory(legacy.history, "sync", prepared.yaml);
    rebuildManifestIndex();
    persistOrchestratorSnapshot();
    return legacy;
  }

  const item: OrchestratorManifest = {
    id: uid("manifest"),
    env_id: envId,
    env_name: envName,
    component,
    resource_kind: resource.kind,
    resource_name: resource.name,
    resource_namespace: prepared.namespace,
    yaml: prepared.yaml,
    created_at: now,
    updated_at: now,
    history: pushHistory([], "sync", prepared.yaml),
    source_type: "sync_from_workbench",
    source_batch_id: null,
    source_file_name: null,
    source_doc_index: null,
  };
  manifests.value = [item, ...manifests.value];
  rebuildManifestIndex();
  persistOrchestratorSnapshot();
  return item;
}

function saveManifestYaml(id: string, yaml: string, action: ManifestHistoryItem["action"] = "save"): boolean {
  const target = manifests.value.find((m) => m.id === id);
  if (!target) return false;
  if (action === "save" && target.yaml === yaml) return true;
  target.yaml = yaml;
  target.updated_at = nowIso();
  target.history = pushHistory(target.history, action, yaml);
  persistOrchestratorSnapshot();
  return true;
}

function setManifestComponent(id: string, component: string): boolean {
  const target = manifests.value.find((m) => m.id === id);
  if (!target) return false;
  const nextComponent = normalizeComponent(component);
  const ns = getComponentNamespace(target.env_id, nextComponent);
  ensureComponentMeta(target.env_id, nextComponent, target.resource_namespace);
  target.component = nextComponent;
  applyNamespaceToManifest(target, getComponentNamespace(target.env_id, nextComponent) || ns);
  target.updated_at = nowIso();
  rebuildManifestIndex();
  persistOrchestratorSnapshot();
  return true;
}

function createManifestDraft(envId: string, envName: string, component: string, yaml = ""): OrchestratorManifest {
  const now = nowIso();
  const normalized = normalizeComponent(component);
  const meta = ensureComponentMeta(envId, normalized, "default");
  const item: OrchestratorManifest = {
    id: uid("manifest"),
    env_id: envId,
    env_name: envName,
    component: normalized,
    resource_kind: "Unknown",
    resource_name: "unnamed",
    resource_namespace: meta.namespace,
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
  persistOrchestratorSnapshot();
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
  if (isClusterScopedKind(identity.kind)) {
    target.resource_namespace = null;
  } else {
    const ns = getComponentNamespace(target.env_id, target.component);
    target.resource_namespace = ns;
  }
  target.updated_at = nowIso();
  rebuildManifestIndex();
  persistOrchestratorSnapshot();
  return true;
}

function deleteManifest(id: string) {
  manifests.value = manifests.value.filter((m) => m.id !== id);
  rebuildManifestIndex();
  persistOrchestratorSnapshot();
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
    persistOrchestratorSnapshot();
  }

  const componentHints = new Map<string, string | null>();
  for (const resource of resources) {
    const component = normalizeComponent(resource.component);
    if (!componentHints.has(component)) {
      componentHints.set(component, resource.namespace);
    }
  }
  for (const [component, hint] of componentHints) {
    ensureComponentMeta(envId, component, hint);
  }

  for (const resource of resources) {
    const component = normalizeComponent(resource.component);
    const prepared = prepareNamespacedYaml(
      envId,
      component,
      resource.kind,
      resource.yaml,
      resource.namespace
    );
    const key = manifestKey(envId, resource.kind, resource.name, prepared.namespace);
    const existing =
      manifestIndex.get(key) ??
      manifests.value.find(
        (m) =>
          m.env_id === envId &&
          m.resource_kind === resource.kind &&
          m.resource_name === resource.name &&
          m.component === component
      );

    if (existing) {
      if (!overwrite) {
        skipped += 1;
        continue;
      }
      existing.env_name = envName;
      existing.component = component;
      existing.resource_kind = resource.kind;
      existing.resource_name = resource.name;
      existing.resource_namespace = prepared.namespace;
      existing.yaml = prepared.yaml;
      existing.updated_at = now;
      existing.history = pushHistory(existing.history, "save", prepared.yaml);
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
      resource_namespace: prepared.namespace,
      yaml: prepared.yaml,
      created_at: now,
      updated_at: now,
      history: pushHistory([], "save", prepared.yaml),
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
  persistOrchestratorSnapshot();
  return { created, updated, skipped, manifestIds, batchId };
}

function clearImportBatchesRecords() {
  importBatches.value = [];
  persistOrchestratorSnapshot();
}

function requestSwitchToOrchestrator(target?: OrchestratorFocusTarget | null) {
  orchestratorFocusTarget.value = target ?? null;
  switchToOrchestratorRequested.value += 1;
}

export function useOrchestratorStore() {
  return {
    manifests,
    importBatches,
    componentMetas,
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
    clearImportBatchesRecords,
    getComponentNamespace,
    setComponentNamespace,
    ensureComponentMeta,
    deleteComponentMeta,
    copyComponentMetaToEnv,
  };
}
