<script setup lang="ts">
import { NButton, NInput } from "naive-ui";
import type { KeyValuePair } from "../../features/resourceEdit/workloadDraft";

defineProps<{
  title: string;
  pairs: KeyValuePair[];
}>();

const emit = defineEmits<{
  (e: "update:pairs", pairs: KeyValuePair[]): void;
}>();

function addPair(current: KeyValuePair[]) {
  emit("update:pairs", [...current, { key: "", value: "" }]);
}

function remove(current: KeyValuePair[], index: number) {
  emit("update:pairs", current.filter((_, i) => i !== index));
}

function updateKey(current: KeyValuePair[], index: number, key: string) {
  const next = current.map((p, i) => (i === index ? { ...p, key } : p));
  emit("update:pairs", next);
}

function updateValue(current: KeyValuePair[], index: number, value: string) {
  const next = current.map((p, i) => (i === index ? { ...p, value } : p));
  emit("update:pairs", next);
}
</script>

<template>
  <div class="kv-block">
    <div class="kv-head">
      <span class="kv-title">{{ title }}</span>
      <NButton size="tiny" quaternary @click="addPair(pairs)">+ 添加</NButton>
    </div>
    <div v-if="!pairs.length" class="kv-empty">暂无条目</div>
    <div v-for="(pair, i) in pairs" :key="i" class="kv-row">
      <NInput
        :value="pair.key"
        size="small"
        placeholder="key"
        spellcheck="false"
        @update:value="updateKey(pairs, i, $event)"
      />
      <NInput
        :value="pair.value"
        size="small"
        placeholder="value"
        spellcheck="false"
        @update:value="updateValue(pairs, i, $event)"
      />
      <NButton text type="error" size="tiny" @click="remove(pairs, i)">×</NButton>
    </div>
  </div>
</template>

<style scoped>
.kv-block {
  display: grid;
  gap: 0.5rem;
}
.kv-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.kv-title {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--kf-text-secondary);
}
.kv-empty {
  font-size: 0.75rem;
  color: var(--kf-text-muted);
}
.kv-row {
  display: grid;
  grid-template-columns: 1fr 1fr auto;
  gap: 0.5rem;
  align-items: center;
}
</style>
