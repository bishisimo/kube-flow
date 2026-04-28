<script setup lang="ts">
import { computed, h, Fragment, ref } from "vue";
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
  /** 键盘导航时高亮行（与右键菜单高亮独立）。 */
  keyboardFocusRowKey?: string | null;
}>();

const emit = defineEmits<{
  shellKeydown: [e: KeyboardEvent];
}>();

const tableShellRef = ref<HTMLElement | null>(null);

function focusShell() {
  tableShellRef.value?.focus();
}

defineExpose({ focusShell, tableShellRef });

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
      minWidth: isFirst ? 184 : 108,
      resizable: !isFirst,
      ellipsis: col.key === "name" || isFirst ? { tooltip: true } : false,
      sorter: sortable ? "default" : false,
      sortOrder:
        sortable && props.sortBy === col.key ? (props.sortOrder === "asc" ? "ascend" : "descend") : false,
      className: isFirst ? "wb-col-emphasis" : "wb-col-meta",
      render(row) {
        return renderCell(col, row);
      },
    });
  }
  return out;
});

function rowClassName(row: Record<string, unknown>): string {
  const key = props.getRowKey(row);
  const parts: string[] = [];
  if (key && props.keyboardFocusRowKey && key === props.keyboardFocusRowKey) {
    parts.push("wb-row-keyboard-focus");
  }
  if (key && props.activeRowKey && key === props.activeRowKey) {
    parts.push(props.deleteActionArmed ? "wb-row-active wb-row-armed" : "wb-row-active");
  }
  return parts.join(" ");
}

function rowKey(row: Record<string, unknown>) {
  return props.getRowKey(row);
}

function rowProps(row: Record<string, unknown>) {
  const rk = props.getRowKey(row);
  return {
    "data-wb-row": rk || undefined,
    style: { cursor: "pointer" },
    onClick: (e: MouseEvent) => {
      const el = e.target as HTMLElement | null;
      if (el?.closest?.(".n-checkbox, .n-radio, .n-data-table-td--selection, button, a, .n-button")) {
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
  <div
    ref="tableShellRef"
    class="wb-table-keyboard-shell"
    tabindex="0"
    @keydown.capture="emit('shellKeydown', $event)"
  >
  <section class="wb-list-card">
    <header class="wb-list-header">
      <div class="wb-list-heading">
        <span class="wb-list-kicker">RESOURCE MATRIX</span>
        <span class="wb-list-title">资源列表</span>
      </div>
      <div class="wb-list-meta">
        <span class="wb-live-chip">
          <span class="wb-live-dot" aria-hidden="true" />
          实时视图
        </span>
        <span class="wb-list-badge" :title="`${tableRows.length} 条`">{{ tableRows.length }}</span>
      </div>
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
        :bordered="false"
        :bottom-bordered="false"
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
  </div>
</template>

<style scoped>
.wb-table-keyboard-shell {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  outline: none;
}
.wb-table-keyboard-shell:focus-visible {
  border-radius: 14px;
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--wb-accent-forest) 55%, transparent);
}
.wb-list-card {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 0.7rem 1rem 1rem;
  background:
    radial-gradient(circle at 12% 0%, color-mix(in srgb, var(--kf-primary) 10%, transparent), transparent 34%),
    linear-gradient(180deg, color-mix(in srgb, var(--wb-canvas) 88%, #f8fbff), var(--wb-canvas));
}
.wb-list-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.85rem;
  padding: 0.62rem 0.8rem 0.5rem;
  border-bottom: none;
  min-width: 0;
  position: relative;
}
.wb-list-header::after {
  content: "";
  position: absolute;
  left: 0.8rem;
  right: 0.8rem;
  bottom: -0.12rem;
  height: 2px;
  border-radius: 999px;
  background: linear-gradient(
    90deg,
    color-mix(in srgb, var(--kf-primary) 88%, transparent) 0%,
    color-mix(in srgb, var(--wb-accent-forest) 72%, transparent) 48%,
    transparent 100%
  );
  opacity: 0.82;
  box-shadow: 0 0 22px color-mix(in srgb, var(--kf-primary) 20%, transparent);
}
.wb-list-heading {
  display: flex;
  flex-direction: column;
  gap: 0.12rem;
  min-width: 0;
}
.wb-list-kicker {
  font-size: 0.62rem;
  line-height: 1;
  font-weight: 800;
  letter-spacing: 0.16em;
  color: color-mix(in srgb, var(--kf-primary) 74%, var(--wb-text-secondary));
}
.wb-list-title {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  font-size: 1.04rem;
  font-weight: 750;
  letter-spacing: -0.02em;
  color: var(--wb-text-primary);
}
.wb-list-title::before {
  content: "";
  width: 4px;
  height: 1.05em;
  border-radius: 4px;
  flex-shrink: 0;
  background: linear-gradient(180deg, var(--kf-primary) 0%, var(--wb-accent-spring) 100%);
  box-shadow: 0 0 16px color-mix(in srgb, var(--kf-primary) 28%, transparent);
}
.wb-list-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  flex-shrink: 0;
}
.wb-live-chip {
  display: inline-flex;
  align-items: center;
  gap: 0.38rem;
  min-height: 1.7rem;
  padding: 0 0.62rem;
  border: 1px solid color-mix(in srgb, var(--kf-primary) 20%, var(--wb-line));
  border-radius: 999px;
  background: color-mix(in srgb, var(--kf-primary) 7%, var(--wb-panel-elevated));
  color: color-mix(in srgb, var(--kf-primary) 72%, var(--wb-text-primary));
  font-size: 0.72rem;
  font-weight: 750;
}
.wb-live-dot {
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: var(--wb-accent-spring);
  box-shadow:
    0 0 0 3px color-mix(in srgb, var(--wb-accent-spring) 14%, transparent),
    0 0 16px color-mix(in srgb, var(--wb-accent-spring) 55%, transparent);
  animation: wb-live-pulse 1.8s ease-in-out infinite;
}
.wb-list-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 1.65rem;
  min-height: 1.7rem;
  padding: 0 0.5rem;
  border-radius: 999px;
  font-size: 0.72rem;
  font-weight: 700;
  color: color-mix(in srgb, var(--kf-primary) 68%, var(--wb-chip-text));
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--kf-primary) 10%, var(--kf-mix-surface)),
    color-mix(in srgb, var(--wb-accent-forest) 7%, var(--kf-mix-surface))
  );
  border: 1px solid color-mix(in srgb, var(--kf-primary) 18%, var(--wb-line));
  box-shadow: inset 0 1px 0 color-mix(in srgb, #ffffff 72%, transparent);
}
.wb-table-scroll {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.wb-data-table {
  --n-border-radius: 16px;
  width: 100%;
  flex: 1;
  min-height: 0;
}
.wb-data-table:deep(.n-data-table-wrapper) {
  position: relative;
  overflow: hidden;
  border-radius: 16px;
  background:
    linear-gradient(var(--wb-panel-elevated), var(--wb-panel-elevated)) padding-box,
    linear-gradient(
      135deg,
      color-mix(in srgb, var(--kf-primary) 34%, transparent),
      color-mix(in srgb, var(--wb-accent-forest) 18%, transparent),
      color-mix(in srgb, var(--wb-line) 80%, transparent)
    ) border-box;
  border: 1px solid transparent;
  box-shadow:
    0 1px 3px rgba(15, 23, 42, 0.06),
    0 22px 46px color-mix(in srgb, var(--kf-primary) 8%, transparent),
    0 0 0 1px color-mix(in srgb, #ffffff 56%, transparent) inset;
}
.wb-data-table:deep(.n-data-table-wrapper)::before {
  content: "";
  position: absolute;
  inset: 0;
  pointer-events: none;
  background-image:
    linear-gradient(color-mix(in srgb, var(--kf-primary) 7%, transparent) 1px, transparent 1px),
    linear-gradient(90deg, color-mix(in srgb, var(--kf-primary) 5%, transparent) 1px, transparent 1px);
  background-size: 28px 28px;
  mask-image: linear-gradient(180deg, rgba(0, 0, 0, 0.44), transparent 58%);
  opacity: 0.7;
  z-index: 1;
}
.wb-data-table:deep(.n-data-table-base-table) {
  position: relative;
  z-index: 2;
}
.wb-data-table:deep(.n-data-table-base-table-header) {
  position: sticky;
  top: 0;
  z-index: 3;
  box-shadow:
    0 1px 0 var(--wb-line-strong),
    0 10px 22px color-mix(in srgb, var(--kf-primary) 5%, transparent);
}
.wb-data-table:deep(.n-data-table-th) {
  font-weight: 800;
  font-size: 0.69rem;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: color-mix(in srgb, var(--kf-primary) 58%, var(--wb-table-heading-text));
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--kf-primary) 8%, var(--wb-table-header)), var(--wb-table-header));
  border-bottom: 1px solid color-mix(in srgb, var(--kf-primary) 24%, var(--wb-line-strong));
  border-right: none;
  border-left: none;
  padding: 12px 16px;
  transition: background 0.16s ease, color 0.16s ease;
}
.wb-data-table:deep(.n-data-table-td) {
  background: color-mix(in srgb, var(--wb-table-body) 94%, transparent);
  border-bottom: 1px solid color-mix(in srgb, var(--wb-line) 72%, transparent);
  border-right: none;
  border-left: none;
  padding: 12px 16px;
  transition: background 0.16s ease, box-shadow 0.16s ease, color 0.16s ease;
}
.wb-data-table:deep(
  .n-data-table-base-table-body .n-data-table-tr:not(.wb-row-active):not(.wb-row-keyboard-focus):hover .n-data-table-td
) {
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--kf-primary) 8%, transparent), transparent 38%),
    var(--wb-row-hover);
  box-shadow: inset 0 1px 0 color-mix(in srgb, #ffffff 68%, transparent);
}
.wb-data-table:deep(.n-data-table-tr.wb-row-keyboard-focus .n-data-table-td) {
  background:
    linear-gradient(90deg, color-mix(in srgb, var(--kf-primary) 14%, transparent), transparent 42%),
    color-mix(in srgb, var(--wb-accent-spring) 10%, var(--wb-table-body));
  box-shadow:
    inset 3px 0 0 var(--kf-primary),
    inset 0 0 0 1px color-mix(in srgb, var(--kf-primary) 26%, transparent);
}
.wb-data-table:deep(.wb-col-emphasis) {
  font-weight: 600;
  color: var(--wb-text-primary);
}
.wb-data-table:deep(.wb-col-meta) {
  color: var(--wb-text-meta);
  font-size: 0.8125rem;
}
.wb-data-table:deep(.n-data-table-tr.wb-row-active .n-data-table-td) {
  background: linear-gradient(
    90deg,
    color-mix(in srgb, var(--kf-primary) 15%, var(--wb-row-selected)),
    var(--wb-row-selected) 54%,
    color-mix(in srgb, var(--wb-accent-forest) 8%, var(--wb-row-selected))
  );
  box-shadow:
    inset 3px 0 0 var(--kf-primary),
    inset 0 0 0 1px color-mix(in srgb, var(--kf-primary) 15%, transparent);
}
.wb-data-table:deep(.n-data-table-tr.wb-row-armed .n-data-table-td) {
  background: linear-gradient(
    180deg,
    color-mix(in srgb, var(--kf-danger-soft) 72%, var(--kf-mix-surface)),
    color-mix(in srgb, var(--kf-danger-soft) 48%, var(--kf-mix-surface))
  );
  box-shadow: inset 3px 0 0 var(--kf-danger);
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
  box-shadow: var(--wb-focus-ring);
}
.wb-primary-cell-content {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  max-width: 100%;
}
.wb-primary-dot {
  width: 8px;
  height: 8px;
  border-radius: 999px;
  margin-top: 0.32rem;
  background: linear-gradient(
    145deg,
    var(--kf-primary) 0%,
    var(--wb-accent-spring) 100%
  );
  box-shadow:
    0 0 0 1px color-mix(in srgb, var(--kf-primary) 28%, transparent),
    0 0 14px color-mix(in srgb, var(--kf-primary) 22%, transparent);
  flex-shrink: 0;
}
.wb-primary-title {
  font-weight: 620;
  color: var(--wb-text-primary);
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
  box-shadow: var(--wb-focus-ring);
}
.wb-taint-tag {
  --n-font-size: 12px;
  --n-height: 22px;
  --n-border-radius: 999px;
  min-width: 28px;
  justify-content: center;
}
.wb-taint-tag-count {
  --n-color: var(--wb-teal-soft);
  --n-text-color: var(--wb-accent-forest);
}
.wb-taint-tag-empty {
  --n-color: color-mix(in srgb, var(--kf-text-muted) 14%, var(--kf-mix-surface));
  --n-text-color: var(--kf-text-secondary);
}
.wb-empty-wrap {
  padding: 1.25rem 0.75rem 1.5rem;
}
.wb-empty-hint {
  margin: 0.65rem 0 0;
  font-size: 0.8125rem;
  color: var(--wb-text-secondary);
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
@keyframes wb-live-pulse {
  0%,
  100% {
    opacity: 0.72;
    transform: scale(0.92);
  }
  50% {
    opacity: 1;
    transform: scale(1.12);
  }
}
</style>
