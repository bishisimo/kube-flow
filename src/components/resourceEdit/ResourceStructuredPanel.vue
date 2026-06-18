<script setup lang="ts">
import { computed, ref, toRef, watch } from "vue";
import { NAlert, NButton } from "naive-ui";
import ConfigMapEditor from "../ConfigMapEditor.vue";
import SecretEditor from "../SecretEditor.vue";
import WorkloadFormEditor from "./WorkloadFormEditor.vue";
import {
  useResourceEditSession,
  supportsStructuredEdit,
  type EditIntent,
} from "../../features/resourceEdit";
import type { WorkloadDraft } from "../../features/resourceEdit/workloadDraft";
import { workloadDraftToYaml } from "../../features/resourceEdit/workloadDraft";
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
}>();

const initialIntentRef = toRef(props, "initialIntent");
const workloadDraft = ref<WorkloadDraft | null>(null);

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
  const yaml = workloadDraftToYaml(session.parsedObject.value, workloadDraft.value, kind.value);
  await session.applyFullYaml(yaml);
}

watch(
  () => props.rawYaml,
  () => {
    session.resetConfigYaml();
  },
);
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
      <div class="structured-toolbar re-toolbar">
        <NButton type="primary" size="small" :loading="session.saving.value" @click="applyStructured">
          {{ session.saving.value ? "保存中…" : "应用" }}
        </NButton>
      </div>
      <div class="structured-scroll">
        <WorkloadFormEditor
          :kind="kind"
          :obj="session.parsedObject.value"
          @update:draft="workloadDraft = $event"
        />
      </div>
    </div>

    <div v-else-if="isConfigKind" class="kv-wrap">
      <ConfigMapEditor
        v-if="kind === 'ConfigMap'"
        :raw-yaml="session.configYaml.value"
        :saving="session.saving.value"
        @save="(y) => session.applyFullYaml(y)"
        @error="session.handleEditorError"
        @update:yaml="session.setConfigYaml"
      />
      <SecretEditor
        v-else
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
.structured-toolbar {
  flex-shrink: 0;
  flex-wrap: wrap;
  justify-content: flex-end;
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
