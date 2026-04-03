import { ref } from "vue";
import * as jsYaml from "js-yaml";
import { useAppSettingsStore } from "./appSettings";

export interface ResourceSnapshotRef {
  env_id: string;
  resource_kind: string;
  resource_name: string;
  resource_namespace: string | null;
}

export interface ResourceSnapshotItem extends ResourceSnapshotRef {
  id: string;
  created_at: string;
  category: "resource" | "config" | "image";
  source: "manual" | "before-apply" | "before-image-patch";
  pinned: boolean;
  title: string;
  summary: string;
  yaml: string;
}

const STORAGE_KEY = "kube-flow:resource-snapshots";
const snapshots = ref<ResourceSnapshotItem[]>(loadSnapshots());
const { autoSnapshotLimitPerResource } = useAppSettingsStore();

function loadSnapshots(): ResourceSnapshotItem[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((item) => item && typeof item === "object")
      .map((item) => {
        const snapshot = item as Partial<ResourceSnapshotItem>;
        const category =
          snapshot.category ??
          (snapshot.source === "before-image-patch" ? "image" : "resource");
        return {
          ...snapshot,
          category,
          pinned: snapshot.pinned === true,
        } as ResourceSnapshotItem;
      });
  } catch {
    return [];
  }
}

function persist() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(snapshots.value));
  } catch {}
}

function uid(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function summarizeContainers(obj: Record<string, unknown>): string | null {
  const spec = obj.spec as Record<string, unknown> | undefined;
  const podSpec =
    (spec?.template as Record<string, unknown> | undefined)?.spec as Record<string, unknown> | undefined;
  const containers = podSpec?.containers as { name?: string; image?: string }[] | undefined;
  if (!Array.isArray(containers) || containers.length === 0) return null;
  const preview = containers
    .slice(0, 2)
    .map((c) => `${c.name ?? "container"}=${c.image ?? "-"}`)
    .join(" · ");
  return containers.length > 2 ? `${containers.length} 个容器 · ${preview} 等` : `${containers.length} 个容器 · ${preview}`;
}

function dumpInlineScalar(value: string): string {
  return jsYaml.dump(value, { lineWidth: -1 }).trim();
}

function renderStringEntry(key: string, value: string, indent = "  "): string[] {
  const renderedKey = dumpInlineScalar(key);
  if (!value.includes("\n")) {
    return [`${indent}${renderedKey}: ${dumpInlineScalar(value)}`];
  }
  const lines = value.split("\n");
  const hasTrailingNewline = value.endsWith("\n");
  const header = `${indent}${renderedKey}: |${hasTrailingNewline ? "" : "-"}`;
  return [header, ...lines.map((line) => `${indent}  ${line}`)];
}

function renderStringMapSection(name: string, entries: Record<string, unknown> | undefined): string[] {
  const pairs = Object.entries(entries ?? {}).filter(([key]) => key.trim());
  if (pairs.length === 0) return [];
  const lines = [`${name}:`];
  for (const [key, raw] of pairs) {
    lines.push(...renderStringEntry(key, typeof raw === "string" ? raw : String(raw)));
  }
  return lines;
}

function stripManagedFieldsFromParsedObject(parsed: unknown): unknown {
  if (!parsed || typeof parsed !== "object") return parsed;
  const obj = parsed as Record<string, unknown>;
  if (!obj.metadata || typeof obj.metadata !== "object") return obj;
  const metadata = { ...(obj.metadata as Record<string, unknown>) };
  delete metadata.managedFields;
  return {
    ...obj,
    metadata,
  };
}

export function formatResourceSnapshotYaml(yaml: string): string {
  try {
    const parsed = stripManagedFieldsFromParsedObject(jsYaml.load(yaml));
    if (!parsed || typeof parsed !== "object") return yaml;
    const obj = parsed as Record<string, unknown>;
    const kind = typeof obj.kind === "string" ? obj.kind : "";
    if (kind !== "ConfigMap" && kind !== "Secret") {
      return jsYaml.dump(obj, { lineWidth: -1 });
    }

    const metadata = (obj.metadata as Record<string, unknown> | undefined) ?? {};
    const metadataYaml = jsYaml.dump(metadata, { lineWidth: -1 }).trimEnd();
    const baseLines = [
      "apiVersion: v1",
      `kind: ${kind}`,
      "metadata:",
      ...metadataYaml.split("\n").map((line) => `  ${line}`),
    ];

    if (kind === "ConfigMap") {
      const dataSection = renderStringMapSection("data", obj.data as Record<string, unknown> | undefined);
      const binarySection = renderStringMapSection("binaryData", obj.binaryData as Record<string, unknown> | undefined);
      return `${[...baseLines, ...dataSection, ...binarySection].join("\n")}\n`;
    }

    const typeValue = typeof obj.type === "string" && obj.type.trim() ? obj.type : "Opaque";
    const stringDataSection = renderStringMapSection(
      "stringData",
      ((obj.stringData as Record<string, unknown> | undefined) ??
        (obj.data as Record<string, unknown> | undefined))
    );
    return `${[...baseLines, `type: ${dumpInlineScalar(typeValue)}`, ...stringDataSection].join("\n")}\n`;
  } catch {
    return yaml;
  }
}

export function summarizeResourceYaml(yaml: string): string {
  try {
    const parsed = jsYaml.load(yaml);
    if (!parsed || typeof parsed !== "object") return "资源 YAML 快照";
    const obj = parsed as Record<string, unknown>;
    const kind = typeof obj.kind === "string" ? obj.kind : "资源";
    if (kind === "ConfigMap") {
      const data = (obj.data as Record<string, unknown> | undefined) ?? {};
      const binaryData = (obj.binaryData as Record<string, unknown> | undefined) ?? {};
      return `ConfigMap · ${Object.keys(data).length} 个 data，${Object.keys(binaryData).length} 个 binaryData`;
    }
    if (kind === "Secret") {
      const data = (obj.data as Record<string, unknown> | undefined) ?? {};
      const stringData = (obj.stringData as Record<string, unknown> | undefined) ?? {};
      const type = typeof obj.type === "string" && obj.type.trim() ? obj.type : "Opaque";
      return `Secret · ${type} · ${Math.max(Object.keys(data).length, Object.keys(stringData).length)} 个键`;
    }
    const containerSummary = summarizeContainers(obj);
    if (containerSummary) return `${kind} · ${containerSummary}`;
    return `${kind} · YAML 快照`;
  } catch {
    return "资源 YAML 快照";
  }
}

export function listResourceSnapshots(resource: ResourceSnapshotRef | null | undefined): ResourceSnapshotItem[] {
  return listResourceSnapshotsByCategory(resource, "all");
}

export function listResourceSnapshotsByCategory(
  resource: ResourceSnapshotRef | null | undefined,
  category: ResourceSnapshotItem["category"] | "all" = "all"
): ResourceSnapshotItem[] {
  if (!resource) return [];
  return snapshots.value.filter(
    (item) =>
      item.env_id === resource.env_id &&
      item.resource_kind === resource.resource_kind &&
      item.resource_name === resource.resource_name &&
      (item.resource_namespace ?? null) === (resource.resource_namespace ?? null) &&
      (category === "all" || item.category === category)
  );
}

export function createResourceSnapshot(
  resource: ResourceSnapshotRef,
  input: {
    yaml: string;
    category: ResourceSnapshotItem["category"];
    source: ResourceSnapshotItem["source"];
    title?: string;
    summary?: string;
  }
): ResourceSnapshotItem | null {
  const yaml = formatResourceSnapshotYaml(input.yaml.trim()).trim();
  if (!yaml) return null;
  const next: ResourceSnapshotItem = {
    ...resource,
    id: uid("snapshot"),
    created_at: new Date().toISOString(),
    category: input.category,
    source: input.source,
    pinned: false,
    title: input.title?.trim() || "资源快照",
    summary: input.summary?.trim() || summarizeResourceYaml(yaml),
    yaml,
  };
  const sameResource = listResourceSnapshots(resource);
  const isAutomatic = input.source !== "manual";
  const automaticLimit = Math.max(0, Math.floor(autoSnapshotLimitPerResource.value ?? 10));
  const automaticSnapshots = isAutomatic
    ? [next, ...sameResource.filter((item) => item.source !== "manual" && !item.pinned)]
    : sameResource.filter((item) => item.source !== "manual" && !item.pinned);
  const keptAutomaticIds = new Set(
    (automaticLimit === 0 ? automaticSnapshots : automaticSnapshots.slice(0, automaticLimit))
      .map((item) => item.id)
  );
  snapshots.value = [
    next,
    ...snapshots.value.filter((item) => {
      const matchesResource =
        item.env_id === resource.env_id &&
        item.resource_kind === resource.resource_kind &&
        item.resource_name === resource.resource_name &&
        (item.resource_namespace ?? null) === (resource.resource_namespace ?? null);
      if (!matchesResource) return true;
      if (item.source === "manual") return true;
      if (item.pinned) return true;
      return keptAutomaticIds.has(item.id);
    }),
  ];
  persist();
  return next;
}

export function toggleResourceSnapshotPinned(id: string): ResourceSnapshotItem | null {
  const target = snapshots.value.find((item) => item.id === id);
  if (!target) return null;
  target.pinned = !target.pinned;
  persist();
  return target;
}

export function deleteResourceSnapshot(id: string): boolean {
  const before = snapshots.value.length;
  snapshots.value = snapshots.value.filter((item) => item.id !== id);
  if (snapshots.value.length === before) return false;
  persist();
  return true;
}

export function useResourceSnapshotsStore() {
  return {
    snapshots,
    listResourceSnapshots,
    listResourceSnapshotsByCategory,
    createResourceSnapshot,
    toggleResourceSnapshotPinned,
    deleteResourceSnapshot,
  };
}
