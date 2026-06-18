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
  <div class="re-kv-block">
    <div class="re-kv-head">
      <span class="re-kv-title">{{ title }}</span>
      <NButton size="tiny" quaternary class="re-add-btn" @click="addPair(pairs)">+ 添加</NButton>
    </div>
    <p v-if="!pairs.length" class="re-empty-hint">暂无条目，点击「添加」新建</p>
    <div v-for="(pair, i) in pairs" :key="i" class="re-kv-row">
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
      <div class="re-kv-row-actions">
        <NButton text type="error" size="tiny" aria-label="删除" @click="remove(pairs, i)">删除</NButton>
      </div>
    </div>
  </div>
</template>

<style src="./resourceEditUi.css"></style>
