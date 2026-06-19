import * as jsYaml from "js-yaml";
import type { KeyValueRow } from "./kvValidation";

export type KvImportConflictPolicy = "skip" | "overwrite";

export type KvImportDetectMode = "data-map" | "single-file" | "env-lines" | "files";

export interface KvImportEntry {
  key: string;
  value: string;
}

export interface KvImportPreviewItem extends KvImportEntry {
  status: "new" | "conflict" | "invalid";
  reason?: string;
}

function valueToString(value: unknown): string {
  if (typeof value === "string") return value;
  if (value == null) return "";
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (typeof value === "object") {
    return jsYaml.dump(value, { lineWidth: -1 }).trimEnd();
  }
  return String(value);
}

function mapToEntries(map: Record<string, unknown>): KvImportEntry[] {
  const entries: KvImportEntry[] = [];
  for (const [key, value] of Object.entries(map)) {
    const k = key.trim();
    if (!k) continue;
    entries.push({ key: k, value: valueToString(value) });
  }
  return entries;
}

function unwrapConfigSection(obj: Record<string, unknown>): KvImportEntry[] | null {
  const section = obj.data ?? obj.stringData;
  if (section && typeof section === "object" && !Array.isArray(section)) {
    return mapToEntries(section as Record<string, unknown>);
  }
  return null;
}

function looksLikeK8sResource(obj: Record<string, unknown>): boolean {
  return typeof obj.apiVersion === "string" || typeof obj.kind === "string";
}

function parseEnvLines(text: string): KvImportEntry[] | null {
  const lines = text.split("\n").map((l) => l.trim()).filter(Boolean);
  if (!lines.length) return null;
  const entries: KvImportEntry[] = [];
  for (const line of lines) {
    if (line.startsWith("#")) continue;
    const eq = line.match(/^([A-Za-z0-9_.-]+)\s*=\s*(.*)$/);
    if (eq) {
      entries.push({ key: eq[1], value: eq[2] });
      continue;
    }
    const colon = line.match(/^([A-Za-z0-9_.-]+)\s*:\s*(.*)$/);
    if (colon) {
      entries.push({ key: colon[1], value: colon[2] });
      continue;
    }
    return null;
  }
  return entries.length ? entries : null;
}

/** 解析粘贴内容：优先识别 data/stringData 段或多键映射，否则按单文件或 env 行处理。 */
export function parseKvImportPaste(
  text: string,
  defaultKey: string,
): { entries: KvImportEntry[]; mode: KvImportDetectMode; error?: string } {
  const trimmed = text.trim();
  if (!trimmed) {
    return { entries: [], mode: "single-file", error: "请先粘贴内容或选择文件。" };
  }

  try {
    const loaded = jsYaml.load(trimmed);
    if (loaded && typeof loaded === "object" && !Array.isArray(loaded)) {
      const obj = loaded as Record<string, unknown>;
      const fromSection = unwrapConfigSection(obj);
      if (fromSection?.length) {
        return { entries: fromSection, mode: "data-map" };
      }
      if (!looksLikeK8sResource(obj)) {
        const entries = mapToEntries(obj);
        if (entries.length) {
          return { entries, mode: "data-map" };
        }
      }
    }
    if (typeof loaded === "string" && loaded.trim()) {
      const key = defaultKey.trim() || "config.yaml";
      return { entries: [{ key, value: loaded }], mode: "single-file" };
    }
  } catch {
    // fall through
  }

  const envEntries = parseEnvLines(trimmed);
  if (envEntries) {
    return { entries: envEntries, mode: "env-lines" };
  }

  const key = defaultKey.trim() || "config.yaml";
  return { entries: [{ key, value: text }], mode: "single-file" };
}

/** 将本地文件读为配置项，键名默认取文件名。 */
export function entriesFromFiles(files: File[]): Promise<KvImportEntry[]> {
  return Promise.all(
    files.map(
      (file) =>
        new Promise<KvImportEntry>((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => {
            resolve({
              key: file.name,
              value: typeof reader.result === "string" ? reader.result : "",
            });
          };
          reader.onerror = () => reject(reader.error);
          reader.readAsText(file);
        }),
    ),
  );
}

export function buildImportPreview(
  entries: KvImportEntry[],
  existingKeys: Iterable<string>,
): KvImportPreviewItem[] {
  const existing = new Set([...existingKeys].map((k) => k.trim()).filter(Boolean));
  const seen = new Set<string>();
  const preview: KvImportPreviewItem[] = [];

  for (const entry of entries) {
    const key = entry.key.trim();
    if (!key) {
      preview.push({ ...entry, status: "invalid", reason: "键名为空" });
      continue;
    }
    if (seen.has(key)) {
      preview.push({ ...entry, key, status: "invalid", reason: "导入批次内重复" });
      continue;
    }
    seen.add(key);
    if (existing.has(key)) {
      preview.push({ ...entry, key, status: "conflict" });
      continue;
    }
    preview.push({ ...entry, key, status: "new" });
  }
  return preview;
}

export function applyKvImport(
  rows: KeyValueRow[],
  entries: KvImportEntry[],
  policy: KvImportConflictPolicy,
): {
  rows: KeyValueRow[];
  added: number;
  skipped: number;
  overwritten: number;
} {
  const next = rows.map((r) => ({ ...r }));
  const indexByKey = new Map<string, number>();
  next.forEach((row, i) => {
    const k = row.key.trim();
    if (k) indexByKey.set(k, i);
  });

  let added = 0;
  let skipped = 0;
  let overwritten = 0;
  const batchSeen = new Set<string>();

  for (const entry of entries) {
    const key = entry.key.trim();
    if (!key || batchSeen.has(key)) continue;
    batchSeen.add(key);

    const idx = indexByKey.get(key);
    if (idx === undefined) {
      next.push({ key, value: entry.value });
      indexByKey.set(key, next.length - 1);
      added += 1;
      continue;
    }
    if (policy === "skip") {
      skipped += 1;
      continue;
    }
    next[idx] = { ...next[idx], key, value: entry.value };
    overwritten += 1;
  }

  return { rows: next, added, skipped, overwritten };
}

export function importModeLabel(mode: KvImportDetectMode): string {
  switch (mode) {
    case "data-map":
      return "多条 data 键值";
    case "single-file":
      return "单文件内容";
    case "env-lines":
      return "KEY=VALUE 行";
    case "files":
      return "本地文件";
    default:
      return "未知";
  }
}
