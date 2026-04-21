<script setup lang="ts">
import { ref, watch } from "vue";
import { extractErrorMessage } from "../../utils/errorMessage";
import { useOrchestratorPackagesStore } from "../../stores/orchestratorPackages";

const props = defineProps<{
  visible: boolean;
  selectedEnvId: string;
  selectedComponent: string;
  environments: Array<{ id: string; display_name: string }>;
}>();

const emit = defineEmits<{
  close: [];
  opMessage: [msg: string];
  opError: [msg: string];
}>();

const { copyComponentToEnv } = useOrchestratorPackagesStore();

const copyTargetEnvId = ref("");
const copyOverwrite = ref(true);
const copyLoading = ref(false);

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

async function onCopyComponentToEnv() {
  if (!props.selectedEnvId || !props.selectedComponent || !copyTargetEnvId.value) return;
  const target = props.environments.find((e) => e.id === copyTargetEnvId.value);
  if (!target) return;
  copyLoading.value = true;
  try {
    const result = copyComponentToEnv(
      props.selectedEnvId,
      props.selectedComponent,
      copyTargetEnvId.value,
      target.display_name,
      copyOverwrite.value
    );
    emit("opMessage", `组件已复制到 ${target.display_name}：新增 ${result.copied}，更新 ${result.updated}，跳过 ${result.skipped}`);
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
  <Teleport to="body">
    <div v-if="visible" class="apply-modal-overlay" @click.self="onClose">
      <section class="apply-modal" role="dialog" aria-label="复制组件到环境">
        <header class="apply-head">
          <h3>复制组件到环境</h3>
        </header>
        <div class="apply-body">
          <label class="field-label">
            <span>目标环境</span>
            <select v-model="copyTargetEnvId" class="select copy-select" :disabled="copyLoading">
              <option value="" disabled>选择目标环境</option>
              <option v-for="env in environments.filter((e) => e.id !== selectedEnvId)" :key="env.id" :value="env.id">
                {{ env.display_name }}
              </option>
            </select>
          </label>
          <label class="field-check">
            <input v-model="copyOverwrite" type="checkbox" :disabled="copyLoading" />
            覆盖同名资源
          </label>
          <div class="copy-tip">将复制当前环境下组件 <strong>{{ selectedComponent }}</strong> 的全部资源 YAML。</div>
        </div>
        <footer class="apply-foot">
          <button type="button" class="btn" :disabled="copyLoading" @click="onClose">取消</button>
          <button
            type="button"
            class="btn btn-primary"
            :disabled="!copyTargetEnvId || copyLoading"
            @click="onCopyComponentToEnv"
          >
            {{ copyLoading ? "复制中…" : "开始复制" }}
          </button>
        </footer>
      </section>
    </div>
  </Teleport>
</template>
