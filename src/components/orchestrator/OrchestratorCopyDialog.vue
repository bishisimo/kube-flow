<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NButton, NCheckbox, NSelect } from "naive-ui";
import BaseModal from "../base/BaseModal.vue";
import { extractErrorMessage } from "../../utils/errorMessage";
import { useOrchestratorPackagesStore } from "../../stores/orchestratorPackages";
import { useOrchestratorStore } from "../../stores/orchestrator";

const props = defineProps<{
  visible: boolean;
  selectedEnvId: string;
  selectedComponent: string;
  environments: Array<{ id: string; display_name: string }>;
  /** 当前组件未保存草稿数；大于 0 时默认勾选先保存再拷贝 */
  draftCount?: number;
  /** 跨环境拷贝前落盘当前组件草稿，返回成功保存条数 */
  flushDrafts?: () => number;
}>();

const emit = defineEmits<{
  close: [];
  opMessage: [msg: string];
  opError: [msg: string];
}>();

const { copyComponentToEnv } = useOrchestratorPackagesStore();
const { requestSwitchToOrchestrator } = useOrchestratorStore();

const copyTargetEnvId = ref("");
const copyOverwrite = ref(true);
const saveDraftsFirst = ref(true);
const copyLoading = ref(false);

const draftCount = computed(() => Math.max(0, props.draftCount ?? 0));

const copyTargetOptions = computed(() =>
  props.environments.filter((e) => e.id !== props.selectedEnvId).map((e) => ({ label: e.display_name, value: e.id }))
);

watch(
  () => [props.selectedEnvId, props.environments.map((e) => e.id).join(",")] as const,
  () => {
    const candidates = props.environments.filter((e) => e.id !== props.selectedEnvId);
    if (!candidates.length) {
      copyTargetEnvId.value = "";
      return;
    }
    if (!copyTargetEnvId.value || !candidates.some((e) => e.id === copyTargetEnvId.value)) {
      copyTargetEnvId.value = candidates[0].id;
    }
  },
  { immediate: true }
);

watch(
  () => [props.visible, draftCount.value] as const,
  ([vis]) => {
    if (!vis) return;
    saveDraftsFirst.value = draftCount.value > 0;
  }
);

async function onCopyComponentToEnv() {
  if (!props.selectedEnvId || !props.selectedComponent || !copyTargetEnvId.value) return;
  const target = props.environments.find((e) => e.id === copyTargetEnvId.value);
  if (!target) return;
  copyLoading.value = true;
  try {
    let savedDrafts = 0;
    if (saveDraftsFirst.value && draftCount.value > 0 && props.flushDrafts) {
      savedDrafts = props.flushDrafts();
    }
    const result = copyComponentToEnv(
      props.selectedEnvId,
      props.selectedComponent,
      copyTargetEnvId.value,
      target.display_name,
      copyOverwrite.value
    );
    requestSwitchToOrchestrator({
      env_id: copyTargetEnvId.value,
      component: props.selectedComponent,
    });
    const draftHint = savedDrafts > 0 ? `已先保存 ${savedDrafts} 处草稿。` : "";
    emit(
      "opMessage",
      `组件已跨环境拷贝到 ${target.display_name}：新增 ${result.copied}，更新 ${result.updated}，跳过 ${result.skipped}。${draftHint}`
    );
  } catch (e) {
    emit("opError", extractErrorMessage(e));
  } finally {
    copyLoading.value = false;
    emit("close");
  }
}

function onClose() {
  if (copyLoading.value) return;
  emit("close");
}
</script>

<template>
  <BaseModal :visible="visible" title="跨环境拷贝组件" width="480px" @close="onClose">
    <div class="copy-dialog-body">
      <div class="copy-tip">
        将把当前环境下组件 <strong>{{ selectedComponent }}</strong> 的全部资源 YAML 拷贝到目标环境的编排资产（不会写入 Kubernetes 集群）。
      </div>
      <label class="field-label">
        <span>目标环境</span>
        <NSelect
          v-model:value="copyTargetEnvId"
          :options="copyTargetOptions"
          :disabled="copyLoading"
          filterable
          class="env-select-naive"
        />
      </label>
      <NCheckbox
        v-if="draftCount > 0"
        v-model:checked="saveDraftsFirst"
        :disabled="copyLoading"
        class="field-check-naive"
      >
        先保存本组件全部草稿（{{ draftCount }} 处），再拷贝
      </NCheckbox>
      <NCheckbox v-model:checked="copyOverwrite" :disabled="copyLoading" class="field-check-naive">
        覆盖同名资源
      </NCheckbox>
    </div>
    <template #footer>
      <NButton secondary :disabled="copyLoading" @click="onClose">取消</NButton>
      <NButton
        type="primary"
        :disabled="!copyTargetEnvId || copyLoading"
        :loading="copyLoading"
        @click="onCopyComponentToEnv"
      >
        开始拷贝
      </NButton>
    </template>
  </BaseModal>
</template>

<style scoped>
.copy-dialog-body {
  display: grid;
  gap: 0.65rem;
  padding: 0.1rem 0 0.25rem;
}
.field-label {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  font-size: 0.8rem;
  color: #334155;
}
.env-select-naive {
  width: 100%;
  min-width: 0;
}
.field-check-naive {
  font-size: 0.8rem;
  color: #334155;
}
.copy-tip {
  font-size: 0.75rem;
  color: #64748b;
  line-height: 1.45;
}
</style>
