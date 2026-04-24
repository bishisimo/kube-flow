/**
 * 工作台（main tab）的 Token 语法字典与执行器。
 *
 * TokenSpec 覆盖：
 * - `@ns <name>`   命名空间（来自 namespacesByEnv，含 "all" 特殊值表示"全部"）
 * - `@kind <id>`   资源类型（RESOURCE_KINDS_FLAT）
 * - `#name <str>`  前端名字过滤（自由文本）
 * - `><动作筛选>`  当前列表选中资源的可执行动作（见 resourceActionPalette，无二级 key）
 *
 * Executor：
 * - workbench:navigate  存在 @ns / @kind / #name 任一时组合写入 workbenchPendingNav
 * - workbench:freeText  无 token 但有 freeText 时作为 #name 应用
 */
import { fuzzyMatch } from "./fuzzy";
import type { Executor, ExecutorPlan, TokenSpec, TokenValueCandidate } from "./types";
import { workbenchPendingNav, namespacesByEnv, useEnvStore } from "../../stores/env";
import { RESOURCE_KINDS_FLAT, RESOURCE_GROUPS } from "../../constants/resourceKinds";
import { buildValidResourceKindSet } from "../../features/workbench/apiKindMap";
import { extensionStableKey } from "../../features/workbench/builtinGvk";
import { readFavoriteKindEntries } from "../../features/workbench/composables/useWorkbenchFavoriteKinds";
import { workbenchKindPaletteExtensionHits } from "../../stores/workbenchKindPalette";
import type { ResolvedAliasTarget } from "../../api/types/kube";

const KIND_GROUP_LABEL = new Map<string, string>();
for (const g of RESOURCE_GROUPS) for (const k of g.kinds) KIND_GROUP_LABEL.set(k.id, g.label);

const VALID_RESOURCE_KIND_IDS = buildValidResourceKindSet(RESOURCE_KINDS_FLAT);

function encodeExtensionToken(t: ResolvedAliasTarget): string {
  return `ext:${encodeURIComponent(JSON.stringify(t))}`;
}

function fuzzyFilter<T>(query: string, items: T[], getText: (t: T) => string, max = 60): T[] {
  const q = query.trim();
  if (!q) return items.slice(0, max);
  const scored: Array<{ item: T; score: number }> = [];
  for (const item of items) {
    const m = fuzzyMatch(q, getText(item));
    if (m.score > 0) scored.push({ item, score: m.score });
  }
  scored.sort((a, b) => b.score - a.score);
  return scored.slice(0, max).map((x) => x.item);
}

export function buildWorkbenchTokenSpecs(): TokenSpec[] {
  const { currentId } = useEnvStore();

  const nsSpec: TokenSpec = {
    symbol: "@",
    key: "ns",
    label: "命名空间",
    hint: "限定工作台的命名空间",
    icon: "◈",
    context: "main",
    weight: 10,
    values: (query) => {
      const envId = currentId.value;
      const pool: TokenValueCandidate[] = [
        { value: "all", title: "全部命名空间", subtitle: "清除 ns 过滤", icon: "∀" },
      ];
      if (envId) {
        const names = namespacesByEnv.value[envId] ?? [];
        for (const name of names) pool.push({ value: name, title: name, icon: "◈" });
      }
      return fuzzyFilter(query, pool, (x) => x.title);
    },
  };

  const kindSpec: TokenSpec = {
    symbol: "@",
    key: "kind",
    label: "资源类型",
    hint: "限定工作台展示的资源类型",
    icon: "📦",
    context: "main",
    weight: 9,
    values: (query) => {
      void workbenchKindPaletteExtensionHits.value;
      const pool: TokenValueCandidate[] = [];
      const favs = readFavoriteKindEntries(VALID_RESOURCE_KIND_IDS);
      const favBuiltin = new Set(favs.filter((e) => e.kind === "builtin").map((e) => e.id));
      const favExtKeys = new Set(
        favs.filter((e) => e.kind === "extension").map((e) => extensionStableKey(e.target))
      );
      for (const e of favs) {
        if (e.kind === "builtin") {
          const k = RESOURCE_KINDS_FLAT.find((x) => x.id === e.id);
          if (!k) continue;
          pool.push({
            value: k.id,
            title: k.label,
            subtitle: `${KIND_GROUP_LABEL.get(k.id) ?? ""} · 收藏`.trim(),
            hint: k.id,
            icon: "★",
            section: "收藏",
            keywords: [k.id, k.label],
          });
        } else {
          const t = e.target;
          pool.push({
            value: encodeExtensionToken(t),
            title: t.kind,
            subtitle: `${t.api_version} · ${t.plural} · 收藏`,
            icon: "★",
            section: "收藏",
            keywords: [t.kind, t.plural, ...(t.short_names ?? [])],
          });
        }
      }
      for (const k of RESOURCE_KINDS_FLAT) {
        if (favBuiltin.has(k.id)) continue;
        pool.push({
          value: k.id,
          title: k.label,
          subtitle: KIND_GROUP_LABEL.get(k.id) ?? "",
          hint: k.id,
          icon: "📦",
          section: "内置",
          keywords: [k.id, k.label],
        });
      }
      for (const t of workbenchKindPaletteExtensionHits.value) {
        if (favExtKeys.has(extensionStableKey(t))) continue;
        pool.push({
          value: encodeExtensionToken(t),
          title: t.kind,
          subtitle: `${t.api_version} · ${t.plural}`,
          icon: "📦",
          section: "扩展",
          keywords: [t.kind, t.plural, t.group, ...(t.short_names ?? [])],
        });
      }
      return fuzzyFilter(
        query,
        pool,
        (x) => `${x.title} ${x.subtitle ?? ""} ${x.value} ${x.keywords?.join(" ") ?? ""}`,
        120
      );
    },
  };

  const nameSpec: TokenSpec = {
    symbol: "#",
    key: "name",
    label: "名字过滤",
    hint: "前端按名字包含筛选",
    icon: "🔎",
    context: "main",
    weight: 8,
    freeText: true,
    values: (query) => {
      const q = query.trim();
      if (!q) {
        return [
          {
            value: "",
            title: "输入名字关键字…",
            subtitle: "支持模糊匹配（前端过滤）",
            icon: "🔎",
          },
        ];
      }
      return [{ value: q, title: q, subtitle: "作为名字过滤", icon: "🔎" }];
    },
  };

  return [nsSpec, kindSpec, nameSpec];
}

/* ---------------- Executor ---------------- */

interface WorkbenchPlanParts {
  ns?: string | null;
  kind?: string;
  customTarget?: ResolvedAliasTarget;
  name?: string;
}

function readParts(tokens: ReadonlyArray<{ symbol: string; key: string; value: string }>): WorkbenchPlanParts {
  const parts: WorkbenchPlanParts = {};
  for (const t of tokens) {
    if (t.symbol === "@" && t.key === "ns") {
      parts.ns = t.value === "all" ? null : t.value;
    } else if (t.symbol === "@" && t.key === "kind") {
      if (t.value.startsWith("ext:")) {
        try {
          parts.customTarget = JSON.parse(decodeURIComponent(t.value.slice(4))) as ResolvedAliasTarget;
        } catch {
          parts.kind = t.value;
        }
      } else {
        parts.kind = t.value;
      }
    } else if (t.symbol === "#" && t.key === "name") {
      parts.name = t.value;
    }
  }
  return parts;
}

function describe(parts: WorkbenchPlanParts): string {
  const bits: string[] = [];
  if (parts.customTarget) bits.push(`列 ${parts.customTarget.kind}（CRD）`);
  else if (parts.kind) bits.push(`列 ${parts.kind}`);
  if (parts.ns !== undefined) bits.push(parts.ns ? `在 ${parts.ns}` : "全部命名空间");
  if (parts.name) bits.push(`名字含 "${parts.name}"`);
  return bits.length ? bits.join(" · ") : "切到工作台";
}

export function buildWorkbenchExecutors(): Executor[] {
  const { currentId } = useEnvStore();

  const navigate: Executor = {
    id: "workbench:navigate",
    context: "main",
    priority: 100,
    match: (ctx) => {
      const parts = readParts(ctx.tokens);
      const hasAny =
        parts.ns !== undefined || parts.kind || parts.customTarget || parts.name !== undefined;
      if (!hasAny) return null;
      const envId = currentId.value ?? undefined;
      const plan: ExecutorPlan = {
        title: describe(parts),
        subtitle: "工作台导航",
        icon: "⏎",
        run: () => {
          const pending: {
            envId?: string;
            kind?: string;
            namespace?: string | null;
            nameFilter?: string;
            customTarget?: ResolvedAliasTarget | null;
            focusResourceList?: boolean;
          } = {};
          if (envId) pending.envId = envId;
          if (parts.customTarget) pending.customTarget = parts.customTarget;
          else if (parts.kind) pending.kind = parts.kind;
          if (parts.ns !== undefined) pending.namespace = parts.ns;
          if (parts.name !== undefined) pending.nameFilter = parts.name;
          pending.focusResourceList = true;
          workbenchPendingNav.value = pending;
        },
      };
      return plan;
    },
  };

  const freeTextFallback: Executor = {
    id: "workbench:freeText",
    context: "main",
    priority: 10,
    match: (ctx) => {
      const text = ctx.freeText.trim();
      if (!text || ctx.tokens.length) return null;
      return {
        title: `按名字过滤 "${text}"`,
        subtitle: "工作台",
        icon: "🔎",
        run: () => {
          const envId = currentId.value ?? undefined;
          workbenchPendingNav.value = { envId, nameFilter: text, focusResourceList: true };
        },
      };
    },
  };

  return [navigate, freeTextFallback];
}
