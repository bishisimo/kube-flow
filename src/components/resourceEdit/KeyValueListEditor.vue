<script setup lang="ts">
import { NInput } from "naive-ui";
import type { KeyValuePair } from "../../features/resourceEdit/workloadDraft";

const props = defineProps<{
  title: string;
  pairs: KeyValuePair[];
}>();

const emit = defineEmits<{
  (e: "update:pairs", pairs: KeyValuePair[]): void;
}>();

function updateAt(index: number, patch: Partial<KeyValuePair>) {
  emit(
    "update:pairs",
    props.pairs.map((pair, i) => (i === index ? { ...pair, ...patch } : pair)),
  );
}

function addPair() {
  emit("update:pairs", [...props.pairs, { key: "", value: "" }]);
}

function removeAt(index: number) {
  emit(
    "update:pairs",
    props.pairs.filter((_, i) => i !== index),
  );
}
</script>

<template>
  <div class="re-kv-block re-kv-block--compact">
    <div class="re-kv-head">
      <span class="re-kv-title">{{ title }}</span>
      <button type="button" class="re-kv-add" @click="addPair">+ 添加</button>
    </div>

    <p v-if="!pairs.length" class="re-empty-hint re-empty-hint--compact">暂无条目</p>

    <div v-else class="re-kv-rows" role="list">
      <div
        v-for="(item, index) in pairs"
        :key="index"
        class="re-kv-row"
        role="listitem"
      >
        <NInput
          :value="item.key"
          size="small"
          placeholder="key"
          class="re-kv-key"
          spellcheck="false"
          @update:value="updateAt(index, { key: $event })"
        />
        <span class="re-kv-sep" aria-hidden="true">=</span>
        <NInput
          :value="item.value"
          size="small"
          placeholder="value"
          class="re-kv-value"
          spellcheck="false"
          @update:value="updateAt(index, { value: $event })"
        />
        <button
          type="button"
          class="re-kv-remove"
          aria-label="删除"
          @click="removeAt(index)"
        >
          ×
        </button>
      </div>
    </div>
  </div>
</template>

<style src="./resourceEditUi.css"></style>
<style scoped>
.re-kv-block--compact {
  gap: 0.35rem;
  padding: 0.45rem 0.55rem;
}

.re-kv-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.re-kv-add {
  border: 1px dashed color-mix(in srgb, var(--kf-border) 85%, transparent);
  border-radius: 6px;
  background: transparent;
  color: var(--kf-text-secondary);
  font-size: 0.68rem;
  font-weight: 600;
  line-height: 1.2;
  padding: 0.12rem 0.45rem;
  cursor: pointer;
}

.re-kv-add:hover {
  color: var(--kf-primary);
  border-color: color-mix(in srgb, var(--kf-primary) 45%, var(--kf-border));
  background: color-mix(in srgb, var(--kf-primary) 8%, transparent);
}

.re-kv-rows {
  display: flex;
  flex-direction: column;
  gap: 0.22rem;
}

.re-kv-row {
  display: grid;
  grid-template-columns: minmax(4.5rem, 0.38fr) auto minmax(5rem, 1fr) 1.35rem;
  align-items: center;
  gap: 0.3rem;
  min-width: 0;
}

.re-kv-sep {
  color: var(--kf-text-muted);
  font-size: 0.72rem;
  font-weight: 600;
  flex-shrink: 0;
}

.re-kv-key,
.re-kv-value {
  min-width: 0;
}

.re-kv-key :deep(.n-input),
.re-kv-value :deep(.n-input) {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.72rem;
}

.re-kv-remove {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.35rem;
  height: 1.35rem;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--kf-text-muted);
  font-size: 0.95rem;
  line-height: 1;
  cursor: pointer;
}

.re-kv-remove:hover {
  color: var(--kf-danger);
  background: color-mix(in srgb, var(--kf-danger) 10%, transparent);
}

.re-empty-hint--compact {
  padding: 0.35rem 0.45rem;
  font-size: 0.68rem;
}
</style>
