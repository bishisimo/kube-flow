<script setup lang="ts">
/**
 * 环境关联的工作台视图元信息：以表格与图表分模块展示切换环境时会恢复的上下文。
 */
import { ref, computed, watch, h } from "vue";
import { NButton, NDataTable, NPopconfirm } from "naive-ui";
import type { DataTableColumns } from "naive-ui";
import type { Environment } from "../../api/env";
import { readEnvViewState, resetEnvViewState } from "../../stores/env";
import {
  buildRecentNamespaceChartItems,
  buildWorkbenchContextTableRows,
  buildWorkbenchFilterTableRows,
  hasActiveWorkbenchFilters,
  type WorkbenchViewTableRow,
} from "../../features/env/envWorkbenchViewMeta";
import {
  clearEnvRecentNamespaces,
  readEnvRecentNamespaces,
} from "../../features/workbench/composables/useWorkbenchRecents";

const props = defineProps<{
  env: Environment | null;
}>();

const recentNamespaces = ref<string[]>([]);
const viewRevision = ref(0);

const viewState = computed(() => {
  if (!props.env) return null;
  void viewRevision.value;
  return readEnvViewState(props.env.id);
});

const contextRows = computed(() =>
  viewState.value ? buildWorkbenchContextTableRows(viewState.value) : []
);

const filterRows = computed(() =>
  viewState.value ? buildWorkbenchFilterTableRows(viewState.value) : []
);

const filtersActive = computed(() =>
  viewState.value ? hasActiveWorkbenchFilters(viewState.value) : false
);

const chartItems = computed(() => buildRecentNamespaceChartItems(recentNamespaces.value));

const metaColumns: DataTableColumns<WorkbenchViewTableRow> = [
  {
    title: "项",
    key: "item",
    width: 88,
    ellipsis: { tooltip: true },
  },
  {
    title: "值",
    key: "value",
    ellipsis: { tooltip: true },
    render(row) {
      return h(
        "span",
        {
          class: row.empty ? "cell-muted" : "cell-value",
          title: row.value,
        },
        row.value
      );
    },
  },
];

function reload() {
  if (!props.env) {
    recentNamespaces.value = [];
    return;
  }
  recentNamespaces.value = readEnvRecentNamespaces(props.env.id);
  viewRevision.value += 1;
}

watch(
  () => props.env?.id,
  () => reload(),
  { immediate: true }
);

function clearWorkbenchMeta() {
  if (!props.env) return;
  resetEnvViewState(props.env.id);
  clearEnvRecentNamespaces(props.env.id);
  reload();
}

defineExpose({ reload });
</script>

<template>
  <div v-if="env" class="panel">
    <header class="panel-header">
      <span class="panel-title">视图快照</span>
      <NPopconfirm @positive-click="clearWorkbenchMeta">
        <template #trigger>
          <NButton size="tiny" quaternary type="error">清除</NButton>
        </template>
        清除后将重置命名空间、资源类型与筛选条件，确认继续？
      </NPopconfirm>
    </header>

    <section class="module">
      <header class="module-head">
        <span class="module-title">上下文</span>
        <span class="module-desc">命名空间与资源类型</span>
      </header>
      <NDataTable
        class="meta-table"
        size="small"
        :bordered="true"
        :single-line="false"
        :columns="metaColumns"
        :data="contextRows"
        :pagination="false"
      />
    </section>

    <section class="module">
      <header class="module-head">
        <span class="module-title">筛选器</span>
        <span class="module-desc">{{ filtersActive ? "已启用" : "未启用" }}</span>
      </header>
      <NDataTable
        class="meta-table"
        size="small"
        :bordered="true"
        :single-line="false"
        :columns="metaColumns"
        :data="filterRows"
        :pagination="false"
      />
    </section>

    <section class="module">
      <header class="module-head">
        <span class="module-title">最近命名空间</span>
        <span class="module-desc">{{ chartItems.length ? `${chartItems.length} 条` : "暂无" }}</span>
      </header>
      <div v-if="!chartItems.length" class="chart-empty">暂无数据</div>
      <div v-else class="chart-panel">
        <div class="chart-axis">
          <span>100%</span>
          <span>50%</span>
          <span>0%</span>
        </div>
        <div class="chart-body">
          <div v-for="item in chartItems" :key="item.name" class="chart-row">
            <div class="chart-label" :title="item.name">
              <span class="chart-rank">#{{ item.rank }}</span>
              <span class="chart-name">{{ item.name }}</span>
            </div>
            <div class="chart-bar-track">
              <div
                class="chart-bar-fill"
                :style="{ width: `${item.percent}%` }"
                :title="`${item.name} · 最近度 ${item.percent}%`"
              />
            </div>
            <span class="chart-percent">{{ item.percent }}%</span>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}
.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}
.panel-title {
  font-size: 0.8125rem;
  font-weight: 600;
  color: var(--kf-text-secondary);
}
.module {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  padding: 0.65rem 0.75rem;
  border: 1px solid var(--kf-border);
  border-radius: 10px;
  background: var(--kf-bg-soft);
}
.module-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 0.5rem;
}
.module-title {
  font-size: 0.8125rem;
  font-weight: 600;
  color: var(--kf-text-primary);
}
.module-desc {
  font-size: 0.6875rem;
  color: var(--kf-text-muted);
  white-space: nowrap;
}
.meta-table :deep(.n-data-table) {
  background: transparent;
}
.meta-table :deep(.n-data-table-th) {
  background: var(--kf-surface-strong);
  font-size: 0.75rem;
}
.meta-table :deep(.n-data-table-td) {
  font-size: 0.8125rem;
  vertical-align: top;
  background: var(--kf-surface-strong);
}
.meta-table :deep(.cell-value) {
  color: var(--kf-text-primary);
  word-break: break-word;
}
.meta-table :deep(.cell-muted) {
  color: var(--kf-text-muted);
}
.chart-empty {
  padding: 1rem 0.75rem;
  border: 1px dashed color-mix(in srgb, var(--kf-border) 80%, transparent);
  border-radius: 8px;
  text-align: center;
  font-size: 0.8125rem;
  color: var(--kf-text-muted);
  background: var(--kf-surface-strong);
}
.chart-panel {
  display: grid;
  grid-template-columns: 2.25rem minmax(0, 1fr);
  gap: 0.45rem;
  padding: 0.55rem 0.65rem;
  border: 1px solid color-mix(in srgb, var(--kf-border) 80%, transparent);
  border-radius: 8px;
  background: var(--kf-surface-strong);
}
.chart-axis {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  align-items: flex-end;
  padding: 1.35rem 0 0.15rem;
  font-size: 0.625rem;
  color: var(--kf-text-muted);
  line-height: 1;
}
.chart-body {
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
  min-width: 0;
}
.chart-row {
  display: grid;
  grid-template-columns: minmax(5.5rem, 34%) minmax(0, 1fr) 2.25rem;
  gap: 0.45rem;
  align-items: center;
}
.chart-label {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  min-width: 0;
}
.chart-rank {
  flex-shrink: 0;
  font-size: 0.6875rem;
  font-weight: 600;
  color: var(--kf-text-muted);
}
.chart-name {
  font-size: 0.8125rem;
  color: var(--kf-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.chart-bar-track {
  height: 0.55rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--kf-border) 55%, var(--kf-surface-strong));
  overflow: hidden;
}
.chart-bar-fill {
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(
    90deg,
    color-mix(in srgb, var(--kf-info) 72%, var(--kf-primary)) 0%,
    color-mix(in srgb, var(--kf-primary) 88%, white) 100%
  );
  transition: width 0.25s ease;
}
.chart-percent {
  font-size: 0.6875rem;
  font-variant-numeric: tabular-nums;
  color: var(--kf-text-muted);
  text-align: right;
}
</style>
