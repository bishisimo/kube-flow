<script setup lang="ts">
import { computed } from "vue";
import { formatCodeCell, type DiffRow } from "../../features/orchestrator/yamlDiff";

const props = defineProps<{
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
</script>

<template>
  <Teleport to="body">
    <div v-if="diffRows.length" class="diff-modal-overlay" @click.self="emit('close')">
      <section class="diff-modal" role="dialog" aria-label="差异详情">
        <div class="diff-head">
          <div class="diff-title">
            集群与草稿差异
            <span class="diff-stat added">+{{ diffStats.added }}</span>
            <span class="diff-stat removed">-{{ diffStats.removed }}</span>
            <span class="diff-stat modified">~{{ diffStats.modified }}</span>
          </div>
          <button type="button" class="btn btn-small" @click="emit('close')">关闭差异</button>
        </div>
        <div class="diff-table-wrap">
          <table class="diff-table">
            <tbody>
              <tr v-for="(row, idx) in diffRows" :key="idx" :class="`row-${row.type}`">
                <td class="ln">{{ row.leftLineNo ?? "" }}</td>
                <td class="code left" v-html="formatCodeCell(row, 'left')"></td>
                <td class="ln">{{ row.rightLineNo ?? "" }}</td>
                <td class="code right" v-html="formatCodeCell(row, 'right')"></td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>
    </div>
  </Teleport>
</template>
