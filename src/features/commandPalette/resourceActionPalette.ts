/**
 * 工作台「>…」：对当前列表选中资源列出可执行动作（`>` 后可直接筛选），由 Executor 触发 Main 的 runAction。
 */
import { fuzzyMatch } from "./fuzzy";
import type { Executor, ExecutorPlan, TokenSpec, TokenValueCandidate } from "./types";
import { workbenchResourcePaletteAdapter } from "../../stores/workbenchResourcePalette";

function fuzzyFilterValues(query: string, items: TokenValueCandidate[], max = 80): TokenValueCandidate[] {
  const q = query.trim();
  if (!q) return items.slice(0, max);
  const scored: Array<{ item: TokenValueCandidate; score: number }> = [];
  for (const item of items) {
    const text = `${item.title} ${item.subtitle ?? ""} ${item.keywords?.join(" ") ?? ""}`;
    const m = fuzzyMatch(q, text);
    if (m.score > 0) scored.push({ item, score: m.score });
  }
  scored.sort((a, b) => b.score - a.score);
  return scored.slice(0, max).map((x) => x.item);
}

export function buildResourceActionTokenSpec(): TokenSpec {
  return {
    symbol: ">",
    key: "",
    label: "动作",
    hint: "列出当前资源可执行操作，继续输入可筛选",
    icon: "⚡",
    context: "main",
    weight: 12,
    values: (query) => {
      const a = workbenchResourcePaletteAdapter.value;
      if (!a) {
        return [
          {
            value: "__please_select",
            title: "请切换到工作台",
            subtitle: "在工作台页面使用资源动作",
            icon: "⚠️",
          },
        ];
      }
      return fuzzyFilterValues(query, a.getValueCandidates());
    },
  };
}

export function buildResourceActionExecutor(): Executor {
  return {
    id: "workbench:resource-action",
    context: "main",
    priority: 101,
    match: (ctx): ExecutorPlan | null => {
      const hasWorkbenchNavToken = ctx.tokens.some((t) => t.symbol === "@" || t.symbol === "#");
      if (hasWorkbenchNavToken) return null;
      const act = ctx.tokens.find((t) => t.symbol === ">" && t.key === "");
      if (!act) return null;
      const a = workbenchResourcePaletteAdapter.value;
      if (!a) return null;
      if (act.value === "__please_select") {
        return {
          title: "请先选中资源",
          subtitle: "在工作台列表用 ↑↓ 选择一行",
          icon: "⚠️",
          run: () => {},
        };
      }
      const title =
        a.getValueCandidates().find((c) => c.value === act.value)?.title ?? act.value;
      return {
        title: `执行：${title}`,
        subtitle: "资源动作",
        icon: "⚡",
        run: () => {
          a.runAction(act.value);
        },
      };
    },
  };
}
