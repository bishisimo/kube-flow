<script setup lang="ts">
import { computed } from "vue";
import { NButton, NCheckbox } from "naive-ui";
import type { ResourceKind } from "../../constants/resourceKinds";
import { WORKBENCH_SORTABLE_KEYS } from "../../features/workbench";

type TableColumn = { key: string; label: string };

type PodRollupBadge = { key: string; tone: string; value: string | number };

const props = defineProps<{
  tableRows: Record<string, unknown>[];
  tableColumns: TableColumn[];
  batchDeleteMode: boolean;
  selectedRowKeys: Set<string>;
  sortBy: string;
  sortOrder: "asc" | "desc";
  selectedKind: ResourceKind;
  nsSelectionDisabled: boolean;
  selectedNamespace: string | null;
  getRowKey: (row: Record<string, unknown>) => string;
  toggleSelectAll: () => void;
  toggleRowSelection: (row: Record<string, unknown>) => void;
  onRowClick: (row: Record<string, unknown>) => void;
  onRowContextMenu: (row: Record<string, unknown>, e: MouseEvent) => void;
  onNamespaceRowDblClick: (name: string) => void;
  isSelectedRow: (row: Record<string, unknown>) => boolean;
  onSortColumn: (key: string) => void;
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
}>();

const allRowsSelected = computed(
  () => props.tableRows.length > 0 && props.tableRows.every((r) => props.selectedRowKeys.has(props.getRowKey(r)))
);

const someRowsSelected = computed(
  () => props.selectedRowKeys.size > 0 && props.selectedRowKeys.size < props.tableRows.length
);
</script>

<template>
  <div class="table-wrap">
    <table class="resource-table">
      <thead>
        <tr>
          <th v-if="batchDeleteMode" class="col-checkbox">
            <NCheckbox
              :checked="allRowsSelected"
              :indeterminate="someRowsSelected"
              @update:checked="toggleSelectAll"
            />
          </th>
          <th
            v-for="col in tableColumns"
            :key="col.key"
            :class="{ sortable: WORKBENCH_SORTABLE_KEYS.has(col.key) }"
            @click="WORKBENCH_SORTABLE_KEYS.has(col.key) && onSortColumn(col.key)"
          >
            {{ col.label }}
            <span v-if="WORKBENCH_SORTABLE_KEYS.has(col.key) && sortBy === col.key" class="sort-indicator">
              {{ sortOrder === "asc" ? "↑" : "↓" }}
            </span>
          </th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="(row, i) in tableRows"
          :key="i"
          class="row-clickable"
          :class="{ 'row-selected': isSelectedRow(row) }"
          @click="onRowClick(row)"
          @contextmenu.prevent="onRowContextMenu(row, $event)"
          @dblclick="selectedKind === 'namespaces' && row.name && onNamespaceRowDblClick(String(row.name))"
        >
          <td v-if="batchDeleteMode" class="col-checkbox" @click.stop>
            <NCheckbox :checked="selectedRowKeys.has(getRowKey(row))" @update:checked="toggleRowSelection(row)" />
          </td>
          <td
            v-for="col in tableColumns"
            :key="col.key"
            :class="{ 'cell-drillable': isCellDrillable(col.key, row) && col.key !== 'subjects' }"
            @click="
              (e) => {
                if (isCellDrillable(col.key, row) && col.key !== 'subjects') {
                  e.stopPropagation();
                  if (col.key === 'replicas') onReplicasClick(row);
                  else if (col.key === 'roleRef') onRoleRefClick(row);
                  else onPvcCellClick(row, col.key);
                }
              }
            "
          >
            <template v-if="col.key === 'subjects' && getSubjectsList(row).length > 0">
              <template v-for="(s, si) in getSubjectsList(row)" :key="si">
                <span class="cell-link" @click="(e: MouseEvent) => { e.stopPropagation(); onSubjectClick(row, s); }">
                  {{ getSubjectLabel(s, row) }}
                </span>
                <span v-if="si < getSubjectsList(row).length - 1">, </span>
              </template>
            </template>
            <template v-else>
              <span
                v-if="isStatusColumn(col.key)"
                class="status-pill"
                :class="`status-${statusTone(row[col.key as keyof typeof row])}`"
              >
                {{ normalizeStatus(row[col.key as keyof typeof row]) }}
              </span>
              <template v-else-if="isPodRollupColumn(col.key)">
                <div class="pod-rollup-cell">
                  <template v-for="badge in buildPodRollupBadges(row[col.key as keyof typeof row])" :key="badge.key">
                    <span class="pod-rollup-badge" :class="`pod-rollup-badge-${badge.tone}`"
                      ><span class="pod-rollup-dot" />{{ badge.value }}</span
                    >
                  </template>
                  <span v-if="buildPodRollupBadges(row[col.key as keyof typeof row]).length === 0" class="pod-rollup-empty"
                    >-</span
                  >
                </div>
              </template>
              <template v-else-if="col.key === 'recentRestart'">
                <span :class="{ 'recent-restart-hot': isRecentRestartHot(row.podRollup) }">{{
                  formatRecentRestart(row.podRollup)
                }}</span>
              </template>
              <template v-else-if="isNodeAllocColumn(col.key)">
                <span class="node-alloc-pill" :class="`node-alloc-pill-${nodeAllocTone(row[col.key as keyof typeof row])}`">
                  {{ row[col.key as keyof typeof row] }}
                </span>
              </template>
              <template v-else-if="selectedKind === 'nodes' && col.key === 'taints'">
                <button
                  type="button"
                  class="taint-entry-btn"
                  :class="{ 'taint-entry-btn-empty': row[col.key as keyof typeof row] === '无' }"
                  @click.stop="openNodeTaintsFromRow(row)"
                >
                  <span class="taint-entry-label">污点</span>
                  <span class="taint-entry-value">{{ row[col.key as keyof typeof row] }}</span>
                </button>
              </template>
              <template v-else>
                {{ row[col.key as keyof typeof row] }}
              </template>
            </template>
          </td>
        </tr>
      </tbody>
    </table>
    <div v-if="!tableRows.length" class="empty-table">
      <div class="empty-emoji" aria-hidden="true">📭</div>
      <p class="empty-title">暂无资源</p>
      <p class="empty-desc">可尝试调整命名空间、资源类型或筛选条件。</p>
      <div class="empty-actions">
        <NButton secondary size="small" @click="clearAllFilters">清空筛选</NButton>
        <NButton
          v-if="!nsSelectionDisabled && selectedNamespace !== null"
          secondary
          size="small"
          @click="selectAllNamespaces"
        >
          切回默认命名空间
        </NButton>
        <NButton secondary size="small" @click="openKindSelector">切换资源类型</NButton>
      </div>
    </div>
  </div>
</template>

<style scoped>
.table-wrap {
  flex: 1;
  overflow: auto;
  padding: 0.9rem 1rem 1rem;
  background: linear-gradient(180deg, #f7faff 0%, #f3f7fc 100%);
}
.resource-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.8125rem;
  background: rgba(255, 255, 255, 0.95);
  border: 1px solid rgba(148, 163, 184, 0.24);
  border-radius: 14px;
  box-shadow: 0 12px 30px rgba(15, 23, 42, 0.08);
  overflow: hidden;
}
.resource-table th.col-checkbox,
.resource-table td.col-checkbox {
  width: 2.5rem;
  padding: 0.5rem 0.5rem;
  vertical-align: middle;
}
.col-checkbox input {
  cursor: pointer;
}
.resource-table th,
.resource-table td {
  padding: 0.5rem 0.75rem;
  text-align: left;
}
.resource-table td {
  border-bottom: 1px solid #e2e8f0;
}
.resource-table th {
  font-weight: 600;
  color: #475569;
  background: linear-gradient(180deg, #f8fbff 0%, #f2f6fc 100%);
  position: sticky;
  top: 0;
  z-index: 1;
}
.resource-table th.sortable {
  cursor: pointer;
  user-select: none;
}
.resource-table th.sortable:hover {
  color: #2563eb;
}
.sort-indicator {
  margin-left: 0.25rem;
  font-size: 0.75rem;
  color: #64748b;
}
.resource-table tbody tr:nth-child(odd),
.resource-table tbody tr:nth-child(even) {
  background: #fff;
}
.resource-table tbody tr.row-clickable {
  cursor: pointer;
  background: #fff;
}
.resource-table tbody tr.row-clickable:hover {
  background: #f3f8ff;
}
.resource-table tbody tr.row-clickable.row-selected {
  background: #eaf3ff;
  box-shadow: inset 3px 0 0 #2563eb;
}
.resource-table td.cell-drillable {
  cursor: pointer;
  color: #2563eb;
}
.resource-table td.cell-drillable:hover {
  text-decoration: underline;
}
.status-pill {
  display: inline-flex;
  align-items: center;
  padding: 0.12rem 0.48rem;
  border-radius: 999px;
  font-size: 0.75rem;
  line-height: 1.2;
  font-weight: 600;
  border: 1px solid transparent;
}
.status-pill.status-ok {
  color: #15803d;
  background: #f0fdf4;
  border-color: #bbf7d0;
}
.status-pill.status-warn {
  color: #b45309;
  background: #fffbeb;
  border-color: #fde68a;
}
.status-pill.status-error {
  color: #b91c1c;
  background: #fef2f2;
  border-color: #fecaca;
}
.status-pill.status-neutral {
  color: #475569;
  background: #f8fafc;
  border-color: #e2e8f0;
}
.pod-rollup-cell {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
}
.pod-rollup-badge {
  display: inline-flex;
  align-items: center;
  border-radius: 999px;
  padding: 2px 8px;
  font-size: 12px;
  font-weight: 600;
  line-height: 1.3;
  border: 1px solid transparent;
}
.pod-rollup-dot {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: currentColor;
  margin-right: 6px;
  opacity: 0.9;
}
.pod-rollup-badge-running {
  background: rgba(34, 197, 94, 0.16);
  color: #166534;
  border-color: rgba(34, 197, 94, 0.35);
}
.pod-rollup-badge-pending {
  background: rgba(245, 158, 11, 0.16);
  color: #92400e;
  border-color: rgba(245, 158, 11, 0.35);
}
.pod-rollup-badge-succeeded {
  background: rgba(100, 116, 139, 0.16);
  color: #334155;
  border-color: rgba(100, 116, 139, 0.35);
}
.pod-rollup-badge-failed {
  background: rgba(239, 68, 68, 0.16);
  color: #991b1b;
  border-color: rgba(239, 68, 68, 0.35);
}
.pod-rollup-badge-abnormal {
  background: rgba(190, 24, 93, 0.18);
  color: #9f1239;
  border-color: rgba(190, 24, 93, 0.35);
}
.pod-rollup-empty {
  color: rgba(15, 23, 42, 0.45);
}
.recent-restart-hot {
  color: #b91c1c;
  font-weight: 600;
}
.resource-table .cell-link {
  cursor: pointer;
  color: #2563eb;
}
.resource-table .cell-link:hover {
  text-decoration: underline;
}
.taint-entry-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  min-height: 1.85rem;
  padding: 0 0.55rem;
  border: 1px solid #bfdbfe;
  border-radius: 999px;
  background: #eff6ff;
  color: #1d4ed8;
  cursor: pointer;
  font: inherit;
}
.taint-entry-btn:hover {
  background: #dbeafe;
  border-color: #93c5fd;
}
.taint-entry-btn-empty {
  border-color: #cbd5e1;
  background: #f8fafc;
  color: #475569;
}
.taint-entry-btn-empty:hover {
  background: #f1f5f9;
  border-color: #94a3b8;
}
.taint-entry-label {
  font-size: 0.68rem;
  font-weight: 700;
  letter-spacing: 0.04em;
}
.taint-entry-value {
  font-size: 0.78rem;
  font-weight: 700;
}
.node-alloc-pill {
  display: inline-flex;
  align-items: center;
  padding: 0.2rem 0.55rem;
  border-radius: 999px;
  background: #f8fafc;
  color: #334155;
  font-size: 0.75rem;
  line-height: 1.3;
  white-space: nowrap;
}
.node-alloc-pill-warn {
  background: #fef3c7;
  color: #b45309;
}
.node-alloc-pill-danger {
  background: #fee2e2;
  color: #b91c1c;
}
.empty-table {
  margin: 1rem 0 0;
  border: 1px dashed #bfcee3;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.88);
  padding: 1.25rem 1rem;
  text-align: center;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.9);
}
.empty-actions {
  margin-top: 0.8rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.45rem;
  flex-wrap: wrap;
}
.empty-emoji {
  font-size: 1.5rem;
  line-height: 1;
}
.empty-title {
  margin: 0;
  font-size: 0.95rem;
  color: #334155;
  font-weight: 600;
}
.empty-desc {
  font-size: 0.8125rem;
  color: #94a3b8;
}
</style>
