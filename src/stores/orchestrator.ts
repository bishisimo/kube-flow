/**
 * 资源编排台状态：维护 YAML 资产、组件归类、历史快照与页面切换请求。
 */
import { ref } from "vue";
import * as jsYaml from "js-yaml";

export interface OrchestratorResourceRef {
  kind: string;
  name: string;
  namespace: string | null;
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
}

const STORAGE_KEY = "kube-flow:orchestrator:manifests";

const manifests = ref<OrchestratorManifest[]>(loadManifests());
const switchToOrchestratorRequested = ref(0);

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
  } catch {}
}

function uid(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function nowIso(): string {
  return new Date().toISOString();
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
      },
      ...manifests.value,
    ];
    copied += 1;
  }
  persist();
  return { copied, updated, skipped };
}

function requestSwitchToOrchestrator() {
  switchToOrchestratorRequested.value += 1;
}

export function useOrchestratorStore() {
  return {
    manifests,
    switchToOrchestratorRequested,
    requestSwitchToOrchestrator,
    upsertFromWorkbenchSync,
    saveManifestYaml,
    setManifestComponent,
    createManifestDraft,
    setManifestIdentity,
    deleteManifest,
    copyComponentToEnv,
  };
}
