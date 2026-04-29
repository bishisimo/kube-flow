<script setup lang="ts">
/**
 * 环境列表的标签筛选条：选中任一标签时展示含该标签的环境（OR 语义）。
 */
import { computed } from "vue";
import { NTag } from "naive-ui";

const props = defineProps<{
  tags: string[];
  selected: Set<string>;
}>();

const emit = defineEmits<{
  (e: "toggle", tag: string): void;
  (e: "clear"): void;
}>();

const hasTags = computed(() => props.tags.length > 0);

const activeColor = {
  color: "color-mix(in srgb, var(--kf-primary) 16%, var(--kf-surface-strong))",
  textColor: "var(--kf-primary)",
  borderColor: "color-mix(in srgb, var(--kf-primary) 52%, var(--kf-border))",
} as const;
const idleColor = {
  color: "var(--kf-surface-strong)",
  textColor: "var(--kf-text-secondary)",
  borderColor: "var(--kf-border)",
} as const;
</script>

<template>
  <div v-if="hasTags" class="env-filter-bar">
    <span class="filter-label">按标签筛选：</span>
    <div class="filter-tags">
      <NTag
        v-for="tag in tags"
        :key="tag"
        :color="selected.has(tag) ? activeColor : idleColor"
        size="small"
        round
        class="filter-tag"
        @click="emit('toggle', tag)"
      >
        {{ tag }}
      </NTag>
      <NTag
        v-if="selected.size"
        size="small"
        round
        class="filter-tag clear"
        :color="{
          color: 'var(--kf-surface-strong)',
          textColor: 'var(--kf-text-secondary)',
          borderColor: 'color-mix(in srgb, var(--kf-text-secondary) 40%, var(--kf-border))',
        }"
        @click="emit('clear')"
      >
        清除
      </NTag>
    </div>
  </div>
</template>

<style scoped>
.env-filter-bar {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-bottom: 1rem;
  flex-wrap: wrap;
}
.filter-label {
  font-size: 0.8125rem;
  color: var(--kf-text-secondary);
  flex-shrink: 0;
}
.filter-tags {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.4rem;
}
.filter-tag {
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s, color 0.15s;
}
.filter-tag:hover {
  filter: brightness(0.97);
}
.filter-tag.clear {
  border-style: dashed !important;
}
.filter-tag.clear :deep(.n-tag__border) {
  border-style: dashed;
}
</style>
