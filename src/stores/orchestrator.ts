/**
 * 编排中心状态：维护 YAML 资产、组件归类、历史快照与页面切换请求。
 */
import { ref } from "vue";
import * as jsYaml from "js-yaml";

export interface OrchestratorResourceRef {
  kind: string;
  name: string;
  namespace: string | null;
}

export interface OrchestratorImportResourceInput {
  component: string;
  kind: string;
  name: string;
  namespace: string | null;
  yaml: string;
  source_file_name?: string | null;
  source_doc_index?: number | null;
}

export interface ManifestHistoryItem {
  id: string;
  at: string;
  action: "sync" | "save" | "apply" | "restore";
  yaml: string;
}

export interface OrchestratorManifest {
  id: string;
  env_id: string;
  env_name: string;
  component: string;
  resource_kind: string;
  resource_name: string;
  resource_namespace: string | null;
  yaml: string;
  created_at: string;
  updated_at: string;
  history: ManifestHistoryItem[];
  source_type?: "manual" | "import_file" | "import_text" | "sync_from_workbench" | "package_sync";
  source_batch_id?: string | null;
  source_file_name?: string | null;
  source_doc_index?: number | null;
}

export interface OrchestratorImportBatch {
  id: string;
  env_id: string;
  env_name: string;
  name: string;
  source_kind: "file" | "text";
  file_count: number;
  document_count: number;
  resource_count: number;
  error_count: number;
  warning_count: number;
  created_at: string;
  strategy_snapshot: {
    component: string;
    overwrite: boolean;
  };
  summary: string;
}

export interface OrchestratorPackageResourceSnapshot {
  id: string;
  source_manifest_id: string;
  component: string;
  resource_kind: string;
  resource_name: string;
  resource_namespace: string | null;
  yaml: string;
}

export interface OrchestratorPackageVersion {
  id: string;
  label: string;
  tag: string | null;
  source_env_id: string;
  source_env_name: string;
  component_names: string[];
  created_at: string;
  resources: OrchestratorPackageResourceSnapshot[];
}

export interface OrchestratorPackageDeploymentRecord {
  id: string;
  at: string;
  package_id: string;
  package_name: string;
  version_id: string;
  version_label: string;
  target_env_id: string;
  target_env_name: string;
  mode: "sync" | "apply";
  total: number;
  success: number;
  failed: number;
  errors: string[];
}

export interface OrchestratorPackage {
  id: string;
  name: string;
  description: string;
  created_at: string;
  updated_at: string;
  versions: OrchestratorPackageVersion[];
  deployments: OrchestratorPackageDeploymentRecord[];
}

export interface OrchestratorFocusTarget {
  env_id: string;
  component: string;
  manifest_id?: string | null;
  resource_kind?: string | null;
  resource_name?: string | null;
  resource_namespace?: string | null;
}

const STORAGE_KEY = "kube-flow:orchestrator:manifests";
const STORAGE_KEY_PACKAGES = "kube-flow:orchestrator:packages";
const STORAGE_KEY_BATCHES = "kube-flow:orchestrator:import-batches";

const manifests = ref<OrchestratorManifest[]>(loadManifests());
const packages = ref<OrchestratorPackage[]>(loadPackages());
const importBatches = ref<OrchestratorImportBatch[]>(loadImportBatches());
const switchToOrchestratorRequested = ref(0);
const orchestratorFocusTarget = ref<OrchestratorFocusTarget | null>(null);

function loadManifests(): OrchestratorManifest[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((m) => m && typeof m === "object") as OrchestratorManifest[];
  } catch {
    return [];
  }
}

function persist() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(manifests.value));
    localStorage.setItem(STORAGE_KEY_PACKAGES, JSON.stringify(packages.value));
    localStorage.setItem(STORAGE_KEY_BATCHES, JSON.stringify(importBatches.value));
  } catch {}
}

function loadPackages(): OrchestratorPackage[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_PACKAGES);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((p) => p && typeof p === "object")
      .map((p) => {
        const pkg = p as OrchestratorPackage;
        const versions = Array.isArray(pkg.versions)
          ? pkg.versions.map((v) => ({
              ...v,
              tag: typeof v.tag === "string" && v.tag.trim() ? v.tag.trim() : null,
            }))
          : [];
        return {
          ...pkg,
          versions,
          deployments: Array.isArray(pkg.deployments) ? pkg.deployments : [],
        } as OrchestratorPackage;
      });
  } catch {
    return [];
  }
}

function loadImportBatches(): OrchestratorImportBatch[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_BATCHES);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((item) => item && typeof item === "object") as OrchestratorImportBatch[];
  } catch {
    return [];
  }
}

function uid(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function nowIso(): string {
  return new Date().toISOString();
}

function formatRfc3339Local(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  const year = date.getFullYear();
  const month = pad(date.getMonth() + 1);
  const day = pad(date.getDate());
  const hours = pad(date.getHours());
  const minutes = pad(date.getMinutes());
  const seconds = pad(date.getSeconds());
  const offsetMinutes = -date.getTimezoneOffset();
  const sign = offsetMinutes >= 0 ? "+" : "-";
  const offsetHours = pad(Math.floor(Math.abs(offsetMinutes) / 60));
  const offsetRemainMinutes = pad(Math.abs(offsetMinutes) % 60);
  return `${year}-${month}-${day}T${hours}:${minutes}:${seconds}${sign}${offsetHours}:${offsetRemainMinutes}`;
}

function batchLabel(sourceKind: "file" | "text", now = new Date()): string {
  const prefix = sourceKind === "text" ? "创建" : "导入";
  return `${formatRfc3339Local(now)} ${prefix}`;
}

function buildHistory(action: ManifestHistoryItem["action"], yaml: string): ManifestHistoryItem {
  return {
    id: uid("hist"),
    at: nowIso(),
    action,
    yaml,
  };
}

function pushHistory(
  history: ManifestHistoryItem[],
  action: ManifestHistoryItem["action"],
  yaml: string
): ManifestHistoryItem[] {
  const latest = history[0];
  if (latest && latest.action === action && latest.yaml === yaml) {
    return history;
  }
  return [buildHistory(action, yaml), ...history].slice(0, 30);
}

function normalizeComponent(component: string): string {
  const v = component.trim();
  return v || "default";
}

function sanitizeYamlForSync(yaml: string): string {
  try {
    const parsed = jsYaml.load(yaml);
    if (!parsed || typeof parsed !== "object") return yaml;
    const obj = parsed as Record<string, unknown>;
    const next: Record<string, unknown> = { ...obj };

    const meta =
      next.metadata && typeof next.metadata === "object"
        ? ({ ...(next.metadata as Record<string, unknown>) } as Record<string, unknown>)
        : null;
    if (meta) {
      delete meta.managedFields;
      delete meta.generation;
      delete meta.resourceVersion;
      delete meta.uid;
      delete meta.creationTimestamp;
      next.metadata = meta;
    }

    delete next.status;
    return jsYaml.dump(next, { lineWidth: -1 });
  } catch {
    return yaml;
  }
}

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
  const existing = manifests.value.find(
    (m) =>
      m.env_id === envId &&
      m.resource_kind === resource.kind &&
      m.resource_name === resource.name &&
      (m.resource_namespace ?? null) === (resource.namespace ?? null)
  );
  if (existing) {
    existing.env_name = envName;
    existing.component = component;
    existing.yaml = sanitizedYaml;
    existing.updated_at = now;
    existing.history = pushHistory(existing.history, "sync", sanitizedYaml);
    persist();
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
  persist();
  return item;
}

function saveManifestYaml(id: string, yaml: string, action: ManifestHistoryItem["action"] = "save"): boolean {
  const target = manifests.value.find((m) => m.id === id);
  if (!target) return false;
  if (action === "save" && target.yaml === yaml) {
    return true;
  }
  target.yaml = yaml;
  target.updated_at = nowIso();
  target.history = pushHistory(target.history, action, yaml);
  persist();
  return true;
}

function setManifestComponent(id: string, component: string): boolean {
  const target = manifests.value.find((m) => m.id === id);
  if (!target) return false;
  target.component = normalizeComponent(component);
  target.updated_at = nowIso();
  persist();
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
    history: yaml ? [buildHistory("save", yaml)] : [],
    source_type: "manual",
    source_batch_id: null,
    source_file_name: null,
    source_doc_index: null,
  };
  manifests.value = [item, ...manifests.value];
  persist();
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
  persist();
  return true;
}

function deleteManifest(id: string) {
  manifests.value = manifests.value.filter((m) => m.id !== id);
  persist();
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
  }

  for (const resource of resources) {
    const component = normalizeComponent(resource.component);
    const existing = manifests.value.find(
      (m) =>
        m.env_id === envId &&
        m.resource_kind === resource.kind &&
        m.resource_name === resource.name &&
        (m.resource_namespace ?? null) === (resource.namespace ?? null)
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

  persist();
  return { created, updated, skipped, manifestIds, batchId };
}

function createPackage(name: string, description = ""): OrchestratorPackage {
  const trimmedName = name.trim();
  if (!trimmedName) {
    throw new Error("应用包名称不能为空。");
  }
  if (packages.value.some((p) => p.name === trimmedName)) {
    throw new Error(`应用包已存在：${trimmedName}`);
  }
  const now = nowIso();
  const next: OrchestratorPackage = {
    id: uid("pkg"),
    name: trimmedName,
    description: description.trim(),
    created_at: now,
    updated_at: now,
    versions: [],
    deployments: [],
  };
  packages.value = [next, ...packages.value];
  persist();
  return next;
}

function deletePackage(id: string): boolean {
  const before = packages.value.length;
  packages.value = packages.value.filter((p) => p.id !== id);
  const changed = before !== packages.value.length;
  if (changed) persist();
  return changed;
}

function pad2(n: number): string {
  return String(n).padStart(2, "0");
}

function buildVersionLabelFromDate(d: Date): string {
  const y = d.getFullYear();
  const m = pad2(d.getMonth() + 1);
  const day = pad2(d.getDate());
  const hh = pad2(d.getHours());
  const mm = pad2(d.getMinutes());
  const ss = pad2(d.getSeconds());
  return `${y}${m}${day}-${hh}${mm}${ss}`;
}

function createPackageVersion(
  packageId: string,
  sourceEnvId: string,
  sourceEnvName: string,
  componentNames: string[]
): OrchestratorPackageVersion {
  const pkg = packages.value.find((p) => p.id === packageId);
  if (!pkg) throw new Error("未找到应用包。");
  const normalizedComponents = Array.from(new Set(componentNames.map((c) => normalizeComponent(c))));
  if (!normalizedComponents.length) {
    throw new Error("至少选择一个组件。");
  }
  const source = manifests.value
    .filter((m) => m.env_id === sourceEnvId && normalizedComponents.includes(m.component))
    .sort((a, b) => {
      if (a.component !== b.component) return a.component.localeCompare(b.component);
      if (a.resource_kind !== b.resource_kind) return a.resource_kind.localeCompare(b.resource_kind);
      return a.resource_name.localeCompare(b.resource_name);
    });
  if (!source.length) {
    throw new Error("所选组件没有可打包资源。");
  }
  const createdAt = nowIso();
  const version: OrchestratorPackageVersion = {
    id: uid("pkgv"),
    label: buildVersionLabelFromDate(new Date(createdAt)),
    tag: null,
    source_env_id: sourceEnvId,
    source_env_name: sourceEnvName,
    component_names: normalizedComponents,
    created_at: createdAt,
    resources: source.map((m) => ({
      id: uid("pkgr"),
      source_manifest_id: m.id,
      component: m.component,
      resource_kind: m.resource_kind,
      resource_name: m.resource_name,
      resource_namespace: m.resource_namespace,
      yaml: m.yaml,
    })),
  };
  pkg.versions = [version, ...pkg.versions];
  pkg.updated_at = nowIso();
  persist();
  return version;
}

function findPackageVersion(
  packageId: string,
  versionId: string
): { pkg: OrchestratorPackage; version: OrchestratorPackageVersion } | null {
  const pkg = packages.value.find((p) => p.id === packageId);
  if (!pkg) return null;
  const version = pkg.versions.find((v) => v.id === versionId);
  if (!version) return null;
  return { pkg, version };
}

function setPackageVersionTag(packageId: string, versionId: string, tag: string): boolean {
  const found = findPackageVersion(packageId, versionId);
  if (!found) return false;
  const next = tag.trim();
  found.version.tag = next ? next : null;
  found.pkg.updated_at = nowIso();
  persist();
  return true;
}

function deletePackageVersion(packageId: string, versionId: string): boolean {
  const pkg = packages.value.find((p) => p.id === packageId);
  if (!pkg) return false;
  const beforeCount = pkg.versions.length;
  pkg.versions = pkg.versions.filter((v) => v.id !== versionId);
  if (pkg.versions.length === beforeCount) return false;
  pkg.deployments = pkg.deployments.filter((d) => d.version_id !== versionId);
  pkg.updated_at = nowIso();
  persist();
  return true;
}

function syncPackageVersionToEnv(
  packageId: string,
  versionId: string,
  targetEnvId: string,
  targetEnvName: string,
  overwrite = true
): { copied: number; updated: number; skipped: number; manifestIds: string[] } {
  const found = findPackageVersion(packageId, versionId);
  if (!found) throw new Error("未找到应用包版本。");
  const { version } = found;
  let copied = 0;
  let updated = 0;
  let skipped = 0;
  const manifestIds: string[] = [];
  const now = nowIso();
  for (const res of version.resources) {
    const existing = manifests.value.find(
      (m) =>
        m.env_id === targetEnvId &&
        m.component === res.component &&
        m.resource_kind === res.resource_kind &&
        m.resource_name === res.resource_name &&
        (m.resource_namespace ?? null) === (res.resource_namespace ?? null)
    );
    if (existing) {
      if (!overwrite) {
        skipped += 1;
        continue;
      }
      existing.env_name = targetEnvName;
      existing.yaml = res.yaml;
      existing.updated_at = now;
      existing.history = pushHistory(existing.history, "save", res.yaml);
      manifestIds.push(existing.id);
      updated += 1;
      continue;
    }
    const next: OrchestratorManifest = {
      id: uid("manifest"),
      env_id: targetEnvId,
      env_name: targetEnvName,
      component: res.component,
      resource_kind: res.resource_kind,
      resource_name: res.resource_name,
      resource_namespace: res.resource_namespace,
      yaml: res.yaml,
      created_at: now,
      updated_at: now,
      history: pushHistory([], "save", res.yaml),
      source_type: "package_sync",
      source_batch_id: null,
      source_file_name: null,
      source_doc_index: null,
    };
    manifests.value = [next, ...manifests.value];
    manifestIds.push(next.id);
    copied += 1;
  }
  persist();
  return { copied, updated, skipped, manifestIds };
}

function recordPackageDeployment(
  packageId: string,
  versionId: string,
  targetEnvId: string,
  targetEnvName: string,
  mode: "sync" | "apply",
  success: number,
  failed: number,
  errors: string[]
): OrchestratorPackageDeploymentRecord {
  const found = findPackageVersion(packageId, versionId);
  if (!found) throw new Error("未找到应用包版本。");
  const { pkg, version } = found;
  const item: OrchestratorPackageDeploymentRecord = {
    id: uid("pkgdeploy"),
    at: nowIso(),
    package_id: pkg.id,
    package_name: pkg.name,
    version_id: version.id,
    version_label: version.label,
    target_env_id: targetEnvId,
    target_env_name: targetEnvName,
    mode,
    total: version.resources.length,
    success,
    failed,
    errors,
  };
  pkg.deployments = [item, ...pkg.deployments].slice(0, 50);
  pkg.updated_at = nowIso();
  persist();
  return item;
}

function copyComponentToEnv(
  sourceEnvId: string,
  component: string,
  targetEnvId: string,
  targetEnvName: string,
  overwrite = true
): { copied: number; updated: number; skipped: number } {
  const normalizedComponent = normalizeComponent(component);
  const sourceList = manifests.value.filter(
    (m) => m.env_id === sourceEnvId && m.component === normalizedComponent
  );
  if (!sourceList.length) return { copied: 0, updated: 0, skipped: 0 };

  let copied = 0;
  let updated = 0;
  let skipped = 0;
  const now = nowIso();

  for (const source of sourceList) {
    const existing = manifests.value.find(
      (m) =>
        m.env_id === targetEnvId &&
        m.component === normalizedComponent &&
        m.resource_kind === source.resource_kind &&
        m.resource_name === source.resource_name &&
        (m.resource_namespace ?? null) === (source.resource_namespace ?? null)
    );
    if (existing) {
      if (!overwrite) {
        skipped += 1;
        continue;
      }
      existing.env_name = targetEnvName;
      existing.yaml = source.yaml;
      existing.updated_at = now;
      existing.history = pushHistory(existing.history, "save", source.yaml);
      updated += 1;
      continue;
    }

    manifests.value = [
      {
        id: uid("manifest"),
        env_id: targetEnvId,
        env_name: targetEnvName,
        component: normalizedComponent,
        resource_kind: source.resource_kind,
        resource_name: source.resource_name,
        resource_namespace: source.resource_namespace,
        yaml: source.yaml,
        created_at: now,
        updated_at: now,
        history: pushHistory([], "save", source.yaml),
        source_type: source.source_type,
        source_batch_id: source.source_batch_id ?? null,
        source_file_name: source.source_file_name ?? null,
        source_doc_index: source.source_doc_index ?? null,
      },
      ...manifests.value,
    ];
    copied += 1;
  }
  persist();
  return { copied, updated, skipped };
}

function requestSwitchToOrchestrator(target?: OrchestratorFocusTarget | null) {
  orchestratorFocusTarget.value = target ?? null;
  switchToOrchestratorRequested.value += 1;
}

export function useOrchestratorStore() {
  return {
    manifests,
    packages,
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
    copyComponentToEnv,
    createPackage,
    deletePackage,
    createPackageVersion,
    setPackageVersionTag,
    deletePackageVersion,
    syncPackageVersionToEnv,
    recordPackageDeployment,
  };
}
