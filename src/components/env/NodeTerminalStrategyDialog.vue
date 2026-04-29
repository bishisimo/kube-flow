<script setup lang="ts">
/**
 * 节点终端策略弹窗：编排 Node/Pod 右键打开终端时的"主机入口 -> 切换步骤"流水线。
 *
 * 策略数据在 nodeTerminalStrategy store 本地持久化；切换用户密码写入凭证存储。
 */
import { ref, computed, watch } from "vue";
import {
  NAlert,
  NButton,
  NCard,
  NCheckbox,
  NInput,
  NInputGroup,
  NModal,
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
import { strongholdAdjacentModalTrapFocusEnabled } from "../../stores/strongholdAuth";
import { useEnvCredential } from "../../features/env/useEnvCredential";
import EnvCredentialPanel from "./EnvCredentialPanel.vue";

const props = defineProps<{
  visible: boolean;
  env: Environment | null;
}>();

const emit = defineEmits<{
  (e: "update:visible", value: boolean): void;
  (e: "saved"): void;
}>();

const STEP_TYPE_OPTIONS: Array<{ label: string; value: NodeTerminalStepType }> = [
  { label: "switch_user", value: "switch_user" },
  { label: "ssh", value: "ssh" },
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

watch(
  () => [props.visible, props.env?.id] as const,
  async ([open, envId]) => {
    if (!open || !envId || !props.env) return;
    error.value = "";
    credential.reset();
    form.value = {
      ...(getNodeTerminalStrategy(envId) ?? defaultStrategy(envId)),
      envId,
    };
    await credential.refresh();
  },
  { immediate: true }
);

function stepHint(type: NodeTerminalStepType): string {
  return type === "switch_user"
    ? "切换到目标用户后继续执行后续步骤。"
    : "使用指定用户连接到节点地址模板解析出的目标主机。";
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

function close() {
  emit("update:visible", false);
}

async function submit() {
  if (!props.env) return;
  error.value = "";
  loading.value = true;
  try {
    setNodeTerminalStrategy(props.env.id, form.value);
    emit("saved");
    close();
  } catch (e) {
    error.value = extractErrorMessage(e);
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <NModal
    :show="visible"
    preset="card"
    title="节点终端切换策略"
    style="width: 640px; max-width: calc(100vw - 32px);"
    :mask-closable="!loading"
    :close-on-esc="!loading"
    :auto-focus="false"
    :trap-focus="strongholdAdjacentModalTrapFocusEnabled"
    @update:show="(v: boolean) => emit('update:visible', v)"
  >
    <div v-if="env" class="body">
      <p class="hint">
        环境：<strong>{{ env.display_name }}</strong>。右键 Node 或 Pod 打开节点终端时，会先进入该环境主机，再执行这里配置的切换命令。
      </p>

      <NCheckbox
        :checked="form.enabled"
        @update:checked="(v: boolean) => updateField('enabled', v)"
      >
        启用节点终端切换策略
      </NCheckbox>
      <p class="hint">
        按步骤编排节点终端进入流程。当前支持 <code>switch_user</code> 和 <code>ssh</code> 两种步骤，后续也可以继续扩展更多带凭证的步骤。
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
                  :placeholder="step.type === 'switch_user' ? '目标用户，例如 root / deploy' : 'SSH 用户，例如 root'"
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
    </div>

    <NAlert v-if="error" type="error" :show-icon="false" size="small" class="form-error">
      {{ error }}
    </NAlert>

    <template #footer>
      <div class="footer">
        <NButton :disabled="loading" @click="close">取消</NButton>
        <NButton type="primary" :loading="loading" @click="submit">保存策略</NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.body {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}
.hint {
  margin: 0;
  font-size: 0.8125rem;
  color: #64748b;
  line-height: 1.5;
}
.hint code {
  padding: 0 4px;
  border-radius: 4px;
  background: #eef2ff;
  color: #4338ca;
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
  color: #475569;
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
  background: #f8fafc;
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
  background: #e2e8f0;
  color: #475569;
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
  color: #64748b;
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
  margin-top: 0.6rem;
}
.footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
}
</style>
