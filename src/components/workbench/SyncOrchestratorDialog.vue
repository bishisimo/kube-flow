<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { NButton, NCheckbox, NInput, NRadio, NRadioGroup, NSelect } from "naive-ui";
import BaseModal from "../base/BaseModal.vue";
import type { SyncRelatedRef } from "../../features/workbench/utils/parseWorkloadRefs";

const props = defineProps<{
  visible: boolean;
  components: string[];
  relatedRefs: SyncRelatedRef[];
  loadingRelated: boolean;
  relatedError: string | null;
  initialResourceName: string;
}>();

const emit = defineEmits<{
  close: [];
  sync: [mode: "existing" | "new", componentName: string, selectedRefKeys: string[]];
}>();

const mode = ref<"existing" | "new">("new");
const existingComponent = ref("");
const newComponent = ref("");
const selectedRefKeys = ref<string[]>([]);

function relatedRefKey(r: SyncRelatedRef): string {
  return `${r.kind}|${r.namespace ?? ""}|${r.name}`;
}

watch(
  () => [props.visible, props.components] as const,
  ([vis]) => {
    if (!vis) return;
    mode.value = "new";
    existingComponent.value = props.components[0] ?? "";
    newComponent.value = props.initialResourceName;
    selectedRefKeys.value = [];
  }
);

watch(
  () => props.relatedRefs,
  (refs) => {
    selectedRefKeys.value = refs.map(relatedRefKey);
  }
);

const selectedCount = computed(() => selectedRefKeys.value.length);
const totalCount = computed(() => props.relatedRefs.length);
const allSelected = computed(
  () => props.relatedRefs.length > 0 && selectedRefKeys.value.length === props.relatedRefs.length
);
const componentOptions = computed(() => props.components.map((name) => ({ label: name, value: name })));

function toggleAll(checked: boolean) {
  selectedRefKeys.value = checked ? props.relatedRefs.map(relatedRefKey) : [];
}

function onConfirm() {
  const name = mode.value === "existing"
    ? existingComponent.value.trim()
    : newComponent.value.trim();
  emit("sync", mode.value, name, selectedRefKeys.value);
}
</script>

<template>
  <BaseModal :visible="visible" title="同步到编排中心" width="640px" @close="emit('close')">
      <div class="sync-orchestrator-modal" role="dialog" aria-label="同步到编排中心">
        <h3 class="sync-orchestrator-title">同步到编排中心</h3>
        <p class="sync-orchestrator-desc">
          选择将当前资源同步到哪个应用组件，可用于继续维护已有组件或新建组件。
        </p>
        <div class="sync-orchestrator-mode-row">
          <NRadioGroup v-model:value="mode" name="sync-mode">
            <NRadio value="new">新增应用组件</NRadio>
            <NRadio value="existing" :disabled="components.length === 0">加入已有应用组件</NRadio>
          </NRadioGroup>
        </div>
        <label v-if="mode === 'existing'" class="sync-field">
          <span>已有组件</span>
          <NSelect
            v-model:value="existingComponent"
            class="filter-input"
            :disabled="components.length === 0"
            placeholder="选择组件"
            :options="componentOptions"
          />
        </label>
        <label v-else class="sync-field">
          <span>新组件名称</span>
          <NInput v-model:value="newComponent" type="text" class="filter-input" placeholder="输入应用组件名称" />
        </label>
        <div class="sync-related-panel">
          <div class="sync-related-head">
            <span>关联资源</span>
            <small v-if="totalCount > 0">已选 {{ selectedCount }} / {{ totalCount }}</small>
          </div>
          <div v-if="loadingRelated" class="sync-related-empty">正在解析关联资源…</div>
          <div v-else-if="relatedError" class="sync-related-error">{{ relatedError }}</div>
          <template v-else>
            <label v-if="totalCount > 0" class="sync-select-all">
              <NCheckbox :checked="allSelected" @update:checked="toggleAll">
                全选关联资源
              </NCheckbox>
            </label>
            <div v-if="totalCount > 0" class="sync-related-list">
              <label v-for="r in relatedRefs" :key="relatedRefKey(r)" class="sync-related-item">
                <NCheckbox
                  :checked="selectedRefKeys.includes(relatedRefKey(r))"
                  @update:checked="(checked: boolean) => checked
                    ? selectedRefKeys.push(relatedRefKey(r))
                    : selectedRefKeys = selectedRefKeys.filter((item) => item !== relatedRefKey(r))"
                />
                <span>{{ r.kind }}/{{ r.name }}</span>
                <small>{{ r.namespace || "default" }}</small>
              </label>
            </div>
            <div v-else class="sync-related-empty">当前资源未检测到可关联同步的配置与 RBAC 资源。</div>
          </template>
        </div>
        <div class="sync-orchestrator-actions" v-if="false" />
      </div>
    <template #footer>
      <NButton secondary @click="emit('close')">取消</NButton>
      <NButton
        type="primary"
        :disabled="(mode === 'existing' && !existingComponent) || loadingRelated"
        @click="onConfirm"
      >
            确认同步
      </NButton>
    </template>
  </BaseModal>
</template>
