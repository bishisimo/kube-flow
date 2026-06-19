<script setup lang="ts">
import { computed, ref, toRef, watch } from "vue";
import { NAlert, NButton } from "naive-ui";
import { stripManagedFields } from "../../utils/yaml";
import ConfigMapEditor from "../ConfigMapEditor.vue";
import SecretEditor from "../SecretEditor.vue";
import WorkloadFormEditor from "./WorkloadFormEditor.vue";
import {
  useResourceEditSession,
  supportsStructuredEdit,
  type EditIntent,
} from "../../features/resourceEdit";
import type { WorkloadDraft } from "../../features/resourceEdit/workloadDraft";
import {
  applyWorkloadDraft,
  validateWorkloadDraftShell,
} from "../../features/resourceEdit/workloadDraft";
import {
  areWorkloadYamlRegionsDirty,
  markWorkloadYamlRegionsClean,
  mergeWorkloadYamlRegions,
  validateWorkloadYamlRegions,
  workloadObjectToYaml,
  type WorkloadYamlRegionState,
} from "../../features/resourceEdit/workloadRegions";
import type { SelectedResource } from "../../features/workbench/contracts";

const props = defineProps<{
  envId: string | null;
  resource: SelectedResource | null;
  rawYaml: string;
  initialIntent?: EditIntent | null;
  refreshYaml: () => Promise<void>;
  strongholdLockedHandler: (message: string, retry: () => void) => Promise<boolean>;
}>();

const emit = defineEmits<{
  (e: "navigate", payload: {
    targetKind: string;
    namespace: string | null;
    resourceName?: string | null;
  }): void;
  (e: "dirty-change", dirty: boolean): void;
}>();

const initialIntentRef = toRef(props, "initialIntent");
const workloadDraft = ref<WorkloadDraft | null>(null);
const workloadBaseline = ref("");
const workloadRegions = ref<WorkloadYamlRegionState[]>([]);
const regionsDirty = ref(false);
const configMapEditorRef = ref<InstanceType<typeof ConfigMapEditor> | null>(null);
const secretEditorRef = ref<InstanceType<typeof SecretEditor> | null>(null);

const session = useResourceEditSession({
  envId: toRef(props, "envId"),
  resource: toRef(props, "resource"),
  rawYaml: toRef(props, "rawYaml"),
  initialIntent: initialIntentRef,
  onRefresh: () => props.refreshYaml(),
  onStrongholdLocked: props.strongholdLockedHandler,
});

const kind = computed(() => props.resource?.kind ?? "");
const isConfigKind = computed(() => kind.value === "ConfigMap" || kind.value === "Secret");
const showWorkloadForm = computed(() => supportsStructuredEdit(kind.value));
const configBaseline = computed(() => stripManagedFields(props.rawYaml));

const formDirty = computed(() => {
  if (!workloadDraft.value) return false;
  return JSON.stringify(workloadDraft.value) !== workloadBaseline.value;
});

const isDirty = computed(() => {
  if (showWorkloadForm.value && session.editability.value.structuredAllowed) {
    return formDirty.value || regionsDirty.value || areWorkloadYamlRegionsDirty(workloadRegions.value);
  }
  if (isConfigKind.value) {
    return session.configYaml.value !== configBaseline.value;
  }
  return false;
});

watch(isDirty, (dirty) => emit("dirty-change", dirty), { immediate: true });

watch(
  () => [kind.value, session.parsedObject.value] as const,
  () => {
    workloadDraft.value = null;
    workloadBaseline.value = "";
    workloadRegions.value = [];
    regionsDirty.value = false;
  },
);

function onWorkloadDraftUpdate(draft: WorkloadDraft) {
  workloadDraft.value = draft;
  if (!workloadBaseline.value) {
    workloadBaseline.value = JSON.stringify(draft);
  }
}

function onWorkloadRegionsUpdate(regions: WorkloadYamlRegionState[]) {
  workloadRegions.value = regions;
}

function onRegionsDirtyChange(dirty: boolean) {
  regionsDirty.value = dirty;
}

watch(
  () => props.rawYaml,
  () => {
    session.resetConfigYaml();
  },
);

function navigateToParent() {
  const e = session.editability.value;
  if (!e.parentKind || !e.parentName || !props.resource) return;
  emit("navigate", {
    targetKind: e.parentKind,
    namespace: props.resource.namespace,
    resourceName: e.parentName,
  });
}

async function applyStructured() {
  if (!session.parsedObject.value || !workloadDraft.value || !kind.value) return;

  const shellError = validateWorkloadDraftShell(workloadDraft.value);
  if (shellError) {
    session.error.value = shellError;
    return;
  }
  const regionError = validateWorkloadYamlRegions(workloadRegions.value);
  if (regionError) {
    session.error.value = regionError;
    return;
  }

  session.error.value = null;
  const fromForm = applyWorkloadDraft(session.parsedObject.value, workloadDraft.value, kind.value);
  const { obj, error } = mergeWorkloadYamlRegions(fromForm, kind.value, workloadRegions.value);
  if (error) {
    session.error.value = error;
    return;
  }

  const yaml = workloadObjectToYaml(obj);
  await session.applyFullYaml(yaml);
  if (!session.error.value) {
    workloadBaseline.value = JSON.stringify(workloadDraft.value);
    workloadRegions.value = markWorkloadYamlRegionsClean(workloadRegions.value);
    regionsDirty.value = false;
  }
}

async function applyConfig() {
  if (kind.value === "ConfigMap") {
    configMapEditorRef.value?.save();
    return;
  }
  secretEditorRef.value?.save();
}

const applyDisabled = computed(() => {
  if (!isConfigKind.value) return false;
  if (kind.value === "ConfigMap") {
    return Boolean(configMapEditorRef.value?.hasEmptyRow);
  }
  return Boolean(secretEditorRef.value?.hasEmptyRow);
});

async function apply() {
  if (showWorkloadForm.value && session.editability.value.structuredAllowed) {
    await applyStructured();
    return;
  }
  if (isConfigKind.value) {
    await applyConfig();
  }
}

defineExpose({
  apply,
  isDirty,
  saving: computed(() => session.saving.value),
  applyDisabled,
});
</script>

<template>
  <div class="structured-panel">
    <div v-if="session.error.value" class="edit-error">{{ session.error.value }}</div>

    <NAlert
      v-if="session.editability.value.reason && !session.editability.value.structuredAllowed"
      type="warning"
      :bordered="false"
      class="editability-banner"
    >
      <div class="banner-row">
        <span>{{ session.editability.value.reason }}</span>
        <NButton
          v-if="session.editability.value.parentKind && session.editability.value.parentName"
          text
          type="primary"
          size="small"
          @click="navigateToParent"
        >
          前往 {{ session.editability.value.parentKind }}/{{ session.editability.value.parentName }}
        </NButton>
      </div>
    </NAlert>

    <div v-if="showWorkloadForm && session.editability.value.structuredAllowed" class="structured-body">
      <div class="structured-scroll">
        <WorkloadFormEditor
          :kind="kind"
          :obj="session.parsedObject.value"
          @update:draft="onWorkloadDraftUpdate"
          @update:regions="onWorkloadRegionsUpdate"
          @regions-dirty-change="onRegionsDirtyChange"
        />
      </div>
    </div>

    <div v-else-if="isConfigKind" class="kv-wrap">
      <ConfigMapEditor
        v-if="kind === 'ConfigMap'"
        ref="configMapEditorRef"
        hide-apply
        :raw-yaml="session.configYaml.value"
        :saving="session.saving.value"
        @save="(y) => session.applyFullYaml(y)"
        @error="session.handleEditorError"
        @update:yaml="session.setConfigYaml"
      />
      <SecretEditor
        v-else
        ref="secretEditorRef"
        hide-apply
        :raw-yaml="session.configYaml.value"
        :saving="session.saving.value"
        @save="(y) => session.applyFullYaml(y)"
        @error="session.handleEditorError"
        @update:yaml="session.setConfigYaml"
      />
    </div>

    <div v-else class="fallback re-fallback">
      <span class="re-fallback-icon" aria-hidden="true">📄</span>
      <p>此资源类型请使用 YAML 标签进行编辑。</p>
    </div>
  </div>
</template>

<style src="./resourceEditUi.css"></style>
<style scoped>
.structured-panel {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.edit-error {
  flex-shrink: 0;
  padding: 0.5rem 1rem;
  font-size: 0.8125rem;
  color: #dc2626;
  background: #fef2f2;
}
.editability-banner {
  flex-shrink: 0;
  margin: 0.75rem 1rem 0;
}
.banner-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  flex-wrap: wrap;
}
.structured-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.structured-scroll {
  flex: 1;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  -webkit-overflow-scrolling: touch;
}
.kv-wrap {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow-x: hidden;
  overflow-y: auto;
  -webkit-overflow-scrolling: touch;
}
.fallback {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
</style>
