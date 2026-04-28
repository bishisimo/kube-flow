<script setup lang="ts">
/**
 * 新建环境弹窗：本地 kubeconfig / SSH 隧道两种方式。
 *
 * 创建成功后触发 `created` 事件，外部负责重新加载环境列表并关闭弹窗。
 * SSH 场景若保存密码触发 Stronghold 未解锁错误，会唤起解锁弹窗并在解锁后自动重试保存+关闭。
 */
import { ref, watch, computed } from "vue";
import { NAlert, NButton, NCheckbox, NInput, NModal, NSelect, NTabs, NTab } from "naive-ui";
import {
  envCreateLocal,
  envCreateSshWithHost,
  envListContextsFromKubeconfig,
  envListSshConfigHosts,
  type KubeContextInfo,
} from "../../api/env";
import { credentialSave } from "../../api/credential";
import { useStrongholdAuthStore } from "../../stores/strongholdAuth";
import { extractErrorMessage } from "../../utils/errorMessage";
import EnvTagInput from "./EnvTagInput.vue";

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{
  (e: "update:visible", value: boolean): void;
  (e: "created"): void;
}>();

type EnvType = "local" | "ssh";

const strongholdAuth = useStrongholdAuthStore();

const newType = ref<EnvType>("local");
const displayName = ref("");
const tags = ref<string[]>([]);

const kubeconfigPath = ref("~/.kube/config");
const discoveredContexts = ref<KubeContextInfo[]>([]);
const discoverLoading = ref(false);
const discoverError = ref("");
const selectedContextKeys = ref<string[]>([]);

const sshConfigHosts = ref<string[]>([]);
const sshHost = ref("");
const remoteKubeconfigPath = ref("~/.kube/config");
const localPort = ref("");
const sshIdleProtection = ref(false);
const password = ref("");

const createLoading = ref(false);
const createError = ref("");

const CREDENTIAL_GUARD = {
  title: "解锁环境凭证",
  description: "保存或清除环境 SSH 密码需要访问凭证存储，请先输入 Stronghold 主密码解锁。",
};

const contextOptions = computed(() =>
  discoveredContexts.value.map((c) => ({
    label: `${c.context_name}${c.cluster_name ? `  ·  ${c.cluster_name}` : ""}`,
    value: c.context_name,
  }))
);

const hostOptions = computed(() =>
  sshConfigHosts.value.map((h) => ({ label: h, value: h }))
);

const selectedContexts = computed(() => {
  const keys = new Set(selectedContextKeys.value);
  return discoveredContexts.value.filter((c) => keys.has(c.context_name));
});

function resetForm() {
  newType.value = "local";
  displayName.value = "";
  tags.value = [];
  kubeconfigPath.value = "~/.kube/config";
  discoveredContexts.value = [];
  selectedContextKeys.value = [];
  discoverError.value = "";

  sshHost.value = "";
  remoteKubeconfigPath.value = "~/.kube/config";
  localPort.value = "";
  sshIdleProtection.value = false;
  password.value = "";

  createError.value = "";
}

async function refreshSshHosts() {
  try {
    sshConfigHosts.value = await envListSshConfigHosts();
    if (sshConfigHosts.value.length) sshHost.value = sshConfigHosts.value[0];
  } catch {
    sshConfigHosts.value = [];
  }
}

watch(
  () => props.visible,
  async (open) => {
    if (!open) return;
    resetForm();
    await refreshSshHosts();
  }
);

async function discoverContexts() {
  discoverError.value = "";
  discoverLoading.value = true;
  try {
    const list = await envListContextsFromKubeconfig(kubeconfigPath.value);
    discoveredContexts.value = list;
    selectedContextKeys.value = list.map((c) => c.context_name);
  } catch (e) {
    discoverError.value = extractErrorMessage(e);
    discoveredContexts.value = [];
  } finally {
    discoverLoading.value = false;
  }
}

function parseLocalPort(value: string): number | null {
  const v = value.trim();
  if (!v) return null;
  const n = parseInt(v, 10);
  if (Number.isNaN(n) || n < 1 || n > 65535) return null;
  return n;
}

function close() {
  emit("update:visible", false);
}

async function submit() {
  createError.value = "";
  createLoading.value = true;
  try {
    if (newType.value === "local") {
      if (!displayName.value.trim()) {
        createError.value = "请输入连接名称";
        return;
      }
      if (!selectedContexts.value.length) {
        createError.value = "请至少选择一个 context";
        return;
      }
      await envCreateLocal(
        displayName.value.trim(),
        kubeconfigPath.value,
        selectedContexts.value,
        tags.value
      );
      emit("created");
      close();
      return;
    }

    if (!displayName.value.trim()) {
      createError.value = "请输入连接名称";
      return;
    }
    if (!sshHost.value.trim()) {
      createError.value = "请选择 ~/.ssh/config 中的 Host";
      return;
    }
    const port = parseLocalPort(localPort.value);
    if (localPort.value.trim() && port === null) {
      createError.value = "本地端口需为 1–65535 的整数";
      return;
    }

    const createdEnv = await envCreateSshWithHost(
      displayName.value.trim(),
      sshHost.value.trim(),
      remoteKubeconfigPath.value.trim() || "~/.kube/config",
      port,
      [],
      tags.value,
      sshIdleProtection.value ? true : null
    );
    const tunnelId = createdEnv.ssh_tunnel_id;
    const pwd = password.value;

    const finishCreate = async () => {
      if (pwd && tunnelId) await credentialSave(tunnelId, pwd);
      emit("created");
      close();
    };

    try {
      await finishCreate();
    } catch (e) {
      const message = extractErrorMessage(e);
      const strongholdRequired = await strongholdAuth.checkAndHandle(
        message,
        () => {
          createLoading.value = true;
          void finishCreate()
            .catch((retryErr: unknown) => {
              createError.value = extractErrorMessage(retryErr);
            })
            .finally(() => {
              createLoading.value = false;
            });
        },
        CREDENTIAL_GUARD
      );
      if (strongholdRequired) {
        createError.value = "需要先解锁 Stronghold，解锁后会自动继续保存密码并完成创建。";
        return;
      }
      throw e;
    }
  } catch (e) {
    createError.value = extractErrorMessage(e);
  } finally {
    createLoading.value = false;
  }
}
</script>

<template>
  <NModal
    :show="visible"
    preset="card"
    title="新建环境"
    style="width: 520px; max-width: calc(100vw - 32px);"
    :mask-closable="!createLoading"
    :close-on-esc="!createLoading"
    :auto-focus="false"
    @update:show="(v: boolean) => emit('update:visible', v)"
  >
    <NTabs v-model:value="newType" type="segment" size="medium" animated>
      <NTab name="local">本地 kubeconfig</NTab>
      <NTab name="ssh">SSH 隧道</NTab>
    </NTabs>

    <div class="form-body">
      <template v-if="newType === 'local'">
        <label class="form-field">
          <span class="field-label">连接名称</span>
          <NInput v-model:value="displayName" placeholder="例如：本地 Minikube" />
        </label>

        <label class="form-field">
          <span class="field-label">kubeconfig 路径</span>
          <div class="field-row">
            <NInput v-model:value="kubeconfigPath" placeholder="~/.kube/config" />
            <NButton :loading="discoverLoading" @click="discoverContexts">
              发现 Context
            </NButton>
          </div>
        </label>
        <NAlert v-if="discoverError" type="error" :show-icon="false" size="small">
          {{ discoverError }}
        </NAlert>

        <label class="form-field">
          <span class="field-label">标签</span>
          <EnvTagInput v-model="tags" />
        </label>

        <label v-if="contextOptions.length" class="form-field">
          <span class="field-label">选择要加入的 Context</span>
          <NSelect
            v-model:value="selectedContextKeys"
            multiple
            filterable
            :options="contextOptions"
            placeholder="勾选要纳入的 context"
          />
        </label>
      </template>

      <template v-else>
        <label class="form-field">
          <span class="field-label">连接名称</span>
          <NInput v-model:value="displayName" placeholder="例如：生产跳板机" />
        </label>

        <label class="form-field">
          <span class="field-label">~/.ssh/config Host</span>
          <NSelect
            v-model:value="sshHost"
            :options="hostOptions"
            placeholder="请选择"
            filterable
          />
        </label>
        <NAlert v-if="!sshConfigHosts.length" type="warning" :show-icon="false" size="small">
          未检测到 ~/.ssh/config 中的 Host，请确认本机存在 ~/.ssh/config 且包含 Host 配置。
        </NAlert>

        <label class="form-field">
          <span class="field-label">远程 kubeconfig 路径</span>
          <NInput v-model:value="remoteKubeconfigPath" placeholder="~/.kube/config" />
        </label>

        <label class="form-field">
          <span class="field-label">本地端口</span>
          <NInput v-model:value="localPort" placeholder="留空=自动分配" />
        </label>

        <NCheckbox v-model:checked="sshIdleProtection">启用空闲保护</NCheckbox>

        <label class="form-field">
          <span class="field-label">SSH 认证密码（可选）</span>
          <NInput
            v-model:value="password"
            class="kf-password-input"
            type="password"
            show-password-on="click"
            placeholder="创建后自动保存到安全存储"
          />
        </label>

        <label class="form-field">
          <span class="field-label">标签</span>
          <EnvTagInput v-model="tags" />
        </label>

        <p class="form-hint">
          选择本机 ~/.ssh/config 中的 Host，建立隧道后使用远程主机上的 kubeconfig；留空则自动分配端口，或填写 1–65535 指定固定端口。
        </p>
      </template>
    </div>

    <NAlert v-if="createError" type="error" :show-icon="false" size="small" class="form-error">
      {{ createError }}
    </NAlert>

    <template #footer>
      <div class="footer">
        <NButton :disabled="createLoading" @click="close">取消</NButton>
        <NButton type="primary" :loading="createLoading" @click="submit">创建</NButton>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.form-body {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
  margin-top: 1rem;
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
.field-row {
  display: flex;
  gap: 0.5rem;
  align-items: center;
}
.field-row :first-child {
  flex: 1;
}
.form-hint {
  margin: 0;
  font-size: 0.75rem;
  color: #94a3b8;
  line-height: 1.5;
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
