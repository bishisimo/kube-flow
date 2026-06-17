<script setup lang="ts">
import { ref, watch } from "vue";
import { NButton, NCard, NInput, NInputNumber, NSelect, NSwitch } from "naive-ui";
import KeyValueListEditor from "./KeyValueListEditor.vue";
import ContainerFormCard from "./ContainerFormCard.vue";
import type { K8sObject } from "../../features/resourceEdit/types";
import {
  createEmptyWorkloadDraft,
  parseWorkloadDraft,
  type ContainerDraft,
  type VolumeDraft,
  type WorkloadDraft,
} from "../../features/resourceEdit/workloadDraft";
import { isWorkloadKind } from "../../features/resourceEdit/workloadPaths";

const props = defineProps<{
  kind: string;
  obj: K8sObject | null;
}>();

const emit = defineEmits<{
  (e: "update:draft", draft: WorkloadDraft): void;
}>();

const draft = ref<WorkloadDraft>(createEmptyWorkloadDraft());

watch(
  () => [props.kind, props.obj] as const,
  ([kind, obj]) => {
    if (!obj || !isWorkloadKind(kind)) {
      draft.value = createEmptyWorkloadDraft();
      return;
    }
    draft.value = parseWorkloadDraft(obj, kind);
    emit("update:draft", draft.value);
  },
  { immediate: true, deep: true },
);

watch(
  draft,
  (v) => emit("update:draft", v),
  { deep: true },
);

function emptyContainer(): ContainerDraft {
  return {
    name: "",
    image: "",
    imagePullPolicy: "IfNotPresent",
    commandText: "",
    argsText: "",
    env: [],
    cpuRequest: "",
    memoryRequest: "",
    cpuLimit: "",
    memoryLimit: "",
    ports: [],
    volumeMounts: [],
  };
}

function addInitContainer() {
  draft.value.initContainers.push(emptyContainer());
}

function addContainer() {
  draft.value.containers.push(emptyContainer());
}

function addVolume() {
  draft.value.volumes.push({
    name: "",
    type: "emptyDir",
    configMapName: "",
    secretName: "",
    pvcName: "",
    hostPath: "",
  });
}

function addToleration() {
  draft.value.tolerations.push({ key: "", operator: "Equal", value: "", effect: "" });
}

function updateVolume(i: number, partial: Partial<VolumeDraft>) {
  draft.value.volumes[i] = { ...draft.value.volumes[i], ...partial };
}
</script>

<template>
  <div class="workload-form">
    <NCard title="Metadata" size="small" class="module-card">
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
    </NCard>

    <NCard title="Spec" size="small" class="module-card">
      <div v-if="kind === 'Deployment' || kind === 'StatefulSet'" class="field">
        <span class="label">Replicas</span>
        <NInputNumber v-model:value="draft.replicas" :min="0" size="small" />
      </div>

      <template v-if="kind === 'Deployment'">
        <div class="field">
          <span class="label">Strategy</span>
          <NSelect
            v-model:value="draft.strategyType"
            size="small"
            :options="[
              { label: 'RollingUpdate', value: 'RollingUpdate' },
              { label: 'Recreate', value: 'Recreate' },
            ]"
          />
        </div>
        <div v-if="draft.strategyType === 'RollingUpdate'" class="sub-grid">
          <label class="field">
            <span class="label">maxSurge</span>
            <NInput v-model:value="draft.maxSurge" size="small" />
          </label>
          <label class="field">
            <span class="label">maxUnavailable</span>
            <NInput v-model:value="draft.maxUnavailable" size="small" />
          </label>
        </div>
      </template>

      <template v-if="kind === 'StatefulSet'">
        <label class="field">
          <span class="label">serviceName</span>
          <NInput v-model:value="draft.serviceName" size="small" />
        </label>
        <label class="field">
          <span class="label">updateStrategy</span>
          <NSelect
            v-model:value="draft.updateStrategyType"
            size="small"
            :options="[
              { label: 'RollingUpdate', value: 'RollingUpdate' },
              { label: 'OnDelete', value: 'OnDelete' },
            ]"
          />
        </label>
      </template>

      <template v-if="kind === 'DaemonSet'">
        <label class="field">
          <span class="label">updateStrategy</span>
          <NSelect
            v-model:value="draft.updateStrategyType"
            size="small"
            :options="[
              { label: 'RollingUpdate', value: 'RollingUpdate' },
              { label: 'OnDelete', value: 'OnDelete' },
            ]"
          />
        </label>
      </template>

      <template v-if="kind === 'Job'">
        <div class="sub-grid">
          <label class="field">
            <span class="label">parallelism</span>
            <NInputNumber v-model:value="draft.parallelism" :min="1" size="small" />
          </label>
          <label class="field">
            <span class="label">completions</span>
            <NInputNumber v-model:value="draft.completions" :min="1" size="small" />
          </label>
          <label class="field">
            <span class="label">backoffLimit</span>
            <NInputNumber v-model:value="draft.backoffLimit" :min="0" size="small" />
          </label>
        </div>
      </template>

      <template v-if="kind === 'CronJob'">
        <label class="field">
          <span class="label">schedule</span>
          <NInput v-model:value="draft.schedule" size="small" placeholder="0 * * * *" />
        </label>
        <div class="field row">
          <span class="label">suspend</span>
          <NSwitch v-model:value="draft.suspend" />
        </div>
        <label class="field">
          <span class="label">concurrencyPolicy</span>
          <NSelect
            v-model:value="draft.concurrencyPolicy"
            size="small"
            :options="[
              { label: 'Allow', value: 'Allow' },
              { label: 'Forbid', value: 'Forbid' },
              { label: 'Replace', value: 'Replace' },
            ]"
          />
        </label>
        <div class="sub-grid">
          <label class="field">
            <span class="label">job parallelism</span>
            <NInputNumber v-model:value="draft.parallelism" :min="1" size="small" />
          </label>
          <label class="field">
            <span class="label">job completions</span>
            <NInputNumber v-model:value="draft.completions" :min="1" size="small" />
          </label>
          <label class="field">
            <span class="label">job backoffLimit</span>
            <NInputNumber v-model:value="draft.backoffLimit" :min="0" size="small" />
          </label>
        </div>
      </template>
    </NCard>

    <NCard title="Pod Template" size="small" class="module-card">
      <KeyValueListEditor
        title="Template Labels"
        :pairs="draft.templateLabels"
        @update:pairs="draft.templateLabels = $event"
      />
      <KeyValueListEditor
        title="Template Annotations"
        :pairs="draft.templateAnnotations"
        @update:pairs="draft.templateAnnotations = $event"
      />

      <div class="section">
        <div class="section-head">
          <h4 class="section-title">Init Containers</h4>
          <NButton size="tiny" quaternary @click="addInitContainer">+ 添加</NButton>
        </div>
        <ContainerFormCard
          v-for="(c, i) in draft.initContainers"
          :key="`init-${i}`"
          :container="c"
          :index="i"
          init
          @update:container="draft.initContainers[i] = $event"
          @remove="draft.initContainers.splice(i, 1)"
        />
      </div>

      <div class="section">
        <div class="section-head">
          <h4 class="section-title">Containers</h4>
          <NButton size="tiny" quaternary @click="addContainer">+ 添加</NButton>
        </div>
        <ContainerFormCard
          v-for="(c, i) in draft.containers"
          :key="`ctr-${i}`"
          :container="c"
          :index="i"
          @update:container="draft.containers[i] = $event"
          @remove="draft.containers.splice(i, 1)"
        />
      </div>

      <div class="section">
        <div class="section-head">
          <h4 class="section-title">Volumes</h4>
          <NButton size="tiny" quaternary @click="addVolume">+ 添加</NButton>
        </div>
        <div v-for="(vol, vi) in draft.volumes" :key="vi" class="volume-row">
          <NInput
            :value="vol.name"
            size="small"
            placeholder="name"
            @update:value="updateVolume(vi, { name: $event })"
          />
          <NSelect
            :value="vol.type"
            size="small"
            :options="[
              { label: 'emptyDir', value: 'emptyDir' },
              { label: 'configMap', value: 'configMap' },
              { label: 'secret', value: 'secret' },
              { label: 'pvc', value: 'pvc' },
              { label: 'hostPath', value: 'hostPath' },
            ]"
            @update:value="updateVolume(vi, { type: $event })"
          />
          <NInput
            v-if="vol.type === 'configMap'"
            :value="vol.configMapName"
            size="small"
            placeholder="configMap name"
            @update:value="updateVolume(vi, { configMapName: $event })"
          />
          <NInput
            v-else-if="vol.type === 'secret'"
            :value="vol.secretName"
            size="small"
            placeholder="secret name"
            @update:value="updateVolume(vi, { secretName: $event })"
          />
          <NInput
            v-else-if="vol.type === 'pvc'"
            :value="vol.pvcName"
            size="small"
            placeholder="claimName"
            @update:value="updateVolume(vi, { pvcName: $event })"
          />
          <NInput
            v-else-if="vol.type === 'hostPath'"
            :value="vol.hostPath"
            size="small"
            placeholder="host path"
            @update:value="updateVolume(vi, { hostPath: $event })"
          />
          <NButton text type="error" size="tiny" @click="draft.volumes.splice(vi, 1)">×</NButton>
        </div>
      </div>

      <label class="field">
        <span class="label">serviceAccountName</span>
        <NInput v-model:value="draft.serviceAccountName" size="small" />
      </label>
      <label class="field">
        <span class="label">restartPolicy</span>
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

      <KeyValueListEditor
        title="nodeSelector"
        :pairs="draft.nodeSelector"
        @update:pairs="draft.nodeSelector = $event"
      />

      <div class="section">
        <div class="section-head">
          <h4 class="section-title">Tolerations</h4>
          <NButton size="tiny" quaternary @click="addToleration">+ 添加</NButton>
        </div>
        <div v-for="(t, ti) in draft.tolerations" :key="ti" class="toleration-row">
          <NInput v-model:value="t.key" size="small" placeholder="key" />
          <NInput v-model:value="t.operator" size="small" placeholder="operator" />
          <NInput v-model:value="t.value" size="small" placeholder="value" />
          <NInput v-model:value="t.effect" size="small" placeholder="effect" />
          <NButton text type="error" size="tiny" @click="draft.tolerations.splice(ti, 1)">×</NButton>
        </div>
      </div>
    </NCard>
  </div>
</template>

<style scoped>
.workload-form {
  display: grid;
  gap: 1rem;
  padding: 1rem;
  min-height: min-content;
}
.module-card {
  border-radius: 12px;
}
.module-card :deep(.n-card__content) {
  display: grid;
  gap: 0.85rem;
}
.field {
  display: grid;
  gap: 0.3rem;
}
.field.row {
  grid-template-columns: auto 1fr;
  align-items: center;
}
.label {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--kf-text-secondary);
}
.sub-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
  gap: 0.75rem;
}
.section {
  display: grid;
  gap: 0.65rem;
  padding-top: 0.5rem;
  border-top: 1px dashed var(--kf-border);
}
.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.section-title {
  margin: 0;
  font-size: 0.8125rem;
  font-weight: 700;
  color: var(--kf-text-primary);
}
.volume-row,
.toleration-row {
  display: grid;
  grid-template-columns: 1fr 120px 1fr auto;
  gap: 0.5rem;
  align-items: center;
}
.toleration-row {
  grid-template-columns: 1fr 90px 1fr 90px auto;
}
</style>
