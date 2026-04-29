export interface CompactRailItemInput {
  id: string;
  label: string;
  context?: string | null;
  fallback?: string;
}

export interface CompactRailItemMeta {
  id: string;
  shortLabel: string;
  fullLabel: string;
}

const GENERIC_TOKENS = new Set([
  "cluster",
  "context",
  "default",
  "env",
  "environment",
  "host",
  "k8s",
  "kube",
  "kubeflow",
  "log",
  "logs",
  "main",
  "pod",
  "prod",
  "shell",
  "terminal",
  "test",
  "workload",
]);

function splitLabel(label: string): string[] {
  return label
    .split(/[\s/|:@()[\]{}]+|(?:\s+-\s+)|(?:\s+\/\s+)/g)
    .map((part) => part.trim())
    .filter(Boolean);
}

function isAsciiToken(value: string): boolean {
  return /^[A-Za-z0-9_-]+$/.test(value);
}

function normalizeToken(token: string): string {
  return token.replace(/^[^A-Za-z0-9\u4e00-\u9fff]+|[^A-Za-z0-9\u4e00-\u9fff]+$/g, "");
}

function abbreviateToken(token: string, maxLength: number): string {
  if (token.length <= maxLength) return token;
  if (isAsciiToken(token)) return token.slice(0, maxLength);
  return Array.from(token).slice(0, maxLength).join("");
}

function deriveBaseLabel(label: string, fallback = "Item", maxLength = 8): string {
  const normalized = label.trim();
  if (!normalized) return fallback;

  const tokens = splitLabel(normalized).map(normalizeToken).filter(Boolean);
  const preferred =
    [...tokens].reverse().find((token) => !GENERIC_TOKENS.has(token.toLowerCase())) ??
    tokens[tokens.length - 1] ??
    normalized;

  return abbreviateToken(preferred, maxLength);
}

export function buildCompactRailItems(inputs: CompactRailItemInput[]): Record<string, CompactRailItemMeta> {
  const byBase = new Map<string, CompactRailItemInput[]>();
  const baseById = new Map<string, string>();

  for (const input of inputs) {
    const base = deriveBaseLabel(input.label, input.fallback ?? "Item");
    baseById.set(input.id, base);
    const group = byBase.get(base) ?? [];
    group.push(input);
    byBase.set(base, group);
  }

  const result: Record<string, CompactRailItemMeta> = {};
  const usedLabels = new Map<string, number>();
  for (const input of inputs) {
    const base = baseById.get(input.id) ?? (input.fallback ?? "Item");
    const group = byBase.get(base) ?? [];
    let shortLabel = base;

    if (group.length > 1) {
      const contextBase = input.context ? deriveBaseLabel(input.context, "", 4) : "";
      shortLabel = contextBase && contextBase !== base ? `${base}-${contextBase}` : base;
    }

    const duplicateIndex = usedLabels.get(shortLabel) ?? 0;
    usedLabels.set(shortLabel, duplicateIndex + 1);
    if (duplicateIndex > 0) {
      shortLabel = `${shortLabel}${duplicateIndex + 1}`;
    }

    result[input.id] = {
      id: input.id,
      shortLabel,
      fullLabel: input.label,
    };
  }

  return result;
}
