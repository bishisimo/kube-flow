/**
 * 编排中心——应用包管理子 store。
 * 负责 OrchestratorPackage / OrchestratorPackageVersion / OrchestratorPackageDeploymentRecord 的 CRUD，
 * 以及跨环境的包同步与组件复制操作。
 */
import { ref } from "vue";
import { createStorage } from "../utils/storage";
import {
  uid,
  nowIso,
  buildVersionLabelFromDate,
  normalizeComponent,
  pushHistory,
} from "./orchestratorUtils";
import type {
  OrchestratorManifest,
  OrchestratorPackage,
  OrchestratorPackageVersion,
  OrchestratorPackageDeploymentRecord,
} from "./orchestratorTypes";

export type {
  OrchestratorPackage,
  OrchestratorPackageVersion,
  OrchestratorPackageDeploymentRecord,
} from "./orchestratorTypes";

// ── 导入 manifests 状态（只在此文件读写，不触发循环依赖） ──────────────────────
import { manifests, manifestStorage, manifestIndex, rebuildManifestIndex, manifestKey } from "./orchestrator";

// ── Storage ───────────────────────────────────────────────────────────────────

const packageStorage = createStorage<OrchestratorPackage[]>({
  key: "kube-flow:orchestrator:packages",
  version: 1,
  fallback: [],
  migrate: (old) => {
    const arr = Array.isArray(old) ? old : [];
    return arr
      .filter((p) => p && typeof p === "object")
      .map((p) => {
        const pkg = p as OrchestratorPackage;
        const versions = Array.isArray(pkg.versions)
          ? pkg.versions.map((v) => ({
              ...v,
              tag: typeof v.tag === "string" && v.tag.trim() ? v.tag.trim() : null,
            }))
          : [];
        return { ...pkg, versions, deployments: Array.isArray(pkg.deployments) ? pkg.deployments : [] } as OrchestratorPackage;
      });
  },
});

const packages = ref<OrchestratorPackage[]>(packageStorage.read());

function persistPackages() {
  packageStorage.write(packages.value);
}

// ── Package CRUD ──────────────────────────────────────────────────────────────

function createPackage(name: string, description = ""): OrchestratorPackage {
  const trimmedName = name.trim();
  if (!trimmedName) throw new Error("应用包名称不能为空。");
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
  persistPackages();
  return next;
}

function deletePackage(id: string): boolean {
  const before = packages.value.length;
  packages.value = packages.value.filter((p) => p.id !== id);
  const changed = before !== packages.value.length;
  if (changed) persistPackages();
  return changed;
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
  if (!normalizedComponents.length) throw new Error("至少选择一个组件。");
  const source = manifests.value
    .filter((m) => m.env_id === sourceEnvId && normalizedComponents.includes(m.component))
    .sort((a, b) => {
      if (a.component !== b.component) return a.component.localeCompare(b.component);
      if (a.resource_kind !== b.resource_kind) return a.resource_kind.localeCompare(b.resource_kind);
      return a.resource_name.localeCompare(b.resource_name);
    });
  if (!source.length) throw new Error("所选组件没有可打包资源。");
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
  persistPackages();
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
  persistPackages();
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
  persistPackages();
  return true;
}

// ── Cross-env operations (read + write manifests) ────────────────────────────

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
    const key = manifestKey(targetEnvId, res.resource_kind, res.resource_name, res.resource_namespace ?? null);
    const existing = manifestIndex.get(key);
    if (existing) {
      if (!overwrite) { skipped += 1; continue; }
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
  rebuildManifestIndex();
  manifestStorage.write(manifests.value);
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
  persistPackages();
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
    const key = manifestKey(targetEnvId, source.resource_kind, source.resource_name, source.resource_namespace ?? null);
    const existing = manifestIndex.get(key);
    if (existing) {
      if (!overwrite) { skipped += 1; continue; }
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
  rebuildManifestIndex();
  manifestStorage.write(manifests.value);
  return { copied, updated, skipped };
}

export function useOrchestratorPackagesStore() {
  return {
    packages,
    createPackage,
    deletePackage,
    createPackageVersion,
    findPackageVersion,
    setPackageVersionTag,
    deletePackageVersion,
    syncPackageVersionToEnv,
    recordPackageDeployment,
    copyComponentToEnv,
  };
}
