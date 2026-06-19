<script setup lang="ts" generic="T">
import { computed, ref, watch } from "vue";
import { NPopconfirm } from "naive-ui";

const props = withDefaults(
  defineProps<{
    items: T[];
    createItem: () => T;
    getLabel: (item: T, index: number) => string;
    emptyHint?: string;
    addLabel?: string;
    /** 更紧凑的 Tab 条 */
    compact?: boolean;
    /** 删除前弹出确认 */
    confirmRemove?: boolean;
    removeConfirmText?: (item: T, index: number) => string;
  }>(),
  {
    emptyHint: "暂无条目",
    addLabel: "添加",
    compact: false,
    confirmRemove: false,
  },
);

const emit = defineEmits<{
  (e: "update:items", value: T[]): void;
}>();

const activeTab = ref("0");

const activeIndex = computed(() => {
  const i = Number(activeTab.value);
  return Number.isFinite(i) && i >= 0 && i < props.items.length ? i : 0;
});

const activeItem = computed(() => props.items[activeIndex.value] ?? null);

const shouldConfirmRemove = computed(() => props.confirmRemove);

function syncActiveTab() {
  const len = props.items.length;
  if (!len) {
    activeTab.value = "0";
    return;
  }
  const i = Number(activeTab.value);
  if (!Number.isFinite(i) || i < 0 || i >= len) {
    activeTab.value = String(Math.max(0, len - 1));
  }
}

watch(() => props.items.length, syncActiveTab, { immediate: true });
watch(() => props.items, syncActiveTab);

function updateAt(index: number, value: T) {
  emit(
    "update:items",
    props.items.map((item, i) => (i === index ? value : item)),
  );
}

function removeAt(index: number) {
  const next = props.items.filter((_, i) => i !== index);
  emit("update:items", next);
  if (!next.length) {
    activeTab.value = "0";
    return;
  }
  activeTab.value = String(Math.min(index, next.length - 1));
}

function addItem() {
  emit("update:items", [...props.items, props.createItem()]);
  activeTab.value = String(props.items.length);
}

function updateActive(value: T) {
  updateAt(activeIndex.value, value);
}

function confirmMessage(item: T, index: number) {
  return props.removeConfirmText?.(item, index) ?? `确认删除「${props.getLabel(item, index)}」？`;
}
</script>

<template>
  <div class="re-item-tabs re-item-tabs--neutral" :class="{ 're-item-tabs--compact': compact }">
    <div class="re-item-tabbar">
      <div class="re-item-tabbar__track">
        <button
          v-for="(item, index) in items"
          :key="index"
          type="button"
          class="re-item-tab"
          :class="{ 're-item-tab--active': activeTab === String(index) }"
          @click="activeTab = String(index)"
        >
          <span class="re-item-tab__label">{{ getLabel(item, index) }}</span>
          <NPopconfirm
            v-if="shouldConfirmRemove"
            :positive-text="'删除'"
            :negative-text="'取消'"
            @positive-click="removeAt(index)"
          >
            <template #trigger>
              <span
                class="re-item-tab__close"
                role="button"
                tabindex="0"
                aria-label="删除"
                @click.stop
                @keydown.enter.prevent.stop
                @keydown.space.prevent.stop
              >
                ×
              </span>
            </template>
            {{ confirmMessage(item, index) }}
          </NPopconfirm>
          <span
            v-else
            class="re-item-tab__close"
            role="button"
            tabindex="0"
            aria-label="删除"
            @click.stop="removeAt(index)"
            @keydown.enter.prevent.stop="removeAt(index)"
            @keydown.space.prevent.stop="removeAt(index)"
          >
            ×
          </span>
        </button>
        <button
          type="button"
          class="re-item-tab-add"
          :aria-label="addLabel"
          @click="addItem"
        >
          +
        </button>
      </div>
    </div>

    <p v-if="!items.length" class="re-empty-hint">{{ emptyHint }}</p>

    <slot
      v-if="activeItem != null"
      :item="activeItem"
      :index="activeIndex"
      :update="updateActive"
    />
  </div>
</template>

<style src="./resourceEditUi.css"></style>
