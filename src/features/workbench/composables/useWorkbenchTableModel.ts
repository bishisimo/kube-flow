import { computed, type Ref } from "vue";
import type { ResourceKind } from "../../../constants/resourceKinds";
import type { DynamicCrdInstanceItem, ResolvedAliasTarget } from "../../../api/kube";
import { WORKBENCH_SORTABLE_KEYS } from "../constants";
import { getWorkbenchResourceDescriptor } from "../resourceDescriptors";
import {
  buildColumnsForKind,
  buildRowsForKind,
  type WorkbenchTableColumn,
  type WorkbenchTableDescriptorContext,
} from "./workbenchTableDescriptors";

export type { NodeAllocSnapshot, WorkbenchTableColumn } from "./workbenchTableDescriptors";

export type UseWorkbenchTableModelOptions = WorkbenchTableDescriptorContext & {
  selectedKind: Ref<ResourceKind>;
  selectedCustomTarget: Ref<ResolvedAliasTarget | null>;
  dynamicCrdItems: Ref<DynamicCrdInstanceItem[]>;
  nameFilter: Ref<string>;
  nodeFilter: Ref<string>;
  podIpFilter: Ref<string>;
  sortBy: Ref<string>;
  sortOrder: Ref<"asc" | "desc">;
};

/**
 * 工作台资源表：基于 descriptor 构建列与行模型，支持按资源能力扩展。
 */
export function useWorkbenchTableModel(o: UseWorkbenchTableModelOptions) {
  function supportsIpFilter(): boolean {
    return getWorkbenchResourceDescriptor(o.selectedKind.value).capabilities.supportsIpFilter;
  }

  const rawTableRows = computed(() => {
    const crd = o.selectedCustomTarget.value;
    if (crd) {
      return o.dynamicCrdItems.value.map((it) => ({
        name: it.name,
        ns: crd.namespaced ? (it.namespace != null && it.namespace !== "" ? it.namespace : "—") : "—",
        creationTime: it.creation_time ?? "-",
      }));
    }
    return buildRowsForKind(o.selectedKind.value, o);
  });

  function compareForSort(a: unknown, b: unknown, order: "asc" | "desc"): number {
    const va = a == null || a === "-" ? "" : String(a);
    const vb = b == null || b === "-" ? "" : String(b);
    const cmp = va.localeCompare(vb, undefined, { numeric: true });
    return order === "asc" ? cmp : -cmp;
  }

  function compareCreationTime(a: unknown, b: unknown, order: "asc" | "desc"): number {
    const da = a === "-" || !a ? "" : String(a);
    const db = b === "-" || !b ? "" : String(b);
    const cmp = da.localeCompare(db);
    return order === "asc" ? cmp : -cmp;
  }

  function sortRows<T extends Record<string, unknown>>(rows: T[], by: string, order: "asc" | "desc"): T[] {
    if (!by || !rows.length) return rows;
    return [...rows].sort((a, b) => {
      const primary =
        by === "creationTime"
          ? compareCreationTime(a[by], b[by], order)
          : compareForSort(a[by], b[by], order);
      if (primary !== 0) return primary;
      return compareForSort(a.name, b.name, "desc");
    });
  }

  function onSortColumn(key: string) {
    if (!WORKBENCH_SORTABLE_KEYS.has(key)) return;
    if (o.sortBy.value === key) {
      o.sortOrder.value = o.sortOrder.value === "asc" ? "desc" : "asc";
    } else {
      o.sortBy.value = key;
      o.sortOrder.value = key === "creationTime" ? "desc" : "asc";
    }
  }

  const tableColumns = computed((): WorkbenchTableColumn[] => {
    if (o.selectedCustomTarget.value) {
      return [
        { key: "name", label: "名称" },
        { key: "ns", label: "Namespace" },
        { key: "creationTime", label: "创建时间" },
      ];
    }
    return buildColumnsForKind(o.selectedKind.value, o);
  });

  const tableRows = computed(() => {
    let raw = rawTableRows.value as Record<string, unknown>[];
    const q = o.nameFilter.value.trim().toLowerCase();
    if (q) raw = raw.filter((r) => String(r.name ?? "").toLowerCase().includes(q));
    if (o.selectedKind.value === "pods" && o.nodeFilter.value !== "all") {
      raw = raw.filter((r) => String(r.node ?? "") === o.nodeFilter.value);
    }
    if (supportsIpFilter()) {
      const ip = o.podIpFilter.value.trim().toLowerCase();
      if (ip) {
        raw = raw.filter((r) => {
          const candidate = o.selectedKind.value === "services" ? r.clusterIp : r.podIp;
          return String(candidate ?? "").toLowerCase().includes(ip);
        });
      }
    }
    const by = o.sortBy.value;
    const order = o.sortOrder.value;
    if (by && tableColumns.value.some((c) => c.key === by)) {
      return sortRows(raw, by, order);
    }
    return sortRows(raw, "creationTime", "desc");
  });

  return { rawTableRows, tableColumns, tableRows, onSortColumn };
}
