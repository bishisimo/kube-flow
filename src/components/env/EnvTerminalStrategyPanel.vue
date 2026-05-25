<script setup lang="ts">
/**
 * 节点终端策略面板：编排 Node/Pod 右键打开终端时的切换步骤流水线。
 */
import { ref, computed, watch } from "vue";
import {
  NAlert,
  NButton,
  NCard,
  NCheckbox,
  NInput,
  NInputGroup,
  NSelect,
} from "naive-ui";
import type { Environment } from "../../api/env";
import {
  buildNodeTerminalCommand,
  createNodeTerminalStep,
  getNodeTerminalStrategy,
  nodeTerminalSwitchUserCredentialId,
  setNodeTerminalStrategy,
  strategyNeedsSwitchUserPassword,
  type NodeTerminalStepConfig,
  type NodeTerminalStepType,
  type NodeTerminalStrategy,
} from "../../stores/nodeTerminalStrategy";
import { extractErrorMessage } from "../../utils/errorMessage";
import { useEnvCredential } from "../../features/env/useEnvCredential";
import EnvCredentialPanel from "./EnvCredentialPanel.vue";

const props = defineProps<{
  env: Environment | null;
}>();

const emit = defineEmits<{
  (e: "saved"): void;
}>();

const STEP_TYPE_OPTIONS: Array<{ label: string; value: NodeTerminalStepType }> = [
  { label: "switch_user", value: "switch_user" },
  { label: "ssh", value: "ssh" },
  { label: "kind_node_exec", value: "kind_node_exec" },
];

function defaultStrategy(envId: string): NodeTerminalStrategy {
  return {
    envId,
    enabled: false,
    nodeAddressTemplate: "{node}",
    steps: [createNodeTerminalStep("ssh", "root")],
    hasSavedPassword: false,
  };
}

const form = ref<NodeTerminalStrategy>(defaultStrategy(""));
const loading = ref(false);
const error = ref("");

const credential = useEnvCredential({
  credentialId: () => (props.env ? nodeTerminalSwitchUserCredentialId(props.env.id) : null),
  guard: {
    title: "解锁终端策略凭证",
    description: "保存或清除切换用户密码需要访问凭证存储，请先输入 Stronghold 主密码解锁。",
    lockedSave: "需要先解锁 Stronghold，解锁后会自动继续保存。",
    lockedDelete: "需要先解锁 Stronghold，解锁后会自动继续清除。",
  },
  saveOkText: "切换用户密码已保存到当前凭证存储后端。",
  deleteOkText: "已清除切换用户密码。",
});

watch(
  () => credential.exists.value,
  (exists) => {
    if (!props.env) return;
    if (form.value.hasSavedPassword !== exists) {
      form.value = { ...form.value, hasSavedPassword: exists };
      setNodeTerminalStrategy(props.env.id, { hasSavedPassword: exists });
    }
  }
);

const preview = computed(() =>
  buildNodeTerminalCommand(form.value.envId ? form.value : null, "node-01")
);

const needsSwitchUserPassword = computed(() =>
  strategyNeedsSwitchUserPassword(form.value)
);

async function loadForm() {
  const env = props.env;
  if (!env) return;
  error.value = "";
  credential.reset();
  form.value = {
    ...(getNodeTerminalStrategy(env.id) ?? defaultStrategy(env.id)),
    envId: env.id,
  };
  await credential.refresh();
}

watch(
  () => props.env?.id,
  () => {
    void loadForm();
  },
  { immediate: true }
);

function stepHint(type: NodeTerminalStepType): string {
  if (type === "switch_user") return "切换到目标用户后继续执行后续步骤。";
  if (type === "kind_node_exec") {
    return "在当前主机执行 docker exec 进入 kind node 容器，user 作为容器内执行用户。";
  }
  return "使用指定用户连接到节点地址模板解析出的目标主机。";
}

function updateField<K extends keyof NodeTerminalStrategy>(key: K, value: NodeTerminalStrategy[K]) {
  form.value = { ...form.value, [key]: value };
}

function updateStep(stepId: string, patch: Partial<NodeTerminalStepConfig>) {
  form.value = {
    ...form.value,
    steps: form.value.steps.map((step) =>
      step.id === stepId ? { ...step, ...patch } : step
    ),
  };
}

function updateStepType(stepId: string, type: NodeTerminalStepType) {
  const current = form.value.steps.find((step) => step.id === stepId);
  updateStep(stepId, { type, user: current?.user?.trim() || "root" });
}

function addStep(type: NodeTerminalStepType) {
  form.value = {
    ...form.value,
    steps: [...form.value.steps, createNodeTerminalStep(type)],
  };
}

function removeStep(stepId: string) {
  const next = form.value.steps.filter((s) => s.id !== stepId);
  form.value = {
    ...form.value,
    steps: next.length ? next : [createNodeTerminalStep("ssh", "root")],
  };
}

async function submit(): Promise<boolean> {
  if (!props.env) return false;
  error.value = "";
  loading.value = true;
  try {
    setNodeTerminalStrategy(props.env.id, form.value);
    emit("saved");
    return true;
  } catch (e) {
    error.value = extractErrorMessage(e);
    return false;
  } finally {
    loading.value = false;
  }
}

defineExpose({ submit, loading, error });
</script>

<template>
  <div v-if="env" class="panel">
    <p class="hint">
      右键 Node 或 Pod 打开节点终端时，会先进入该环境主机，再执行这里配置的切换命令。
    </p>

    <NCheckbox
      :checked="form.enabled"
      @update:checked="(v: boolean) => updateField('enabled', v)"
    >
      启用节点终端切换策略
    </NCheckbox>
    <p class="hint">
      按步骤编排节点终端进入流程。当前支持 <code>switch_user</code>、<code>ssh</code>、<code>kind_node_exec</code> 三种步骤。
    </p>

    <label class="form-field">
      <span class="field-label">节点地址模板</span>
      <NInput
        :value="form.nodeAddressTemplate"
        placeholder="{node}"
        @update:value="(v: string) => updateField('nodeAddressTemplate', v)"
      />
    </label>

    <section class="steps-section">
      <div class="steps-header">
        <span class="field-label">步骤编排</span>
        <NButton size="small" type="primary" ghost @click="addStep('ssh')">
          + 新增步骤
        </NButton>
      </div>
      <div class="steps-list">
        <NCard
          v-for="(step, index) in form.steps"
          :key="step.id"
          size="small"
          class="step-card"
          :bordered="true"
        >
          <div class="step-row">
            <span class="step-index">{{ index + 1 }}</span>
            <NInputGroup class="step-main">
              <NSelect
                :value="step.type"
                :options="STEP_TYPE_OPTIONS"
                style="width: 150px;"
                @update:value="(v: NodeTerminalStepType) => updateStepType(step.id, v)"
              />
              <NInput
                :value="step.user"
                :placeholder="
                  step.type === 'switch_user'
                    ? '目标用户，例如 root / deploy'
                    : step.type === 'kind_node_exec'
                      ? '容器内用户，例如 root'
                      : 'SSH 用户，例如 root'
                "
                @update:value="(v: string) => updateStep(step.id, { user: v })"
              />
            </NInputGroup>
            <NButton
              quaternary
              circle
              size="small"
              :disabled="form.steps.length <= 1"
              @click="removeStep(step.id)"
            >
              ×
            </NButton>
          </div>
          <div class="step-hint">{{ stepHint(step.type) }}</div>
        </NCard>
      </div>
    </section>

    <EnvCredentialPanel
      v-if="needsSwitchUserPassword"
      :state="credential"
      title="切换用户密码"
      description="仅当步骤中包含 switch_user 时需要配置。密码保存在当前凭证存储后端，后端在切换提示出现时会自动写入。"
    />

    <NCard v-if="preview" size="small" class="preview" :bordered="false">
      <div class="preview-host">预览地址：{{ preview.host }}</div>
      <pre class="preview-code">{{ preview.command }}</pre>
    </NCard>
    <NAlert v-else type="info" size="small" :show-icon="false">
      当前策略未启用，或模板无法生成有效命令。
    </NAlert>

    <NAlert v-if="error" type="error" :show-icon="false" size="small" class="form-error">
      {{ error }}
    </NAlert>
  </div>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}
.hint {
  margin: 0;
  font-size: 0.8125rem;
  color: var(--kf-text-secondary);
  line-height: 1.5;
}
.hint code {
  padding: 0 4px;
  border-radius: 4px;
  background: color-mix(in srgb, var(--kf-primary) 12%, var(--kf-bg-soft));
  color: color-mix(in srgb, var(--kf-primary) 78%, var(--kf-text-primary));
  font-size: 0.78rem;
}
.form-field {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}
.field-label {
  font-size: 0.8125rem;
  font-weight: 500;
  color: var(--kf-text-secondary);
}
.steps-section {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
.steps-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.steps-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
.step-card {
  background: var(--kf-bg-soft);
}
.step-row {
  display: flex;
  align-items: center;
  gap: 0.55rem;
}
.step-index {
  flex-shrink: 0;
  width: 1.5rem;
  height: 1.5rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  background: var(--kf-border);
  color: var(--kf-text-secondary);
  font-size: 0.75rem;
  font-weight: 700;
}
.step-main {
  flex: 1;
  min-width: 0;
}
.step-hint {
  margin-top: 0.3rem;
  margin-left: 2.05rem;
  font-size: 0.76rem;
  color: var(--kf-text-muted);
}
.preview {
  background: #0f172a !important;
  color: #e2e8f0;
}
.preview :deep(.n-card__content) {
  padding: 0.6rem 0.85rem;
}
.preview-host {
  font-size: 0.8125rem;
  color: #cbd5e1;
  margin-bottom: 0.35rem;
}
.preview-code {
  margin: 0;
  font-size: 0.82rem;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
  color: #e2e8f0;
}
.form-error {
  margin-top: 0.25rem;
}
</style>
