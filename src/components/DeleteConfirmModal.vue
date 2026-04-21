<script setup lang="ts">
import { NAlert, NButton, NTag } from "naive-ui";
import BaseModal from "./base/BaseModal.vue";

export interface ResourceRef {
  kind: string;
  name: string;
  namespace: string | null;
}

const props = defineProps<{
  visible: boolean;
  resources: ResourceRef[];
  deleting?: boolean;
  error?: string | null;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "confirm"): void;
}>();

function formatResource(r: ResourceRef): string {
  return r.namespace ? `${r.namespace}/${r.name}` : r.name;
}

function onConfirm() {
  emit("confirm");
}

function onClose() {
  if (!props.deleting) emit("close");
}
</script>

<template>
  <BaseModal :visible="visible" title="删除资源" width="560px" @close="onClose">
    <p class="modal-desc">以下资源将被删除，此操作不可恢复。</p>
    <ul class="delete-list">
      <li v-for="(r, i) in resources" :key="i">
        <NTag size="small" type="warning" :bordered="false" class="kind">{{ r.kind }}</NTag>
        <span class="name">{{ formatResource(r) }}</span>
      </li>
    </ul>
    <NAlert v-if="error" type="error" :show-icon="true" class="modal-error">
      {{ error }}
    </NAlert>
    <template #footer>
      <NButton secondary :disabled="deleting" @click="onClose">
        取消
      </NButton>
      <NButton type="error" :loading="deleting" @click="onConfirm">
        {{ deleting ? "删除中…" : "确认删除" }}
      </NButton>
    </template>
  </BaseModal>
</template>

<style scoped>
.modal-desc {
  margin: 0 0 12px;
  color: var(--text-secondary, #666);
  font-size: 0.9rem;
}

.delete-list {
  margin: 12px 0 16px;
  padding: 0 0 0 20px;
  max-height: 200px;
  overflow-y: auto;
}

.delete-list li {
  margin: 4px 0;
  display: flex;
  gap: 8px;
  align-items: baseline;
}

.delete-list .kind {
  min-width: 84px;
  text-align: center;
}

.delete-list .name {
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.9rem;
}

.modal-error {
  margin: 0 0 12px;
}
</style>
