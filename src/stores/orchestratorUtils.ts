/**
 * 编排中心工具函数 — 纯函数，无副作用，可跨子模块共享。
 */
import * as jsYaml from "js-yaml";
import type { ManifestHistoryItem } from "./orchestratorTypes";
import { uid } from "../utils/uid";

export { uid } from "../utils/uid";

export function nowIso(): string {
  return new Date().toISOString();
}

export function pad2(n: number): string {
  return String(n).padStart(2, "0");
}

export function formatRfc3339Local(date: Date): string {
  const year = date.getFullYear();
  const month = pad2(date.getMonth() + 1);
  const day = pad2(date.getDate());
  const hours = pad2(date.getHours());
  const minutes = pad2(date.getMinutes());
  const seconds = pad2(date.getSeconds());
  const offsetMinutes = -date.getTimezoneOffset();
  const sign = offsetMinutes >= 0 ? "+" : "-";
  const offsetHours = pad2(Math.floor(Math.abs(offsetMinutes) / 60));
  const offsetRemainMinutes = pad2(Math.abs(offsetMinutes) % 60);
  return `${year}-${month}-${day}T${hours}:${minutes}:${seconds}${sign}${offsetHours}:${offsetRemainMinutes}`;
}

export function buildVersionLabelFromDate(d: Date): string {
  const y = d.getFullYear();
  const m = pad2(d.getMonth() + 1);
  const day = pad2(d.getDate());
  const hh = pad2(d.getHours());
  const mm = pad2(d.getMinutes());
  const ss = pad2(d.getSeconds());
  return `${y}${m}${day}-${hh}${mm}${ss}`;
}

export function batchLabel(sourceKind: "file" | "text", now = new Date()): string {
  const prefix = sourceKind === "text" ? "创建" : "导入";
  return `${formatRfc3339Local(now)} ${prefix}`;
}

export function buildHistory(action: ManifestHistoryItem["action"], yaml: string): ManifestHistoryItem {
  return { id: uid("hist"), at: nowIso(), action, yaml };
}

export function pushHistory(
  history: ManifestHistoryItem[],
  action: ManifestHistoryItem["action"],
  yaml: string
): ManifestHistoryItem[] {
  const latest = history[0];
  if (latest && latest.action === action && latest.yaml === yaml) return history;
  return [buildHistory(action, yaml), ...history].slice(0, 30);
}

export function normalizeComponent(component: string): string {
  const v = component.trim();
  return v || "default";
}

/** 常见集群级资源：不同一改写 metadata.namespace。 */
const CLUSTER_SCOPED_KINDS = new Set([
  "Namespace",
  "Node",
  "PersistentVolume",
  "ClusterRole",
  "ClusterRoleBinding",
  "StorageClass",
  "IngressClass",
  "PriorityClass",
  "CustomResourceDefinition",
  "MutatingWebhookConfiguration",
  "ValidatingWebhookConfiguration",
  "CSIDriver",
  "CSINode",
  "VolumeAttachment",
  "RuntimeClass",
  "APIService",
  "FlowSchema",
  "PriorityLevelConfiguration",
]);

export function isClusterScopedKind(kind: string): boolean {
  return CLUSTER_SCOPED_KINDS.has(kind);
}

export function normalizeInstallNamespace(namespace: string | null | undefined): string {
  const v = (namespace ?? "").trim();
  return v || "default";
}

/**
 * 将 YAML 的 metadata.namespace 对齐到组件安装命名空间。
 * 集群级资源会移除 namespace；命名空间级资源写入 targetNamespace。
 */
export function rewriteYamlNamespace(yaml: string, targetNamespace: string, kindHint?: string): string {
  try {
    const parsed = jsYaml.load(yaml);
    if (!parsed || typeof parsed !== "object") return yaml;
    const obj = parsed as Record<string, unknown>;
    const kind =
      (typeof kindHint === "string" && kindHint ? kindHint : null) ||
      (typeof obj.kind === "string" ? obj.kind : "");
    const meta =
      obj.metadata && typeof obj.metadata === "object"
        ? ({ ...(obj.metadata as Record<string, unknown>) } as Record<string, unknown>)
        : ({} as Record<string, unknown>);
    if (isClusterScopedKind(kind)) {
      delete meta.namespace;
    } else {
      meta.namespace = normalizeInstallNamespace(targetNamespace);
    }
    obj.metadata = meta;
    return jsYaml.dump(obj, { lineWidth: -1 });
  } catch {
    return yaml;
  }
}

/** 从资源列表推断组件安装命名空间（取出现次数最多的非空 namespace）。 */
export function inferNamespaceFromResources(
  items: Array<{ kind: string; namespace: string | null }>
): string {
  const counts = new Map<string, number>();
  for (const item of items) {
    if (isClusterScopedKind(item.kind)) continue;
    const ns = normalizeInstallNamespace(item.namespace);
    counts.set(ns, (counts.get(ns) ?? 0) + 1);
  }
  let best = "default";
  let bestCount = 0;
  for (const [ns, count] of counts) {
    if (count > bestCount) {
      best = ns;
      bestCount = count;
    }
  }
  return best;
}

/** Service 的 clusterIP / clusterIPs 由集群分配，同步到编排资产时需剥离；Headless（None）除外。 */
function stripServiceAllocatedClusterIps(obj: Record<string, unknown>): void {
  if (obj.kind !== "Service") return;
  const spec =
    obj.spec && typeof obj.spec === "object" ? (obj.spec as Record<string, unknown>) : null;
  if (!spec) return;
  const clusterIp = typeof spec.clusterIP === "string" ? spec.clusterIP : "";
  if (clusterIp === "None") return;
  const nextSpec = { ...spec };
  delete nextSpec.clusterIP;
  delete nextSpec.clusterIPs;
  obj.spec = nextSpec;
}

export function sanitizeYamlForSync(yaml: string): string {
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
    stripServiceAllocatedClusterIps(next);
    return jsYaml.dump(next, { lineWidth: -1 });
  } catch {
    return yaml;
  }
}
