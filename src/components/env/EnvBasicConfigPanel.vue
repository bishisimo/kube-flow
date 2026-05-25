<script setup lang="ts">
/**
 * 环境基础配置面板：连接名称、标签、本地 context 或 SSH 隧道参数、SSH 凭证。
 */
import { ref, computed, watch } from "vue";
import { NAlert, NCheckbox, NInput, NSelect } from "naive-ui";
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
import { useEnvCredential } from "../../features/env/useEnvCredential";
import EnvTagInput from "./EnvTagInput.vue";
import EnvCredentialPanel from "./EnvCredentialPanel.vue";

const props = defineProps<{
  env: Environment | null;
}>();

const emit = defineEmits<{
  (e: "saved"): void;
  (e: "context-switch", env: Environment, contextName: string): void;
}>();

const displayName = ref("");
const tags = ref<string[]>([]);
const sshHost = ref("");
const remoteKubeconfigPath = ref("~/.kube/config");
const localPort = ref("");
const sshIdleProtection = ref(false);
const sshConfigHosts = ref<string[]>([]);
const loading = ref(false);
const error = ref("");

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

async function loadForm() {
  const env = props.env;
  if (!env) return;
  displayName.value = env.display_name;
  tags.value = [...(env.tags ?? [])];
  sshHost.value = "";
  remoteKubeconfigPath.value = "~/.kube/config";
  localPort.value = "";
  sshIdleProtection.value = !!env.ssh_idle_protection;
  error.value = "";
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
}

watch(
  () => props.env?.id,
  () => {
    void loadForm();
  },
  { immediate: true }
);

function parseLocalPort(value: string): number | null {
  const v = value.trim();
  if (!v) return null;
  const n = parseInt(v, 10);
  if (Number.isNaN(n) || n < 1 || n > 65535) return null;
  return n;
}

async function submit(): Promise<boolean> {
  if (!props.env) return false;
  error.value = "";
  loading.value = true;
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
        error.value = "请选择 ~/.ssh/config 中的 Host";
        return false;
      }
      const port = parseLocalPort(localPort.value);
      if (localPort.value.trim() && port === null) {
        error.value = "本地端口需为 1–65535 的整数";
        return false;
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
    return true;
  } catch (e) {
    error.value = extractErrorMessage(e);
    return false;
  } finally {
    loading.value = false;
  }
}

function onContextChange(value: string) {
  if (!props.env) return;
  emit("context-switch", props.env, value);
}

defineExpose({ submit, loading, error });
</script>

<template>
  <div v-if="env" class="panel">
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
.form-error {
  margin-top: 0.25rem;
}
</style>
