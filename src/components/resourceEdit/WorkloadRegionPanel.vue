<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NButton, NTab, NTabs, useDialog } from "naive-ui";
import YamlRegionEditor from "./YamlRegionEditor.vue";
import {
  WORKLOAD_YAML_REGIONS,
  addYamlRegionItem,
  containerItemImage,
  isWorkloadYamlRegionDirty,
  patchYamlRegion,
  regionsForGroup,
  removeYamlRegionItem,
  updateYamlRegionItem,
  type WorkloadYamlRegionGroup,
  type WorkloadYamlRegionKind,
  type WorkloadYamlRegionState,
} from "../../features/resourceEdit/workloadRegions";

const props = defineProps<{
  group: WorkloadYamlRegionGroup;
  regions: WorkloadYamlRegionState[];
}>();

const emit = defineEmits<{
  (e: "update:regions", value: WorkloadYamlRegionState[]): void;
}>();

const dialog = useDialog();
const activeRegion = ref<WorkloadYamlRegionKind | null>(null);
const activeItemId = ref<string | null>(null);

const visibleRegions = computed(() => regionsForGroup(props.regions, props.group));

/** 可选且为空的列表分区不展示 Tab，避免 Deployment 默认落在空的 Init 容器上。 */
const displayRegions = computed(() =>
  visibleRegions.value.filter((region) => {
    const def = WORKLOAD_YAML_REGIONS.find((d) => d.id === region.id);
    if (!def?.optional || region.mode !== "split-array") return true;
    return region.items.length > 0;
  }),
);

function pickDefaultRegion(regs: WorkloadYamlRegionState[]): WorkloadYamlRegionKind | null {
  if (!regs.length) return null;
  if (props.group === "workload") {
    const containers = regs.find((r) => r.id === "containers");
    if (containers) return containers.id;
  }
  return regs[0].id;
}

watch(
  displayRegions,
  (regs) => {
    if (!regs.length) {
      activeRegion.value = null;
      activeItemId.value = null;
      return;
    }
    if (!regs.some((r) => r.id === activeRegion.value)) {
      activeRegion.value = pickDefaultRegion(regs);
    }
    const current = regs.find((r) => r.id === activeRegion.value);
    if (current?.mode === "split-array") {
      if (!current.items.some((i) => i.id === activeItemId.value)) {
        activeItemId.value = current.items[0]?.id ?? null;
      }
    } else {
      activeItemId.value = null;
    }
  },
  { immediate: true },
);

const currentRegion = computed(
  () => displayRegions.value.find((r) => r.id === activeRegion.value) ?? null,
);

const currentRegionDef = computed(
  () => WORKLOAD_YAML_REGIONS.find((r) => r.id === activeRegion.value) ?? null,
);

const hiddenOptionalRegions = computed(() =>
  visibleRegions.value.filter((region) => {
    const def = WORKLOAD_YAML_REGIONS.find((d) => d.id === region.id);
    return def?.optional && region.mode === "split-array" && region.items.length === 0;
  }),
);

const regionTabLabel = (region: WorkloadYamlRegionState) => {
  const dirty = isWorkloadYamlRegionDirty(region) ? " ·" : "";
  if (region.mode === "split-array") {
    return `${region.label} (${region.items.length})${dirty}`;
  }
  return `${region.label}${dirty}`;
};

function selectRegion(id: WorkloadYamlRegionKind) {
  activeRegion.value = id;
  const region = displayRegions.value.find((r) => r.id === id);
  if (region?.mode === "split-array") {
    activeItemId.value = region.items[0]?.id ?? null;
  }
}

function patchRegion(id: WorkloadYamlRegionKind, next: WorkloadYamlRegionState) {
  emit("update:regions", patchYamlRegion(props.regions, id, next));
}

function onWholeYamlUpdate(yaml: string) {
  const region = currentRegion.value;
  if (!region || region.mode === "split-array") return;
  patchRegion(region.id, { ...region, yaml });
}

function onItemYamlUpdate(yaml: string) {
  const region = currentRegion.value;
  if (!region || region.mode !== "split-array" || !activeItemId.value) return;
  patchRegion(region.id, updateYamlRegionItem(region, activeItemId.value, yaml));
}

function addItem(regionId?: WorkloadYamlRegionKind) {
  const targetId = regionId ?? currentRegion.value?.id;
  const region = props.regions.find((r) => r.id === targetId);
  const def = targetId ? WORKLOAD_YAML_REGIONS.find((r) => r.id === targetId) : null;
  if (!region || !def || region.mode !== "split-array") return;
  const next = addYamlRegionItem(region, def);
  patchRegion(region.id, next);
  activeRegion.value = region.id;
  activeItemId.value = next.items[next.items.length - 1]?.id ?? null;
}

function confirmRemoveItem(itemId: string, label: string) {
  const region = currentRegion.value;
  if (!region) return;
  if (region.id === "containers" && region.items.length <= 1) {
    dialog.warning({
      title: "无法删除",
      content: "至少保留一个业务容器。",
      positiveText: "知道了",
    });
    return;
  }
  dialog.warning({
    title: "删除确认",
    content: `确认删除「${label}」？`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: () => {
      const next = removeYamlRegionItem(region, itemId);
      patchRegion(region.id, next);
      if (activeItemId.value === itemId) {
        activeItemId.value = next.items[0]?.id ?? null;
      }
    },
  });
}

const activeItem = computed(() => {
  const region = currentRegion.value;
  if (!region || region.mode !== "split-array" || !activeItemId.value) return null;
  return region.items.find((i) => i.id === activeItemId.value) ?? null;
});

const activeItemImage = computed(() =>
  activeItem.value ? containerItemImage(activeItem.value.yaml) : "",
);

const editorHeight = computed(() =>
  currentRegion.value?.mode === "split-array" ? "360px" : "300px",
);
</script>

<template>
  <div class="workload-region-panel">
    <div v-if="displayRegions.length > 1" class="region-tabs-wrap">
      <NTabs
        :value="activeRegion ?? undefined"
        type="segment"
        size="small"
        class="re-pod-section-tabs"
        @update:value="selectRegion($event as WorkloadYamlRegionKind)"
      >
        <NTab
          v-for="region in displayRegions"
          :key="region.id"
          :name="region.id"
          :tab="regionTabLabel(region)"
        />
      </NTabs>
    </div>

    <div v-if="hiddenOptionalRegions.length" class="region-optional-actions">
      <span class="region-optional-label">可选配置</span>
      <NButton
        v-for="region in hiddenOptionalRegions"
        :key="region.id"
        size="tiny"
        quaternary
        @click="addItem(region.id)"
      >
        + {{ region.label }}
      </NButton>
    </div>

    <div v-if="currentRegion?.mode === 'split-array'" class="region-split">
      <aside class="region-item-list">
        <div class="region-item-list-head">
          <span class="region-item-list-title">{{ currentRegion.label }}</span>
          <NButton size="tiny" quaternary type="primary" @click="addItem()">+ 添加</NButton>
        </div>

        <div v-if="!currentRegion.items.length" class="region-empty">
          <p>暂无{{ currentRegion.label }}。</p>
          <NButton size="small" type="primary" secondary @click="addItem()">
            添加{{ currentRegion.label }}
          </NButton>
        </div>

        <button
          v-for="item in currentRegion.items"
          :key="item.id"
          type="button"
          class="region-item-row"
          :class="{ 'region-item-row--active': item.id === activeItemId }"
          @click="activeItemId = item.id"
        >
          <div class="region-item-row-main">
            <span class="region-item-row-name">{{ item.label }}</span>
            <span v-if="containerItemImage(item.yaml)" class="region-item-row-image">
              {{ containerItemImage(item.yaml) }}
            </span>
            <span v-else class="region-item-row-image region-item-row-image--muted">未设置镜像</span>
          </div>
          <span
            class="region-item-row-remove"
            title="删除"
            @click.stop="confirmRemoveItem(item.id, item.label)"
          >
            ×
          </span>
        </button>
      </aside>

      <div v-if="activeItem" class="region-editor-wrap">
        <div class="region-editor-head">
          <div class="region-editor-head-main">
            <span class="region-editor-title">{{ activeItem.label }}</span>
            <span v-if="activeItemImage" class="region-editor-image">{{ activeItemImage }}</span>
          </div>
          <span v-if="currentRegionDef" class="region-editor-hint">{{ currentRegionDef.hint }}</span>
        </div>
        <YamlRegionEditor
          :model-value="activeItem.yaml"
          :height="editorHeight"
          @update:model-value="onItemYamlUpdate"
        />
      </div>
    </div>

    <div v-else-if="currentRegion" class="region-whole">
      <div class="region-editor-head">
        <div class="region-editor-head-main">
          <span class="region-editor-title">{{ currentRegion.label }}</span>
        </div>
        <span v-if="currentRegionDef" class="region-editor-hint">{{ currentRegionDef.hint }}</span>
      </div>
      <YamlRegionEditor
        :model-value="currentRegion.yaml"
        :height="editorHeight"
        @update:model-value="onWholeYamlUpdate"
      />
    </div>
  </div>
</template>

<style scoped>
.workload-region-panel {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  min-width: 0;
}

.region-tabs-wrap {
  min-width: 0;
}

.region-optional-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.35rem;
}

.region-optional-label {
  font-size: 0.72rem;
  color: var(--kf-text-muted);
}

.region-split {
  display: grid;
  grid-template-columns: minmax(11rem, 220px) minmax(0, 1fr);
  gap: 0.75rem;
  min-width: 0;
  align-items: start;
}

.region-item-list {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  min-width: 0;
  padding: 0.55rem;
  border: 1px solid var(--kf-border);
  border-radius: 10px;
  background: var(--kf-bg-soft);
}

.region-item-list-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.35rem;
  padding-bottom: 0.25rem;
  border-bottom: 1px dashed color-mix(in srgb, var(--kf-border) 80%, transparent);
}

.region-item-list-title {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--kf-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.03em;
}

.region-item-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 0.35rem;
  align-items: start;
  width: 100%;
  padding: 0.45rem 0.5rem;
  border: 1px solid color-mix(in srgb, var(--kf-border) 85%, transparent);
  border-radius: 8px;
  background: var(--kf-surface-strong);
  color: var(--kf-text-primary);
  text-align: left;
  cursor: pointer;
}

.region-item-row:hover {
  border-color: color-mix(in srgb, var(--kf-primary) 30%, var(--kf-border));
}

.region-item-row--active {
  border-color: color-mix(in srgb, var(--kf-primary) 55%, var(--kf-border));
  background: color-mix(in srgb, var(--kf-primary) 8%, var(--kf-surface-strong));
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--kf-primary) 18%, transparent);
}

.region-item-row-main {
  display: flex;
  flex-direction: column;
  gap: 0.12rem;
  min-width: 0;
}

.region-item-row-name {
  font-size: 0.8rem;
  font-weight: 650;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.region-item-row-image {
  font-size: 0.68rem;
  color: var(--kf-text-secondary);
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.region-item-row-image--muted {
  color: var(--kf-text-muted);
  font-style: italic;
}

.region-item-row-remove {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.1rem;
  height: 1.1rem;
  border-radius: 999px;
  color: var(--kf-text-muted);
  font-size: 0.9rem;
  line-height: 1;
}

.region-item-row-remove:hover {
  background: color-mix(in srgb, var(--kf-danger) 12%, transparent);
  color: var(--kf-danger);
}

.region-editor-wrap,
.region-whole {
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
  min-width: 0;
}

.region-editor-head {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.region-editor-head-main {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 0.45rem;
}

.region-editor-title {
  font-size: 0.84rem;
  font-weight: 650;
  color: var(--kf-text-primary);
}

.region-editor-hint {
  font-size: 0.72rem;
  color: var(--kf-text-muted);
  line-height: 1.4;
}

.region-editor-image {
  font-size: 0.72rem;
  color: var(--kf-text-secondary);
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
}

.region-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.55rem;
  padding: 1rem 0.75rem;
  text-align: center;
  color: var(--kf-text-muted);
  font-size: 0.78rem;
  border: 1px dashed var(--kf-border);
  border-radius: 8px;
  background: var(--kf-surface-strong);
}

@media (max-width: 900px) {
  .region-split {
    grid-template-columns: 1fr;
  }
}
</style>
