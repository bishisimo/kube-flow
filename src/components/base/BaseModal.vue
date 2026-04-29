<script setup lang="ts">
import { computed } from "vue";
import { NCard, NModal } from "naive-ui";

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

const cardStyle = computed(() => (props.width ? { width: props.width } : undefined));
</script>

<template>
  <NModal
    :show="visible"
    :mask-closable="true"
    :auto-focus="false"
    :trap-focus="false"
    @mask-click="onBackdropClick"
    @close="onBackdropClick"
  >
    <NCard
      role="dialog"
      aria-modal="true"
      class="base-modal-panel"
      :title="title"
      :bordered="false"
      :style="cardStyle"
    >
      <slot />
      <template #footer>
        <div v-if="$slots.footer" class="base-modal-footer">
          <slot name="footer" />
        </div>
      </template>
    </NCard>
  </NModal>
</template>

<style scoped>
.base-modal-panel {
  min-width: 360px;
  max-width: min(92vw, 900px);
  max-height: 80vh;
  overflow: hidden;
  border-radius: 14px;
  box-shadow: 0 18px 48px rgba(15, 23, 42, 0.2);
}

.base-modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.6rem;
}
</style>
