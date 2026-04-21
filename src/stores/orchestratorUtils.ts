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
    return jsYaml.dump(next, { lineWidth: -1 });
  } catch {
    return yaml;
  }
}
