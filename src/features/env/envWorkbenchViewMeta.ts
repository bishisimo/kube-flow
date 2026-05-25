import { RESOURCE_GROUPS, type ResourceKind } from "../../constants/resourceKinds";
import type { EnvViewState } from "../../stores/env";

const kindLabelMap = new Map<string, string>(
  RESOURCE_GROUPS.flatMap((group) => group.kinds.map((kind) => [kind.id, kind.label]))
);

export function resourceKindLabel(kindId: string): string {
  return kindLabelMap.get(kindId as ResourceKind) ?? kindId;
}

export function formatNamespaceLabel(namespace: string | null): string {
  return namespace?.trim() ? namespace : "全部";
}

export interface WorkbenchViewContext {
  namespace: string;
  namespaceIsAll: boolean;
  resourceLabel: string;
  resourceDetail: string | null;
  isExtension: boolean;
}

export function buildWorkbenchViewContext(state: EnvViewState): WorkbenchViewContext {
  if (state.customTarget) {
    const target = state.customTarget;
    return {
      namespace: formatNamespaceLabel(state.namespace),
      namespaceIsAll: !state.namespace?.trim(),
      resourceLabel: target.kind,
      resourceDetail: `${target.api_version} · ${target.plural}`,
      isExtension: true,
    };
  }
  return {
    namespace: formatNamespaceLabel(state.namespace),
    namespaceIsAll: !state.namespace?.trim(),
    resourceLabel: resourceKindLabel(state.kind),
    resourceDetail: null,
    isExtension: false,
  };
}

export interface WorkbenchViewTableRow {
  item: string;
  value: string;
  empty: boolean;
}

export function buildWorkbenchContextTableRows(state: EnvViewState): WorkbenchViewTableRow[] {
  const context = buildWorkbenchViewContext(state);
  const rows: WorkbenchViewTableRow[] = [
    {
      item: "命名空间",
      value: context.namespace,
      empty: context.namespaceIsAll,
    },
    {
      item: context.isExtension ? "扩展资源" : "资源类型",
      value: context.resourceLabel,
      empty: !context.isExtension && state.kind === "namespaces" && !state.customTarget,
    },
  ];

  if (context.isExtension && context.resourceDetail) {
    rows.push({
      item: "API",
      value: context.resourceDetail,
      empty: false,
    });
  }

  return rows;
}

export function buildWorkbenchFilterTableRows(state: EnvViewState): WorkbenchViewTableRow[] {
  const filters: Array<[string, string]> = [
    ["名称", state.nameFilter.trim()],
    ["节点", state.nodeFilter === "all" ? "" : state.nodeFilter],
    ["Pod IP", state.podIpFilter.trim()],
    ["标签", state.labelSelector.trim()],
  ];

  return filters.map(([item, raw]) => ({
    item,
    value: raw || "—",
    empty: !raw,
  }));
}

export function hasActiveWorkbenchFilters(state: EnvViewState): boolean {
  return buildWorkbenchFilterTableRows(state).some((row) => !row.empty);
}

export interface RecentNamespaceChartItem {
  name: string;
  rank: number;
  percent: number;
}

export function buildRecentNamespaceChartItems(names: string[]): RecentNamespaceChartItem[] {
  if (!names.length) return [];
  const maxRank = names.length;
  return names.map((name, index) => ({
    name,
    rank: index + 1,
    percent: Math.round(((maxRank - index) / maxRank) * 100),
  }));
}
