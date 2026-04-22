<script setup lang="ts">
import { computed, h, Fragment } from "vue";
import {
  NButton,
  NDataTable,
  NEmpty,
  NSpace,
  NTag,
} from "naive-ui";
import type { DataTableColumns, DataTableRowKey, DataTableSortState } from "naive-ui";
import type { ResourceKind } from "../../constants/resourceKinds";
import { WORKBENCH_SORTABLE_KEYS } from "../../features/workbench";

type TableColumn = { key: string; label: string };
type PodRollupBadge = { key: string; tone: string; value: string | number };

const props = defineProps<{
  workbenchKindLabel?: string;
  effectiveNamespace?: string;
  tableRows: Record<string, unknown>[];
  tableColumns: TableColumn[];
  batchDeleteMode: boolean;
  selectedRowKeys: Set<string>;
  sortBy: string;
  sortOrder: "asc" | "desc";
  selectedKind: ResourceKind;
  listLoading?: boolean;
  nsSelectionDisabled: boolean;
  selectedNamespace: string | null;
  getRowKey: (row: Record<string, unknown>) => string;
  /** 与 DataTable 受控勾选对齐，替代逐行 toggle。 */
  replaceSelectedRowKeys: (keys: string[]) => void;
  /** 远程排序：由列表模型统一重排数据。 */
  setWorkbenchSort: (key: string, order: "asc" | "desc") => void;
  onRowClick: (row: Record<string, unknown>) => void;
  onRowContextMenu: (row: Record<string, unknown>, e: MouseEvent) => void;
  onNamespaceRowDblClick: (name: string) => void;
  isCellDrillable: (key: string, row: Record<string, unknown>) => boolean;
  onReplicasClick: (row: Record<string, unknown>) => void;
  onRoleRefClick: (row: Record<string, unknown>) => void;
  onPvcCellClick: (row: Record<string, unknown>, colKey: string) => void;
  getSubjectsList: (row: Record<string, unknown>) => unknown[];
  getSubjectLabel: (s: unknown, row: Record<string, unknown>) => string;
  onSubjectClick: (row: Record<string, unknown>, s: unknown) => void;
  isStatusColumn: (key: string) => boolean;
  statusTone: (v: unknown) => string;
  normalizeStatus: (v: unknown) => string;
  isPodRollupColumn: (key: string) => boolean;
  buildPodRollupBadges: (v: unknown) => PodRollupBadge[];
  formatRecentRestart: (rollup: unknown) => string;
  isRecentRestartHot: (rollup: unknown) => boolean;
  isNodeAllocColumn: (key: string) => boolean;
  nodeAllocTone: (v: unknown) => string;
  openNodeTaintsFromRow: (row: Record<string, unknown>) => void;
  clearAllFilters: () => void;
  selectAllNamespaces: () => void;
  openKindSelector: () => void;
  /** 当前被右键 ActionMenu 选中的行 key，用于高亮。 */
  activeRowKey?: string | null;
  /** 二次确认删除已触发时，把 active 行再换成告警色。 */
  deleteActionArmed?: boolean;
}>();

const firstDataColumnKey = computed(() => props.tableColumns[0]?.key ?? "");

const checkedRowKeys = computed<DataTableRowKey[]>(() => Array.from(props.selectedRowKeys));

function statusTagType(tone: string): "success" | "warning" | "error" | "default" {
  if (tone === "ok") return "success";
  if (tone === "warn") return "warning";
  if (tone === "error") return "error";
  return "default";
}

function rollupTagType(tone: string): "success" | "warning" | "error" | "default" | "info" {
  switch (tone) {
    case "running":
      return "success";
    case "pending":
      return "warning";
    case "failed":
      return "error";
    case "abnormal":
      return "error";
    case "succeeded":
      return "default";
    default:
      return "info";
  }
}

function podRollupBordered(tone: string): boolean {
  return tone !== "succeeded";
}

function renderCell(col: TableColumn, row: Record<string, unknown>) {
  const key = col.key;
  const isPrimaryColumn = key === firstDataColumnKey.value;

  if (key === "subjects" && props.getSubjectsList(row).length > 0) {
    const items = props.getSubjectsList(row);
    return h(
      Fragment,
      {},
      items.map((s, si) =>
        h(Fragment, { key: si }, [
          h(
            NButton,
            {
              text: true,
              type: "primary",
              size: "tiny",
              onClick: (e: MouseEvent) => {
                e.stopPropagation();
                props.onSubjectClick(row, s);
              },
            },
            { default: () => props.getSubjectLabel(s, row) },
          ),
          si < items.length - 1 ? ", " : null,
        ]),
      ),
    );
  }

  if (props.isStatusColumn(key)) {
    const raw = row[key as keyof typeof row];
    const normalized = props.normalizeStatus(raw);
    return h(
      NTag,
      {
        size: "small",
        bordered: true,
        type: statusTagType(props.statusTone(raw)),
        style: { maxWidth: "100%" },
      },
      { default: () => normalized },
    );
  }

  if (props.isPodRollupColumn(key)) {
    const raw = row[key as keyof typeof row];
    const badges = props.buildPodRollupBadges(raw);
    if (badges.length === 0) {
      return "—";
    }
    return h(
      NSpace,
      { size: 6, wrap: true, inline: true },
      {
        default: () =>
          badges.map((b) =>
            h(
              NTag,
              {
                key: b.key,
                size: "small",
                round: true,
                bordered: podRollupBordered(b.tone),
                type: rollupTagType(b.tone),
              },
              { default: () => b.value },
            ),
          ),
      },
    );
  }

  if (key === "recentRestart") {
    const hot = props.isRecentRestartHot(row.podRollup);
    const text = props.formatRecentRestart(row.podRollup);
    return h(
      "span",
      { class: { "wb-restart-hot": hot }, style: hot ? { fontWeight: "600", color: "var(--n-color-error)" } : undefined },
      text,
    );
  }

  if (props.isNodeAllocColumn(key)) {
    const raw = row[key as keyof typeof row];
    const tone = props.nodeAllocTone(raw);
    const nTagType = tone === "warn" ? "warning" : tone === "danger" ? "error" : "default";
    return h(
      NTag,
      { size: "small", bordered: false, type: nTagType },
      { default: () => String(raw ?? "") },
    );
  }

  if (props.selectedKind === "nodes" && key === "taints") {
    const raw = row[key as keyof typeof row];
    const empty = raw === "无";
    const text = String(raw ?? "无");
    const count = Number(text);
    const hasCount = Number.isFinite(count) && count > 0;
    return h(
      NButton,
      {
        size: "small",
        quaternary: true,
        class: "wb-taint-btn",
        onClick: (e: MouseEvent) => {
          e.stopPropagation();
          props.openNodeTaintsFromRow(row);
        },
      },
      {
        default: () => {
          if (empty) {
            return h(
              NTag,
              { size: "small", round: true, bordered: false, class: "wb-taint-tag wb-taint-tag-empty" },
              { default: () => "无" },
            );
          }
          if (hasCount) {
            return h(
              NTag,
              { size: "small", round: true, bordered: false, type: "info", class: "wb-taint-tag wb-taint-tag-count" },
              { default: () => text },
            );
          }
          return h(
            NTag,
            { size: "small", round: true, bordered: false, type: "info", class: "wb-taint-tag" },
            { default: () => text },
          );
        },
      },
    );
  }

  const drillable = props.isCellDrillable(key, row) && key !== "subjects";
  if (isPrimaryColumn) {
    const valueText = String(row[key as keyof typeof row] ?? "—");
    const content = h("span", { class: "wb-primary-cell-content" }, [
      h("span", { class: "wb-primary-dot", "aria-hidden": "true" }),
      h("span", { class: "wb-primary-title" }, valueText),
    ]);
    if (!drillable) return content;
    return h(
      NButton,
      {
        text: true,
        type: "primary",
        size: "small",
        class: "wb-primary-btn",
        onClick: (e: MouseEvent) => {
          e.stopPropagation();
          if (key === "replicas") props.onReplicasClick(row);
          else if (key === "roleRef") props.onRoleRefClick(row);
          else props.onPvcCellClick(row, key);
        },
      },
      { default: () => content },
    );
  }

  if (drillable) {
    const val = row[key as keyof typeof row];
    return h(
      NButton,
      {
        text: true,
        type: "primary",
        size: "small",
        onClick: (e: MouseEvent) => {
          e.stopPropagation();
          if (key === "replicas") props.onReplicasClick(row);
          else if (key === "roleRef") props.onRoleRefClick(row);
          else props.onPvcCellClick(row, key);
        },
      },
      { default: () => (val == null ? "—" : String(val)) },
    );
  }

  const v = row[key as keyof typeof row];
  return v == null ? "—" : String(v);
}

const columns = computed<DataTableColumns<Record<string, unknown>>>(() => {
  const out: DataTableColumns<Record<string, unknown>> = [];
  if (props.batchDeleteMode) {
    out.push({ type: "selection", width: 44 });
  }
  const firstKey = firstDataColumnKey.value;
  for (const col of props.tableColumns) {
    const sortable = WORKBENCH_SORTABLE_KEYS.has(col.key);
    const isFirst = col.key === firstKey;
    out.push({
      title: col.label,
      key: col.key,
      minWidth: isFirst ? 180 : 100,
      resizable: !isFirst,
      ellipsis: col.key === "name" || isFirst ? { tooltip: true } : false,
      sorter: sortable ? "default" : false,
      sortOrder:
        sortable && props.sortBy === col.key ? (props.sortOrder === "asc" ? "ascend" : "descend") : false,
      className: isFirst ? "wb-col-emphasis" : undefined,
      render(row) {
        return renderCell(col, row);
      },
    });
  }
  return out;
});

function rowClassName(row: Record<string, unknown>): string {
  const key = props.getRowKey(row);
  if (!key || !props.activeRowKey || key !== props.activeRowKey) return "";
  return props.deleteActionArmed ? "wb-row-active wb-row-armed" : "wb-row-active";
}

function rowKey(row: Record<string, unknown>) {
  return props.getRowKey(row);
}

function rowProps(row: Record<string, unknown>) {
  return {
    style: { cursor: "pointer" },
    onClick: (e: MouseEvent) => {
      const el = e.target as HTMLElement | null;
      if (el?.closest?.(".n-checkbox, .n-radio, .n-data-table-td--selection, button, a")) {
        return;
      }
      props.onRowClick(row);
    },
    onContextmenu: (e: MouseEvent) => {
      e.preventDefault();
      props.onRowContextMenu(row, e);
    },
    onDblclick: () => {
      if (props.selectedKind === "namespaces" && row.name) {
        props.onNamespaceRowDblClick(String(row.name));
      }
    },
  };
}

function handleSorterChange(sorter: DataTableSortState | DataTableSortState[] | null) {
  if (Array.isArray(sorter)) return;
  if (!sorter || sorter.order === false) {
    props.setWorkbenchSort("creationTime", "desc");
    return;
  }
  const colKey = String(sorter.columnKey);
  if (!WORKBENCH_SORTABLE_KEYS.has(colKey)) return;
  props.setWorkbenchSort(colKey, sorter.order === "ascend" ? "asc" : "desc");
}

function onUpdateCheckedRowKeys(keys: DataTableRowKey[]) {
  if (!props.batchDeleteMode) return;
  props.replaceSelectedRowKeys(keys.map((k) => String(k)));
}
</script>

<template>
  <section class="wb-list-card">
    <header class="wb-list-header">
      <span class="wb-list-title">资源列表</span>
      <span class="wb-list-badge" :title="`${tableRows.length} 条`">{{ tableRows.length }}</span>
    </header>
    <div class="wb-table-scroll">
      <NDataTable
        class="wb-data-table"
        size="small"
        :columns="columns"
        :data="tableRows"
        :row-key="rowKey"
        :row-props="rowProps"
        :row-class-name="rowClassName"
        :striped="false"
        :single-line="false"
        :bordered="true"
        :bottom-bordered="true"
        :remote="true"
        :flex-height="true"
        :pagination="false"
        :loading="Boolean(listLoading) && tableRows.length > 0"
        :checked-row-keys="batchDeleteMode ? checkedRowKeys : undefined"
        @update:checked-row-keys="onUpdateCheckedRowKeys"
        @update:sorter="handleSorterChange"
      >
        <template #empty>
          <div v-if="listLoading" class="wb-empty-wrap">
            <NEmpty description="正在加载资源…" size="medium" />
          </div>
          <div v-else class="wb-empty-wrap">
            <NEmpty description="暂无资源" size="medium">
              <template #extra>
                <div class="wb-empty-actions">
                  <NButton size="small" secondary @click="clearAllFilters">清空筛选</NButton>
                  <NButton
                    v-if="!nsSelectionDisabled && selectedNamespace !== null"
                    size="small"
                    secondary
                    @click="selectAllNamespaces"
                  >
                    切回默认命名空间
                  </NButton>
                  <NButton size="small" secondary @click="openKindSelector">切换资源类型</NButton>
                </div>
              </template>
            </NEmpty>
            <p class="wb-empty-hint">可尝试调整命名空间、资源类型或筛选条件。</p>
          </div>
        </template>
      </NDataTable>
    </div>
  </section>
</template>

<style scoped>
.wb-list-card {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 0.65rem 1rem 1rem;
  background: var(--wb-canvas, #eef2f9);
}
.wb-list-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
  padding: 0.55rem 0.75rem 0.45rem;
  border-bottom: 1px solid var(--kf-border, rgba(148, 163, 184, 0.22));
  min-width: 0;
}
.wb-list-title {
  font-size: 1rem;
  font-weight: 650;
  letter-spacing: -0.02em;
  color: var(--kf-text-primary, #0f172a);
}
.wb-list-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 1.65rem;
  padding: 0.08rem 0.4rem;
  border-radius: 999px;
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--kf-primary, #2563eb);
  background: var(--kf-primary-soft, #e8f0ff);
  border: 1px solid rgba(37, 99, 235, 0.2);
}
.wb-table-scroll {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.wb-data-table {
  --n-border-radius: 12px;
  width: 100%;
  flex: 1;
  min-height: 0;
}
.wb-data-table:deep(.n-data-table-wrapper) {
  border-radius: 0 0 12px 12px;
  background: #ffffff;
}
.wb-data-table:deep(.n-data-table-base-table-header) {
  position: sticky;
  top: 0;
  z-index: 2;
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.55),
    inset 0 -1px 0 rgba(100, 116, 139, 0.36);
}
.wb-data-table:deep(.n-data-table-th) {
  font-weight: 650;
  font-size: 0.72rem;
  letter-spacing: 0.04em;
  background: linear-gradient(180deg, #edf3fb 0%, #e1eaf6 100%);
  border-bottom: 1px solid rgba(100, 116, 139, 0.32);
}
.wb-data-table:deep(.n-data-table-td) {
  background: #ffffff;
}
.wb-data-table:deep(.wb-col-emphasis) {
  font-weight: 600;
  color: var(--kf-text-primary, #0f172a);
}
.wb-data-table:deep(.n-data-table-tr.wb-row-active .n-data-table-td) {
  background: linear-gradient(180deg, #eef4ff 0%, #e1ebff 100%);
  box-shadow: inset 2px 0 0 var(--kf-primary, #2563eb);
}
.wb-data-table:deep(.n-data-table-tr.wb-row-armed .n-data-table-td) {
  background: linear-gradient(180deg, #fff1f2 0%, #ffe4e6 100%);
  box-shadow: inset 2px 0 0 #dc2626;
}
.wb-primary-btn {
  padding: 0;
  border-radius: 8px;
}
.wb-primary-btn:deep(.n-button__content) {
  width: 100%;
}
.wb-primary-btn:focus-visible {
  outline: none;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.18);
}
.wb-primary-cell-content {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  max-width: 100%;
}
.wb-primary-dot {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  margin-top: 0.32rem;
  background: var(--kf-primary, #2563eb);
  opacity: 0.9;
  flex-shrink: 0;
}
.wb-primary-title {
  font-weight: 620;
  color: var(--kf-text-primary, #0f172a);
  line-height: 1.3;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
}
.wb-taint-btn {
  --n-padding: 0 4px;
  --n-height: 24px;
  --n-border-radius: 999px;
}
.wb-taint-btn:focus-visible {
  outline: none;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.18);
}
.wb-taint-tag {
  --n-font-size: 12px;
  --n-height: 22px;
  --n-border-radius: 999px;
  min-width: 28px;
  justify-content: center;
}
.wb-taint-tag-count {
  --n-color: #dbeafe;
  --n-text-color: #1d4ed8;
}
.wb-taint-tag-empty {
  --n-color: #e2e8f0;
  --n-text-color: #64748b;
}
.wb-empty-wrap {
  padding: 1.25rem 0.75rem 1.5rem;
}
.wb-empty-hint {
  margin: 0.65rem 0 0;
  font-size: 0.8125rem;
  color: var(--kf-text-secondary, #64748b);
  text-align: center;
}
.wb-empty-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.45rem;
  justify-content: center;
}
.wb-empty-actions :deep(.n-button) {
  --n-height: 30px;
  --n-border-radius: 9px;
}
</style>
