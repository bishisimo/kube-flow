<script setup lang="ts" generic="T">
import { computed, ref, watch } from "vue";

export type ItemTabsVariant = "container" | "init" | "volume" | "toleration" | "mount" | "port";

const props = withDefaults(
  defineProps<{
    items: T[];
    createItem: () => T;
    getLabel: (item: T, index: number) => string;
    emptyHint?: string;
    variant?: ItemTabsVariant;
    addLabel?: string;
    /** 容器内嵌套时使用更紧凑的 Tab 条 */
    compact?: boolean;
  }>(),
  {
    emptyHint: "暂无条目",
    variant: "container",
    addLabel: "添加",
    compact: false,
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
</script>

<template>
  <div class="re-item-tabs" :class="[`re-item-tabs--${variant}`, { 're-item-tabs--compact': compact }]">
    <div class="re-item-tabbar">
      <div class="re-item-tabbar__track">
        <button
          v-for="(item, index) in items"
          :key="`${variant}-${index}`"
          type="button"
          class="re-item-tab"
          :class="{ 're-item-tab--active': activeTab === String(index) }"
          @click="activeTab = String(index)"
        >
          <span class="re-item-tab__label">{{ getLabel(item, index) }}</span>
          <span
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
