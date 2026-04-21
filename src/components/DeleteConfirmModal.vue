<script setup lang="ts">
import { computed } from "vue";
import { NAlert, NButton } from "naive-ui";
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

function onConfirm() {
  emit("confirm");
}

function onClose() {
  if (!props.deleting) emit("close");
}

const groupedResources = computed(() => {
  const groups = new Map<string, ResourceRef[]>();
  for (const r of props.resources) {
    const key = r.kind || "Unknown";
    const list = groups.get(key) ?? [];
    list.push(r);
    groups.set(key, list);
  }
  return Array.from(groups.entries()).map(([kind, items]) => ({ kind, items }));
});
</script>

<template>
  <BaseModal :visible="visible" title="删除资源" width="560px" @close="onClose">
    <div class="delete-overview">
      <span class="delete-overview-title">即将删除 {{ resources.length }} 个资源</span>
      <span class="delete-overview-sep">·</span>
      <span class="delete-overview-meta">{{ groupedResources.length }} 种类型</span>
    </div>

    <div class="delete-table-wrap">
      <div class="delete-table-head">
        <span>命名空间</span>
        <span>资源类型</span>
        <span>资源名称</span>
      </div>
      <ul class="delete-table-body">
        <li v-for="(r, i) in resources" :key="`${r.kind}-${r.namespace ?? 'cluster'}-${r.name}-${i}`" class="delete-row">
          <span class="cell ns">{{ r.namespace || "Cluster" }}</span>
          <span class="cell kind">{{ r.kind }}</span>
          <span class="cell name">{{ r.name }}</span>
        </li>
      </ul>
    </div>

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
.delete-overview {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.5rem 0.68rem;
  border-radius: 10px;
  background: color-mix(in srgb, var(--kf-primary-soft, #e8f0ff) 62%, #fff);
  border: 1px solid color-mix(in srgb, var(--kf-primary, #2563eb) 24%, #fff);
  margin-bottom: 0.55rem;
}
.delete-overview-title {
  font-size: 0.84rem;
  font-weight: 650;
  color: #0f172a;
}
.delete-overview-sep {
  color: #94a3b8;
  opacity: 0.9;
}
.delete-overview-meta {
  font-size: 0.76rem;
  color: var(--kf-text-secondary, #64748b);
}
.delete-table-wrap {
  max-height: 300px;
  overflow-y: auto;
  border: 1px solid var(--kf-border, rgba(226, 232, 240, 0.95));
  border-radius: 10px;
  background: #fff;
}
.delete-table-head {
  position: sticky;
  top: 0;
  z-index: 1;
  display: grid;
  grid-template-columns: 1.1fr 1fr 1.8fr;
  gap: 0.5rem;
  padding: 0.52rem 0.68rem;
  background: #f8fafc;
  border-bottom: 1px solid rgba(226, 232, 240, 0.95);
  font-size: 0.72rem;
  font-weight: 700;
  color: #475569;
}
.delete-table-body {
  margin: 0;
  padding: 0;
  list-style: none;
}
.delete-row {
  display: grid;
  grid-template-columns: 1.1fr 1fr 1.8fr;
  gap: 0.5rem;
  align-items: center;
  padding: 0.46rem 0.68rem;
  border-bottom: 1px solid rgba(241, 245, 249, 0.92);
}
.delete-row:last-child {
  border-bottom: none;
}
.cell {
  min-width: 0;
  font-size: 0.8rem;
  line-height: 1.25;
}
.cell.ns {
  color: #334155;
  font-weight: 600;
}
.cell.kind {
  color: #b45309;
  font-weight: 700;
}
.cell.name {
  color: #0f172a;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.modal-error {
  margin: 0.75rem 0 0.25rem;
}
</style>
