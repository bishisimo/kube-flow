<script setup lang="ts">
/**
 * 编辑环境弹窗：支持本地 context 切换、SSH 隧道参数调整、SSH 凭证管理与环境删除。
 *
 * 保存成功后触发 `saved`；删除成功后触发 `removed`；外部负责刷新列表与关闭弹窗。
 */
import { ref, computed, watch } from "vue";
import { NAlert, NButton, NCheckbox, NInput, NModal, NPopconfirm, NSelect } from "naive-ui";
import {
  envEnsureSshTunnelForHost,
  envListSshConfigHosts,
  envListSshTunnels,
  envUpdate,
  effectiveContext,
  type Environment,
} from "../../api/env";
import { kubeRemoveClient } from "../../api/kube";
import { extractErrorMessage } from "../../utils/errorMessage";
import { strongholdAdjacentModalTrapFocusEnabled } from "../../stores/strongholdAuth";
import { useEnvCredential } from "../../features/env/useEnvCredential";
import EnvTagInput from "./EnvTagInput.vue";
import EnvCredentialPanel from "./EnvCredentialPanel.vue";

const props = defineProps<{
  visible: boolean;
  env: Environment | null;
}>();

const emit = defineEmits<{
  (e: "update:visible", value: boolean): void;
  (e: "saved"): void;
  (e: "removed"): void;
  (e: "context-switch", env: Environment, contextName: string): void;
}>();

const displayName = ref("");
const tags = ref<string[]>([]);
const sshHost = ref("");
const remoteKubeconfigPath = ref("~/.kube/config");
const localPort = ref("");
const sshIdleProtection = ref(false);
const sshConfigHosts = ref<string[]>([]);
const editLoading = ref(false);
const editError = ref("");

const credential = useEnvCredential({
  credentialId: () => props.env?.ssh_tunnel_id ?? null,
  guard: {
    title: "解锁环境凭证",
    description: "保存或清除环境 SSH 密码需要访问凭证存储，请先输入 Stronghold 主密码解锁。",
    lockedSave: "需要先解锁 Stronghold，解锁后会自动继续保存。",
    lockedDelete: "需要先解锁 Stronghold，解锁后会自动继续清除。",
  },
  saveOkText: "密码已保存到安全存储",
  deleteOkText: "密码已清除",
});

const isSsh = computed(() => props.env?.source === "ssh_tunnel");
const hostOptions = computed(() =>
  sshConfigHosts.value.map((h) => ({ label: h, value: h }))
);
const contextOptions = computed(() =>
  (props.env?.contexts ?? []).map((c) => ({
    label: `${c.context_name}${c.cluster_name ? `  ·  ${c.cluster_name}` : ""}`,
    value: c.context_name,
  }))
);

watch(
  () => [props.visible, props.env?.id] as const,
  async ([open, id]) => {
    if (!open || !id || !props.env) return;
    const env = props.env;
    displayName.value = env.display_name;
    tags.value = [...(env.tags ?? [])];
    sshHost.value = "";
    remoteKubeconfigPath.value = "~/.kube/config";
    localPort.value = "";
    sshIdleProtection.value = !!env.ssh_idle_protection;
    editError.value = "";
    credential.reset();

    if (env.source === "ssh_tunnel" && env.ssh_tunnel_id) {
      try {
        const [tunnels, hosts] = await Promise.all([
          envListSshTunnels(),
          envListSshConfigHosts(),
        ]);
        sshConfigHosts.value = hosts;
        const tunnel = tunnels.find((t) => t.id === env.ssh_tunnel_id);
        if (tunnel) {
          sshHost.value = tunnel.ssh_host;
          remoteKubeconfigPath.value = tunnel.remote_kubeconfig_path || "~/.kube/config";
          localPort.value = tunnel.local_port != null ? String(tunnel.local_port) : "";
        } else if (hosts.length) {
          sshHost.value = hosts[0];
        }
      } catch {
        try {
          const hosts = await envListSshConfigHosts();
          sshConfigHosts.value = hosts;
          if (hosts.length) sshHost.value = hosts[0];
        } catch {
          /* noop */
        }
      }
      await credential.refresh();
    }
  },
  { immediate: true }
);

function close() {
  emit("update:visible", false);
}

function parseLocalPort(value: string): number | null {
  const v = value.trim();
  if (!v) return null;
  const n = parseInt(v, 10);
  if (Number.isNaN(n) || n < 1 || n > 65535) return null;
  return n;
}

async function submit() {
  if (!props.env) return;
  editError.value = "";
  editLoading.value = true;
  try {
    const env = props.env;
    const normalizedTags = tags.value.map((t) => t.trim()).filter(Boolean);
    let payload: Environment = {
      ...env,
      display_name: displayName.value.trim(),
      tags: normalizedTags,
    };
    if (env.source === "ssh_tunnel") {
      if (!sshHost.value.trim()) {
        editError.value = "请选择 ~/.ssh/config 中的 Host";
        return;
      }
      const port = parseLocalPort(localPort.value);
      if (localPort.value.trim() && port === null) {
        editError.value = "本地端口需为 1–65535 的整数";
        return;
      }
      const tunnelId = await envEnsureSshTunnelForHost(
        sshHost.value.trim(),
        remoteKubeconfigPath.value.trim() || "~/.kube/config",
        port
      );
      payload = {
        ...payload,
        ssh_tunnel_id: tunnelId,
        ssh_idle_protection: sshIdleProtection.value ? true : null,
      };
      await kubeRemoveClient(env.id);
    }
    await envUpdate(payload);
    emit("saved");
    close();
  } catch (e) {
    editError.value = extractErrorMessage(e);
  } finally {
    editLoading.value = false;
  }
}

function requestRemove() {
  if (!props.env) return;
  emit("removed");
}

function onContextChange(value: string) {
  if (!props.env) return;
  emit("context-switch", props.env, value);
}
</script>

<template>
  <NModal
    :show="visible"
    preset="card"
    title="编辑环境"
    style="width: 520px; max-width: calc(100vw - 32px);"
    :mask-closable="!editLoading"
    :close-on-esc="!editLoading"
    :auto-focus="false"
    :trap-focus="strongholdAdjacentModalTrapFocusEnabled"
    @update:show="(v: boolean) => emit('update:visible', v)"
  >
    <div v-if="env" class="form-body">
      <label class="form-field">
        <span class="field-label">连接名称</span>
        <NInput v-model:value="displayName" />
      </label>

      <label class="form-field">
        <span class="field-label">标签</span>
        <EnvTagInput v-model="tags" />
      </label>

      <template v-if="isSsh">
        <label class="form-field">
          <span class="field-label">~/.ssh/config Host</span>
          <NSelect v-model:value="sshHost" :options="hostOptions" filterable />
        </label>
        <label class="form-field">
          <span class="field-label">远程 kubeconfig 路径</span>
          <NInput v-model:value="remoteKubeconfigPath" placeholder="~/.kube/config" />
        </label>
        <label class="form-field">
          <span class="field-label">本地端口</span>
          <NInput v-model:value="localPort" placeholder="留空=自动分配" />
        </label>
        <NCheckbox v-model:checked="sshIdleProtection">启用空闲保护</NCheckbox>

        <EnvCredentialPanel
          :state="credential"
          title="SSH 认证密码"
          description="保存后每次连接自动读取，无需手动输入。密码加密存储在安全存储中。"
        />
      </template>
      <template v-else>
        <label class="form-field">
          <span class="field-label">当前 Context</span>
          <NSelect
            :value="effectiveContext(env) ?? undefined"
            :options="contextOptions"
            @update:value="onContextChange"
          />
        </label>
      </template>
    </div>

    <NAlert v-if="editError" type="error" :show-icon="false" size="small" class="form-error">
      {{ editError }}
    </NAlert>

    <template #footer>
      <div class="footer">
        <NPopconfirm @positive-click="requestRemove">
          <template #trigger>
            <NButton type="error" ghost :disabled="editLoading">删除环境</NButton>
          </template>
          删除后该环境将从列表中移除，确认删除？
        </NPopconfirm>
        <div class="footer-right">
          <NButton :disabled="editLoading" @click="close">取消</NButton>
          <NButton type="primary" :loading="editLoading" @click="submit">保存</NButton>
        </div>
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.form-body {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
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
.form-error {
  margin-top: 0.6rem;
}
.footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}
.footer-right {
  display: flex;
  gap: 0.5rem;
}
</style>
