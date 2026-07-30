<script setup lang="ts">
/**
 * 环境列表筛选条：名称搜索 + 标签筛选（标签为 OR 语义）。
 */
import { computed } from "vue";
import { NInput, NTag } from "naive-ui";

const props = defineProps<{
  tags: string[];
  selected: string[];
  query: string;
}>();

const emit = defineEmits<{
  (e: "update:query", value: string): void;
  (e: "toggle", tag: string): void;
  (e: "clear"): void;
}>();

const hasTags = computed(() => props.tags.length > 0);
const hasSelection = computed(() => props.selected.length > 0 || Boolean(props.query.trim()));
const selectedSet = computed(() => new Set(props.selected));

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

function onQueryInput(value: string) {
  emit("update:query", value ?? "");
}
</script>

<template>
  <div class="env-filter-bar">
    <NInput
      :value="query"
      clearable
      size="small"
      placeholder="搜索环境名称、标签、Host、context…"
      class="env-search"
      @update:value="onQueryInput"
    />
    <div v-if="hasTags" class="filter-tags-row">
      <span class="filter-label">按标签筛选：</span>
      <div class="filter-tags">
        <NTag
          v-for="tag in tags"
          :key="tag"
          :color="selectedSet.has(tag) ? activeColor : idleColor"
          size="small"
          round
          class="filter-tag"
          @click="emit('toggle', tag)"
        >
          {{ tag }}
        </NTag>
        <NTag
          v-if="hasSelection"
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
    <div v-else-if="hasSelection" class="filter-tags-row">
      <NTag
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
        清除搜索
      </NTag>
    </div>
  </div>
</template>

<style scoped>
.env-filter-bar {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  margin-bottom: 1rem;
  flex-shrink: 0;
}
.env-search {
  max-width: 28rem;
}
.filter-tags-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
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
