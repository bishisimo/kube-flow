<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { NInput, NSelect, NTab, NTabs } from "naive-ui";
import CollapsibleModuleCard from "./CollapsibleModuleCard.vue";
import KeyValueListEditor from "./KeyValueListEditor.vue";
import WorkloadRegionPanel from "./WorkloadRegionPanel.vue";
import type { K8sObject } from "../../features/resourceEdit/types";
import {
  createEmptyWorkloadDraft,
  parseWorkloadDraft,
  type WorkloadDraft,
} from "../../features/resourceEdit/workloadDraft";
import { isWorkloadKind } from "../../features/resourceEdit/workloadPaths";
import {
  areWorkloadYamlRegionsDirty,
  extractWorkloadYamlRegions,
  isWorkloadYamlRegionDirty,
  regionsForGroup,
  type WorkloadYamlRegionState,
} from "../../features/resourceEdit/workloadRegions";

const props = defineProps<{
  kind: string;
  obj: K8sObject | null;
}>();

const emit = defineEmits<{
  (e: "update:draft", draft: WorkloadDraft): void;
  (e: "update:regions", regions: WorkloadYamlRegionState[]): void;
  (e: "regions-dirty-change", dirty: boolean): void;
}>();

const draft = ref<WorkloadDraft>(createEmptyWorkloadDraft());
const regions = ref<WorkloadYamlRegionState[]>([]);
const podSection = ref<"labels" | "yaml" | "scheduling" | "pod">("yaml");
const moduleResetKey = ref(0);
const metaExpanded = ref(false);
const specExpanded = ref(false);

const schedulingTabLabel = computed(() => {
  const dirty = regionsForGroup(regions.value, "scheduling").some(isWorkloadYamlRegionDirty);
  return dirty ? "调度 ·" : "调度";
});

const podTabLabel = computed(() => {
  const dirty = regionsForGroup(regions.value, "pod").some(isWorkloadYamlRegionDirty);
  return dirty ? "Pod ·" : "Pod";
});

const labelsTabLabel = computed(() => {
  const dirty = regionsForGroup(regions.value, "template").some(isWorkloadYamlRegionDirty);
  return dirty ? "标签 ·" : "标签";
});

const metaSummary = computed(() => {
  const labels = draft.value.metadata.labels.length;
  const annotations = draft.value.metadata.annotations.length;
  if (!labels && !annotations) return "无 labels / annotations";
  return `${labels} labels · ${annotations} annotations`;
});

const specSummary = computed(() => {
  const dirty = regionsForGroup(regions.value, "resource").some(isWorkloadYamlRegionDirty);
  return dirty ? "已修改" : "YAML";
});

watch(
  () => [props.kind, props.obj] as const,
  ([kind, obj]) => {
    podSection.value = "yaml";
    if (!obj || !isWorkloadKind(kind)) {
      draft.value = createEmptyWorkloadDraft();
      regions.value = [];
      metaExpanded.value = false;
      specExpanded.value = false;
      moduleResetKey.value += 1;
      emit("update:regions", regions.value);
      emit("regions-dirty-change", false);
      return;
    }
    const parsed = parseWorkloadDraft(obj, kind);
    draft.value = parsed;
    regions.value = extractWorkloadYamlRegions(obj, kind);
    metaExpanded.value =
      parsed.metadata.labels.length > 0 || parsed.metadata.annotations.length > 0;
    specExpanded.value = regionsForGroup(regions.value, "resource").some(
      (r) => r.yaml.trim() && r.yaml.trim() !== "{}\n",
    );
    moduleResetKey.value += 1;
    emit("update:draft", draft.value);
    emit("update:regions", regions.value);
    emit("regions-dirty-change", areWorkloadYamlRegionsDirty(regions.value));
  },
  { immediate: true, deep: true },
);

watch(
  draft,
  (v) => emit("update:draft", v),
  { deep: true },
);

watch(
  regions,
  (value) => {
    emit("update:regions", value);
    emit("regions-dirty-change", areWorkloadYamlRegionsDirty(value));
  },
  { deep: true },
);
</script>

<template>
  <div class="workload-form">
    <CollapsibleModuleCard
      title="Metadata"
      accent="meta"
      :summary="metaSummary"
      :default-expanded="metaExpanded"
      :reset-key="moduleResetKey"
    >
      <KeyValueListEditor
        title="Labels"
        :pairs="draft.metadata.labels"
        @update:pairs="draft.metadata.labels = $event"
      />
      <KeyValueListEditor
        title="Annotations"
        :pairs="draft.metadata.annotations"
        @update:pairs="draft.metadata.annotations = $event"
      />
    </CollapsibleModuleCard>

    <CollapsibleModuleCard
      title="Spec"
      accent="spec"
      :summary="specSummary"
      :default-expanded="specExpanded"
      :reset-key="moduleResetKey"
    >
      <WorkloadRegionPanel
        group="resource"
        :regions="regions"
        @update:regions="regions = $event"
      />
    </CollapsibleModuleCard>

    <CollapsibleModuleCard title="Pod Template" accent="template" :collapsible="false">
      <NTabs v-model:value="podSection" type="segment" size="small" class="re-pod-section-tabs">
        <NTab name="yaml" tab="容器 / Volumes" />
        <NTab name="labels" :tab="labelsTabLabel" />
        <NTab name="scheduling" :tab="schedulingTabLabel" />
        <NTab name="pod" :tab="podTabLabel" />
      </NTabs>

      <div v-if="podSection === 'labels'" class="re-pod-section">
        <WorkloadRegionPanel
          group="template"
          :regions="regions"
          @update:regions="regions = $event"
        />
      </div>

      <div v-if="podSection === 'yaml'" class="re-pod-section">
        <WorkloadRegionPanel
          group="workload"
          :regions="regions"
          @update:regions="regions = $event"
        />
      </div>

      <div v-if="podSection === 'scheduling'" class="re-pod-section">
        <WorkloadRegionPanel
          group="scheduling"
          :regions="regions"
          @update:regions="regions = $event"
        />
      </div>

      <div v-if="podSection === 'pod'" class="re-pod-section">
        <div class="re-field-grid re-field-grid--2 re-pod-quick-fields">
          <label class="re-field">
            <span class="re-label">ServiceAccount</span>
            <NInput v-model:value="draft.serviceAccountName" size="small" />
          </label>
          <label class="re-field">
            <span class="re-label">重启策略</span>
            <NSelect
              v-model:value="draft.restartPolicy"
              size="small"
              :options="[
                { label: 'Always', value: 'Always' },
                { label: 'OnFailure', value: 'OnFailure' },
                { label: 'Never', value: 'Never' },
              ]"
            />
          </label>
        </div>

        <WorkloadRegionPanel
          group="pod"
          :regions="regions"
          @update:regions="regions = $event"
        />
      </div>
    </CollapsibleModuleCard>
  </div>
</template>

<style src="./resourceEditUi.css"></style>
<style scoped>
.workload-form {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  padding: 0.75rem;
  min-width: 0;
}

.re-pod-quick-fields {
  margin-bottom: 0.25rem;
}
</style>
