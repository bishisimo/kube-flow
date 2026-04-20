<script setup lang="ts">
import { computed } from "vue";
import { formatCodeCell, type DiffRow } from "../../features/orchestrator/yamlDiff";

const props = defineProps<{
  visible: boolean;
  loading?: boolean;
  notFound?: boolean;
  diffRows: DiffRow[];
}>();

const emit = defineEmits<{
  close: [];
}>();

const diffStats = computed(() => {
  let added = 0;
  let removed = 0;
  let modified = 0;
  for (const r of props.diffRows) {
    if (r.type === "added") added += 1;
    else if (r.type === "removed") removed += 1;
    else if (r.type === "modified") modified += 1;
  }
  return { added, removed, modified };
});

const hasChanges = computed(() => props.diffRows.some((r) => r.type !== "context"));
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="diff-modal-overlay" @click.self="emit('close')">
      <section class="diff-modal" role="dialog" aria-label="集群 vs 本地配置差异">
        <div class="diff-head">
          <div class="diff-title">
            <span>集群 vs 本地配置</span>
            <template v-if="!loading && !notFound && diffRows.length">
              <span v-if="hasChanges" class="diff-stat added">+{{ diffStats.added }}</span>
              <span v-if="hasChanges" class="diff-stat removed">-{{ diffStats.removed }}</span>
              <span v-if="hasChanges" class="diff-stat modified">~{{ diffStats.modified }}</span>
              <span v-if="!hasChanges" class="diff-stat same">与集群一致</span>
            </template>
          </div>
          <button type="button" class="btn btn-small" @click="emit('close')">关闭</button>
        </div>
        <div class="diff-body">
          <div v-if="loading" class="diff-status">正在获取集群资源…</div>
          <div v-else-if="notFound" class="diff-status diff-not-found">
            <span class="diff-not-found-icon">ⓘ</span>
            集群中尚无此资源，应用后将新建。
          </div>
          <div v-else-if="!diffRows.length" class="diff-status">暂无数据</div>
          <div v-else class="diff-table-wrap">
            <table class="diff-table">
              <tbody>
                <tr v-for="(row, idx) in diffRows" :key="idx" :class="`row-${row.type}`">
                  <td class="ln">{{ row.leftLineNo ?? "" }}</td>
                  <td class="code left" v-html="formatCodeCell(row, 'left')" />
                  <td class="ln">{{ row.rightLineNo ?? "" }}</td>
                  <td class="code right" v-html="formatCodeCell(row, 'right')" />
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.diff-modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(15, 23, 42, 0.42);
  z-index: 1100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.5rem;
}
.diff-modal {
  width: min(96vw, 1200px);
  height: min(88vh, 820px);
  background: #fff;
  border-radius: 14px;
  box-shadow: 0 24px 60px rgba(15, 23, 42, 0.28);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.diff-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.85rem 1.25rem;
  border-bottom: 1px solid #e2e8f0;
  background: linear-gradient(180deg, #f8fbff 0%, #ffffff 100%);
  flex-shrink: 0;
}
.diff-title {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.9375rem;
  font-weight: 700;
  color: #0f172a;
}
.diff-stat {
  padding: 0.1rem 0.45rem;
  border-radius: 999px;
  font-size: 0.72rem;
  font-weight: 700;
}
.diff-stat.added { background: #dcfce7; color: #166534; }
.diff-stat.removed { background: #fee2e2; color: #991b1b; }
.diff-stat.modified { background: #fef3c7; color: #92400e; }
.diff-stat.same { background: #f1f5f9; color: #475569; }
.diff-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.diff-status {
  padding: 1.5rem;
  font-size: 0.875rem;
  color: #64748b;
}
.diff-not-found {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  margin: 1.25rem 1.5rem;
  padding: 0.9rem 1.1rem;
  background: #eff6ff;
  border: 1px solid #bfdbfe;
  border-radius: 12px;
  color: #1d4ed8;
  font-size: 0.875rem;
}
.diff-not-found-icon {
  font-size: 1.1rem;
  flex-shrink: 0;
}
.diff-table-wrap {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.diff-table {
  width: 100%;
  border-collapse: collapse;
  font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  font-size: 0.78rem;
  line-height: 1.5;
  table-layout: fixed;
}
.ln {
  width: 42px;
  min-width: 42px;
  padding: 0 0.5rem;
  text-align: right;
  color: #94a3b8;
  background: #f8fafc;
  user-select: none;
  border-right: 1px solid #e2e8f0;
  vertical-align: top;
  white-space: nowrap;
}
.code {
  padding: 0 0.65rem;
  white-space: pre-wrap;
  word-break: break-all;
  vertical-align: top;
}
.left { border-right: 1px solid #e2e8f0; }
.row-context td { background: #fff; color: #334155; }
.row-added .code.right { background: #f0fdf4; color: #166534; }
.row-added .ln:last-of-type { background: #dcfce7; }
.row-removed .code.left { background: #fff7f7; color: #991b1b; }
.row-removed .ln:first-of-type { background: #fee2e2; }
.row-modified .code.left { background: #fffbeb; color: #92400e; }
.row-modified .code.right { background: #f0fdf4; color: #166534; }
.row-modified .ln:first-of-type { background: #fef3c7; }
.row-modified .ln:last-of-type { background: #dcfce7; }
:deep(.inline-removed) { background: #fca5a5; border-radius: 2px; padding: 0 1px; }
:deep(.inline-added) { background: #86efac; border-radius: 2px; padding: 0 1px; }
</style>
