<script setup lang="ts">
const props = defineProps<{
  visible: boolean;
  title?: string;
  width?: string;
}>();

const emit = defineEmits<{
  (e: "close"): void;
}>();

function onBackdropClick() {
  emit("close");
}
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="base-modal-backdrop" @click.self="onBackdropClick">
      <div
        class="base-modal-panel"
        role="dialog"
        aria-modal="true"
        :style="width ? { width } : undefined"
      >
        <h2 v-if="title" class="base-modal-title">{{ title }}</h2>
        <slot />
        <div v-if="$slots.footer" class="base-modal-footer">
          <slot name="footer" />
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.base-modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.base-modal-panel {
  background: var(--bg-card, #fff);
  border-radius: 8px;
  padding: 20px;
  min-width: 360px;
  max-width: 90vw;
  max-height: 80vh;
  overflow: auto;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
  width: v-bind("props.width ?? 'auto'");
}

.base-modal-title {
  margin: 0 0 12px;
  font-size: 1.1rem;
  font-weight: 600;
}

.base-modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 16px;
}
</style>
