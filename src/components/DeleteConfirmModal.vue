<script setup lang="ts">
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
  <Teleport to="body">
    <div v-if="visible" class="modal-backdrop" @click.self="onClose">
      <div class="modal-panel" role="dialog" aria-modal="true" aria-labelledby="delete-modal-title">
        <h2 id="delete-modal-title" class="modal-title">删除资源</h2>
        <p class="modal-desc">以下资源将被删除，此操作不可恢复。</p>
        <ul class="delete-list">
          <li v-for="(r, i) in resources" :key="i">
            <span class="kind">{{ r.kind }}</span>
            <span class="name">{{ formatResource(r) }}</span>
          </li>
        </ul>
        <p v-if="error" class="modal-error">{{ error }}</p>
        <div class="modal-actions">
          <button type="button" class="btn-secondary" :disabled="deleting" @click="onClose">
            取消
          </button>
          <button type="button" class="btn-danger" :disabled="deleting" @click="onConfirm">
            {{ deleting ? "删除中…" : "确认删除" }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-panel {
  background: var(--bg-card, #fff);
  border-radius: 8px;
  padding: 20px;
  min-width: 360px;
  max-width: 90vw;
  max-height: 80vh;
  overflow: auto;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
}

.modal-title {
  margin: 0 0 12px;
  font-size: 1.1rem;
  font-weight: 600;
}

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
  font-size: 0.85rem;
  color: var(--text-secondary, #666);
  min-width: 100px;
}

.delete-list .name {
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.9rem;
}

.modal-error {
  margin: 0 0 12px;
  color: var(--color-danger, #dc2626);
  font-size: 0.9rem;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.btn-secondary {
  padding: 8px 16px;
  border: 1px solid var(--border-color, #e2e8f0);
  background: var(--bg, #f8fafc);
  border-radius: 6px;
  cursor: pointer;
  font-size: 0.9rem;
}

.btn-secondary:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.btn-danger {
  padding: 8px 16px;
  border: none;
  background: var(--color-danger, #dc2626);
  color: white;
  border-radius: 6px;
  cursor: pointer;
  font-size: 0.9rem;
}

.btn-danger:disabled {
  opacity: 0.7;
  cursor: not-allowed;
}
</style>
