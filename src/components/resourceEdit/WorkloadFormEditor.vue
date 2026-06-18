<script setup lang="ts">
import { ref, watch } from "vue";
import { NCard, NInput, NInputNumber, NSelect, NSwitch } from "naive-ui";
import KeyValueListEditor from "./KeyValueListEditor.vue";
import ContainerTabsEditor from "./ContainerTabsEditor.vue";
import ItemTabsEditor from "./ItemTabsEditor.vue";
import type { K8sObject } from "../../features/resourceEdit/types";
import {
  createEmptyWorkloadDraft,
  parseWorkloadDraft,
  type TolerationDraft,
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

function createEmptyVolume(): VolumeDraft {
  return {
    name: "",
    type: "emptyDir",
    configMapName: "",
    secretName: "",
    pvcName: "",
    hostPath: "",
  };
}

function createEmptyToleration(): TolerationDraft {
  return { key: "", operator: "Equal", value: "", effect: "" };
}

function volumeLabel(vol: VolumeDraft, index: number) {
  return vol.name.trim() || `Volume ${index + 1}`;
}

function tolerationLabel(tol: TolerationDraft, index: number) {
  return tol.key.trim() || `Toleration ${index + 1}`;
}
</script>

<template>
  <div class="workload-form">
    <NCard size="small" class="re-module-card">
      <template #header>
        <div class="re-module-head">
          <span class="re-module-accent re-module-accent--meta" />
          <div>
            <div class="re-module-title">Metadata</div>
            <div class="re-module-desc">资源级 labels 与 annotations</div>
          </div>
        </div>
      </template>
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

    <NCard size="small" class="re-module-card">
      <template #header>
        <div class="re-module-head">
          <span class="re-module-accent re-module-accent--spec" />
          <div>
            <div class="re-module-title">Spec</div>
            <div class="re-module-desc">副本、策略与调度参数</div>
          </div>
        </div>
      </template>
      <div v-if="kind === 'Deployment' || kind === 'StatefulSet'" class="re-field">
        <span class="re-label">Replicas</span>
        <NInputNumber v-model:value="draft.replicas" :min="0" size="small" />
      </div>

      <template v-if="kind === 'Deployment'">
        <div class="re-field">
          <span class="re-label">Strategy</span>
          <NSelect
            v-model:value="draft.strategyType"
            size="small"
            :options="[
              { label: 'RollingUpdate', value: 'RollingUpdate' },
              { label: 'Recreate', value: 'Recreate' },
            ]"
          />
        </div>
        <div v-if="draft.strategyType === 'RollingUpdate'" class="re-sub-grid">
          <label class="re-field">
            <span class="re-label">maxSurge</span>
            <NInput v-model:value="draft.maxSurge" size="small" />
          </label>
          <label class="re-field">
            <span class="re-label">maxUnavailable</span>
            <NInput v-model:value="draft.maxUnavailable" size="small" />
          </label>
        </div>
      </template>

      <template v-if="kind === 'StatefulSet'">
        <label class="re-field">
          <span class="re-label">serviceName</span>
          <NInput v-model:value="draft.serviceName" size="small" />
        </label>
        <label class="re-field">
          <span class="re-label">updateStrategy</span>
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
        <label class="re-field">
          <span class="re-label">updateStrategy</span>
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
        <div class="re-sub-grid">
          <label class="re-field">
            <span class="re-label">parallelism</span>
            <NInputNumber v-model:value="draft.parallelism" :min="1" size="small" />
          </label>
          <label class="re-field">
            <span class="re-label">completions</span>
            <NInputNumber v-model:value="draft.completions" :min="1" size="small" />
          </label>
          <label class="re-field">
            <span class="re-label">backoffLimit</span>
            <NInputNumber v-model:value="draft.backoffLimit" :min="0" size="small" />
          </label>
        </div>
      </template>

      <template v-if="kind === 'CronJob'">
        <label class="re-field">
          <span class="re-label">schedule</span>
          <NInput v-model:value="draft.schedule" size="small" placeholder="0 * * * *" />
        </label>
        <div class="re-field row">
          <span class="re-label">suspend</span>
          <NSwitch v-model:value="draft.suspend" />
        </div>
        <label class="re-field">
          <span class="re-label">concurrencyPolicy</span>
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
        <div class="re-sub-grid">
          <label class="re-field">
            <span class="re-label">job parallelism</span>
            <NInputNumber v-model:value="draft.parallelism" :min="1" size="small" />
          </label>
          <label class="re-field">
            <span class="re-label">job completions</span>
            <NInputNumber v-model:value="draft.completions" :min="1" size="small" />
          </label>
          <label class="re-field">
            <span class="re-label">job backoffLimit</span>
            <NInputNumber v-model:value="draft.backoffLimit" :min="0" size="small" />
          </label>
        </div>
      </template>
    </NCard>

    <NCard size="small" class="re-module-card">
      <template #header>
        <div class="re-module-head">
          <span class="re-module-accent re-module-accent--template" />
          <div>
            <div class="re-module-title">Pod Template</div>
            <div class="re-module-desc">initContainers、containers、volumes 与调度</div>
          </div>
        </div>
      </template>
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

      <div class="re-section">
        <h4 class="re-section-title re-section-title--solo">Init Containers</h4>
        <ContainerTabsEditor
          :containers="draft.initContainers"
          init
          empty-hint="无 Init 容器（可选）"
          @update:containers="draft.initContainers = $event"
        />
      </div>

      <div class="re-section">
        <h4 class="re-section-title re-section-title--solo">Containers</h4>
        <ContainerTabsEditor
          :containers="draft.containers"
          empty-hint="至少添加一个业务容器"
          @update:containers="draft.containers = $event"
        />
      </div>

      <div class="re-section">
        <h4 class="re-section-title re-section-title--solo">Volumes</h4>
        <ItemTabsEditor
          :items="draft.volumes"
          variant="volume"
          empty-hint="未配置 Volume"
          :create-item="createEmptyVolume"
          :get-label="volumeLabel"
          add-label="添加 Volume"
          @update:items="draft.volumes = $event"
        >
          <template #default="{ item, update }">
            <div class="re-item-panel">
              <div class="re-field-grid re-field-grid--2">
                <label class="re-field">
                  <span class="re-label">name</span>
                  <NInput
                    :value="item.name"
                    size="small"
                    placeholder="name"
                    @update:value="update({ ...item, name: $event })"
                  />
                </label>
                <label class="re-field">
                  <span class="re-label">type</span>
                  <NSelect
                    :value="item.type"
                    size="small"
                    :options="[
                      { label: 'emptyDir', value: 'emptyDir' },
                      { label: 'configMap', value: 'configMap' },
                      { label: 'secret', value: 'secret' },
                      { label: 'pvc', value: 'pvc' },
                      { label: 'hostPath', value: 'hostPath' },
                    ]"
                    @update:value="update({ ...item, type: $event })"
                  />
                </label>
              </div>
              <label v-if="item.type === 'configMap'" class="re-field">
                <span class="re-label">configMap</span>
                <NInput
                  :value="item.configMapName"
                  size="small"
                  placeholder="configMap name"
                  @update:value="update({ ...item, configMapName: $event })"
                />
              </label>
              <label v-else-if="item.type === 'secret'" class="re-field">
                <span class="re-label">secret</span>
                <NInput
                  :value="item.secretName"
                  size="small"
                  placeholder="secret name"
                  @update:value="update({ ...item, secretName: $event })"
                />
              </label>
              <label v-else-if="item.type === 'pvc'" class="re-field">
                <span class="re-label">claimName</span>
                <NInput
                  :value="item.pvcName"
                  size="small"
                  placeholder="claimName"
                  @update:value="update({ ...item, pvcName: $event })"
                />
              </label>
              <label v-else-if="item.type === 'hostPath'" class="re-field">
                <span class="re-label">hostPath</span>
                <NInput
                  :value="item.hostPath"
                  size="small"
                  placeholder="host path"
                  @update:value="update({ ...item, hostPath: $event })"
                />
              </label>
            </div>
          </template>
        </ItemTabsEditor>
      </div>

      <label class="re-field">
        <span class="re-label">serviceAccountName</span>
        <NInput v-model:value="draft.serviceAccountName" size="small" />
      </label>
      <label class="re-field">
        <span class="re-label">restartPolicy</span>
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

      <div class="re-section">
        <h4 class="re-section-title re-section-title--solo">Tolerations</h4>
        <ItemTabsEditor
          :items="draft.tolerations"
          variant="toleration"
          empty-hint="未配置污点容忍"
          :create-item="createEmptyToleration"
          :get-label="tolerationLabel"
          add-label="添加 Toleration"
          @update:items="draft.tolerations = $event"
        >
          <template #default="{ item, update }">
            <div class="re-item-panel">
              <div class="re-field-grid re-field-grid--2">
                <label class="re-field">
                  <span class="re-label">key</span>
                  <NInput
                    :value="item.key"
                    size="small"
                    placeholder="key"
                    @update:value="update({ ...item, key: $event })"
                  />
                </label>
                <label class="re-field">
                  <span class="re-label">operator</span>
                  <NInput
                    :value="item.operator"
                    size="small"
                    placeholder="Equal"
                    @update:value="update({ ...item, operator: $event })"
                  />
                </label>
                <label class="re-field">
                  <span class="re-label">value</span>
                  <NInput
                    :value="item.value"
                    size="small"
                    placeholder="value"
                    @update:value="update({ ...item, value: $event })"
                  />
                </label>
                <label class="re-field">
                  <span class="re-label">effect</span>
                  <NInput
                    :value="item.effect"
                    size="small"
                    placeholder="NoSchedule"
                    @update:value="update({ ...item, effect: $event })"
                  />
                </label>
              </div>
            </div>
          </template>
        </ItemTabsEditor>
      </div>
    </NCard>
  </div>
</template>

<style src="./resourceEditUi.css"></style>
<style scoped>
.workload-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: 1rem;
  min-width: 0;
}
</style>
