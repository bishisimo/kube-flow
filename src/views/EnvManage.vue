<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import type { Environment, KubeContextInfo, SshTunnel } from "../api/env";
import { useEnvStore } from "../stores/env";
import { useShellStore } from "../stores/shell";
import { extractErrorMessage } from "../utils/errorMessage";
import { useTagInput } from "../features/env/useTagInput";
import { useStrongholdGuardedAction } from "../features/env/useStrongholdGuardedAction";

defineOptions({ name: "EnvManage" });
import { useStrongholdAuthStore } from "../stores/strongholdAuth";
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
} from "../stores/nodeTerminalStrategy";
import {
  envList,
  envUpdate,
  envSetCurrentContext,
  envListContextsFromKubeconfig,
  envCreateLocal,
  envCreateSshWithHost,
  envListSshConfigHosts,
  envListSshTunnels,
  envEnsureSshTunnelForHost,
  effectiveContext,
} from "../api/env";
import { kubeRemoveClient } from "../api/kube";
import { credentialExists, credentialSave, credentialDelete } from "../api/credential";

const emit = defineEmits<{ (e: "use-env"): void }>();
const { openEnv, removeEnv: storeRemoveEnv, loadEnvironments: syncStoreEnvironments } = useEnvStore();
const { pendingOpen, requestSwitchToShell } = useShellStore();
const strongholdAuth = useStrongholdAuthStore();
const guardedAction = useStrongholdGuardedAction();
const environments = ref<Environment[]>([]);
const sshTunnels = ref<SshTunnel[]>([]);
const listLoading = ref(false);

// 新建弹窗
const showNewModal = ref(false);
const newType = ref<"local" | "ssh">("local");
const newDisplayName = ref("");
const newKubeconfigPath = ref("~/.kube/config");
const discoveredContexts = ref<KubeContextInfo[]>([]);
const discoverLoading = ref(false);
const discoverError = ref("");
const selectedContextKeys = ref<Set<string>>(new Set());
const sshConfigHosts = ref<string[]>([]);
const newSshHost = ref("");
const newRemoteKubeconfigPath = ref("~/.kube/config");
const newLocalPort = ref("");
const newSshIdleProtection = ref(false);
const newPasswordInput = ref("");
const newShowPassword = ref(false);
const createLoading = ref(false);
const createError = ref("");
const newTags = ref<string[]>([]);

// 编辑弹窗
const showEditModal = ref(false);
const editingEnv = ref<Environment | null>(null);
const editDisplayName = ref("");
const editSshHost = ref("");
const editRemoteKubeconfigPath = ref("~/.kube/config");
const editLocalPort = ref("");
const editSshIdleProtection = ref(false);
const editLoading = ref(false);
const editError = ref("");

// SSH 密码持久化配置（编辑弹窗内）
const editTunnelId = ref<string | null>(null);
const editCredentialExists = ref(false);
const editPasswordInput = ref("");
const editShowPassword = ref(false);
const editShowPasswordInput = ref(false);
const editPasswordLoading = ref(false);
const editPasswordMsg = ref<{ type: "ok" | "err"; text: string } | null>(null);
const editTags = ref<string[]>([]);

// 终端策略弹窗
const showStrategyModal = ref(false);
const strategyEnv = ref<Environment | null>(null);
const strategyLoading = ref(false);
const strategyError = ref("");
const strategyForm = ref<NodeTerminalStrategy>({
  envId: "",
  enabled: false,
  nodeAddressTemplate: "{node}",
  steps: [createNodeTerminalStep("ssh", "root")],
  hasSavedPassword: false,
});
const strategyCredentialExists = ref(false);
const strategyPasswordInput = ref("");
const strategyShowPassword = ref(false);
const strategyShowPasswordInput = ref(false);
const strategyPasswordLoading = ref(false);
const strategyPasswordMsg = ref<{ type: "ok" | "err"; text: string } | null>(null);

const strategyPreview = computed(() =>
  buildNodeTerminalCommand(strategyForm.value.envId ? strategyForm.value : null, "node-01")
);

const strategyModeHint = computed(() =>
  "按步骤编排节点终端进入流程。当前支持 `switch_user` 和 `ssh` 两种步骤，后续也可以继续扩展更多带凭证的步骤。"
);

function strategyStepHint(type: NodeTerminalStepType): string {
  return type === "switch_user"
    ? "切换到目标用户后继续执行后续步骤。"
    : "使用指定用户连接到节点地址模板解析出的目标主机。";
}

const ENV_CREDENTIAL_GUARD: Parameters<typeof guardedAction.handleError>[3] = {
  title: "解锁环境凭证",
  description: "保存或清除环境 SSH 密码需要访问凭证存储，请先输入 Stronghold 主密码解锁。",
  lockedText: "需要先解锁 Stronghold，解锁后会自动继续。",
};

const STRATEGY_CREDENTIAL_GUARD: Parameters<typeof guardedAction.handleError>[3] = {
  title: "解锁终端策略凭证",
  description: "保存或清除切换用户密码需要访问凭证存储，请先输入 Stronghold 主密码解锁。",
  lockedText: "需要先解锁 Stronghold，解锁后会自动继续。",
};

const { draft: newTagDraft, addFromDraft: _addNewTagFromDraft, removeTag: removeNewTag, onKeydown: onNewTagKeydown } = useTagInput(newTags);
const { draft: editTagDraft, addFromDraft: _addEditTagFromDraft, removeTag: removeEditTag, onKeydown: onEditTagKeydown } = useTagInput(editTags);

// 按标签筛选（多选：选中任一标签则展示含有该标签的环境）
const selectedFilterTags = ref<Set<string>>(new Set());

const allTags = computed(() => {
  const set = new Set<string>();
  for (const env of environments.value) {
    for (const t of env.tags ?? []) if (t.trim()) set.add(t.trim());
  }
  return [...set].sort((a, b) => a.localeCompare(b));
});

const filteredEnvironments = computed(() => {
  const sel = selectedFilterTags.value;
  if (sel.size === 0) return environments.value;
  return environments.value.filter((env) =>
    (env.tags ?? []).some((t) => sel.has(t.trim()))
  );
});

function toggleFilterTag(tag: string) {
  const next = new Set(selectedFilterTags.value);
  if (next.has(tag)) next.delete(tag);
  else next.add(tag);
  selectedFilterTags.value = next;
}

function clearFilter() {
  selectedFilterTags.value = new Set();
}

async function loadList() {
  listLoading.value = true;
  try {
    const [envs, tunnels] = await Promise.all([envList(), envListSshTunnels()]);
    environments.value = envs;
    sshTunnels.value = tunnels;
    await syncStoreEnvironments();
  } finally {
    listLoading.value = false;
  }
}

function getTunnelForEnv(env: Environment): SshTunnel | undefined {
  if (env.source !== "ssh_tunnel" || !env.ssh_tunnel_id) return undefined;
  return sshTunnels.value.find((t) => t.id === env.ssh_tunnel_id);
}

async function openNewModal() {
  showNewModal.value = true;
  newType.value = "local";
  newDisplayName.value = "";
  newKubeconfigPath.value = "~/.kube/config";
  discoveredContexts.value = [];
  selectedContextKeys.value = new Set();
  discoverError.value = "";
  createError.value = "";
  newSshHost.value = "";
  newRemoteKubeconfigPath.value = "~/.kube/config";
  newLocalPort.value = "";
  newSshIdleProtection.value = false;
  newPasswordInput.value = "";
  newShowPassword.value = false;
  newTags.value = [];
  newTagDraft.value = "";
  try {
    sshConfigHosts.value = await envListSshConfigHosts();
    if (sshConfigHosts.value.length) newSshHost.value = sshConfigHosts.value[0];
  } catch {
    sshConfigHosts.value = [];
  }
}

function closeNewModal() {
  showNewModal.value = false;
  newPasswordInput.value = "";
  newShowPassword.value = false;
}

async function discoverContexts() {
  discoverError.value = "";
  discoverLoading.value = true;
  try {
    const list = await envListContextsFromKubeconfig(newKubeconfigPath.value);
    discoveredContexts.value = list;
    selectedContextKeys.value = new Set(list.map((c) => c.context_name));
  } catch (e) {
    discoverError.value = extractErrorMessage(e);
    discoveredContexts.value = [];
  } finally {
    discoverLoading.value = false;
  }
}

function toggleContext(name: string) {
  const next = new Set(selectedContextKeys.value);
  if (next.has(name)) next.delete(name);
  else next.add(name);
  selectedContextKeys.value = next;
}

function parseLocalPort(s: string): number | null {
  const v = s.trim();
  if (!v) return null;
  const n = parseInt(v, 10);
  if (Number.isNaN(n) || n < 1 || n > 65535) return null;
  return n;
}

async function doCreate() {
  createError.value = "";
  createLoading.value = true;
  try {
    if (newType.value === "local") {
      if (!newDisplayName.value.trim()) {
        createError.value = "请输入连接名称";
        return;
      }
      const selected = discoveredContexts.value.filter((c) =>
        selectedContextKeys.value.has(c.context_name)
      );
      if (!selected.length) {
        createError.value = "请至少选择一个 context";
        return;
      }
      await envCreateLocal(newDisplayName.value.trim(), newKubeconfigPath.value, selected, newTags.value);
    } else {
      if (!newDisplayName.value.trim()) {
        createError.value = "请输入连接名称";
        return;
      }
      if (!newSshHost.value.trim()) {
        createError.value = "请选择 ~/.ssh/config 中的 Host";
        return;
      }
      const port = parseLocalPort(newLocalPort.value);
      if (newLocalPort.value.trim() && port === null) {
        createError.value = "本地端口需为 1–65535 的整数";
        return;
      }
      const createdEnv = await envCreateSshWithHost(
        newDisplayName.value.trim(),
        newSshHost.value.trim(),
        newRemoteKubeconfigPath.value.trim() || "~/.kube/config",
        port,
        [],
        newTags.value,
        newSshIdleProtection.value ? true : null
      );
      const password = newPasswordInput.value;
      const tunnelId = createdEnv.ssh_tunnel_id;
      const finishCreate = async () => {
        if (password && tunnelId) {
          await credentialSave(tunnelId, password);
        }
        await loadList();
        closeNewModal();
      };
      try {
        await finishCreate();
      } catch (e) {
        const message = extractErrorMessage(e);
        const isStrongholdRequired = await strongholdAuth.checkAndHandle(message, () => {
          createLoading.value = true;
          void finishCreate()
            .catch((retryError: unknown) => {
              createError.value =
                retryError instanceof Error ? retryError.message : String(retryError);
            })
            .finally(() => {
              createLoading.value = false;
            });
        }, { title: ENV_CREDENTIAL_GUARD.title, description: ENV_CREDENTIAL_GUARD.description });
        if (isStrongholdRequired) {
          createError.value = "需要先解锁 Stronghold，解锁后会自动继续保存密码并完成创建。";
          return;
        }
        throw e;
      }
      return;
    }
    await loadList();
    closeNewModal();
  } catch (e) {
    createError.value = extractErrorMessage(e);
  } finally {
    createLoading.value = false;
  }
}

async function openEditModal(env: Environment) {
  editingEnv.value = env;
  editDisplayName.value = env.display_name;
  editTags.value = [...(env.tags ?? [])];
  editTagDraft.value = "";
  editError.value = "";
  editSshHost.value = "";
  editRemoteKubeconfigPath.value = "~/.kube/config";
  editSshIdleProtection.value = !!env.ssh_idle_protection;
  // 重置密码区状态
  editTunnelId.value = null;
  editCredentialExists.value = false;
  editPasswordInput.value = "";
  editShowPassword.value = false;
  editShowPasswordInput.value = false;
  editPasswordMsg.value = null;

  if (env.source === "ssh_tunnel" && env.ssh_tunnel_id) {
    editTunnelId.value = env.ssh_tunnel_id;
    try {
      const [tunnels, hosts] = await Promise.all([envListSshTunnels(), envListSshConfigHosts()]);
      sshConfigHosts.value = hosts;
      const tunnel = tunnels.find((t) => t.id === env.ssh_tunnel_id);
      if (tunnel) {
        editSshHost.value = tunnel.ssh_host;
        editRemoteKubeconfigPath.value = tunnel.remote_kubeconfig_path || "~/.kube/config";
        editLocalPort.value = tunnel.local_port != null ? String(tunnel.local_port) : "";
      } else if (hosts.length) {
        editSshHost.value = hosts[0];
      }
      try {
        editCredentialExists.value = await credentialExists(env.ssh_tunnel_id);
      } catch (e) {
        const message = extractErrorMessage(e);
        const isStrongholdRequired = await strongholdAuth.checkAndHandle(message, () => {
          void openEditModal(env);
        }, { title: ENV_CREDENTIAL_GUARD.title, description: ENV_CREDENTIAL_GUARD.description });
        if (!isStrongholdRequired) throw e;
      }
    } catch {
      const hosts = await envListSshConfigHosts();
      sshConfigHosts.value = hosts;
      if (hosts.length) editSshHost.value = hosts[0];
      editLocalPort.value = "";
    }
  }
  showEditModal.value = true;
}

async function handleSavePassword() {
  if (!editTunnelId.value || !editPasswordInput.value) return;
  editPasswordLoading.value = true;
  editPasswordMsg.value = null;
  try {
    await credentialSave(editTunnelId.value, editPasswordInput.value);
    editCredentialExists.value = true;
    editPasswordInput.value = "";
    editShowPasswordInput.value = false;
    editPasswordMsg.value = { type: "ok", text: "密码已保存到安全存储" };
  } catch (e) {
    const handled = await guardedAction.handleError(e, () => void handleSavePassword(), editPasswordMsg, {
      ...ENV_CREDENTIAL_GUARD,
      lockedText: "需要先解锁 Stronghold，解锁后会自动继续保存。",
    });
    if (handled) return;
  } finally {
    editPasswordLoading.value = false;
  }
}

async function handleDeletePassword() {
  if (!editTunnelId.value) return;
  editPasswordLoading.value = true;
  editPasswordMsg.value = null;
  try {
    await credentialDelete(editTunnelId.value);
    editCredentialExists.value = false;
    editPasswordMsg.value = { type: "ok", text: "密码已清除" };
  } catch (e) {
    const handled = await guardedAction.handleError(e, () => void handleDeletePassword(), editPasswordMsg, {
      ...ENV_CREDENTIAL_GUARD,
      lockedText: "需要先解锁 Stronghold，解锁后会自动继续清除。",
    });
    if (handled) return;
  } finally {
    editPasswordLoading.value = false;
  }
}

function closeEditModal() {
  showEditModal.value = false;
  editingEnv.value = null;
}

async function doUpdate() {
  if (!editingEnv.value) return;
  editError.value = "";
  editLoading.value = true;
  try {
    const env = editingEnv.value;
    const tags = editTags.value.map((t) => t.trim()).filter(Boolean);
    let payload = { ...env, display_name: editDisplayName.value.trim(), tags };
    if (env.source === "ssh_tunnel") {
      if (!editSshHost.value.trim()) {
        editError.value = "请选择 ~/.ssh/config 中的 Host";
        return;
      }
      const port = parseLocalPort(editLocalPort.value);
      if (editLocalPort.value.trim() && port === null) {
        editError.value = "本地端口需为 1–65535 的整数";
        return;
      }
      const tunnelId = await envEnsureSshTunnelForHost(
        editSshHost.value.trim(),
        editRemoteKubeconfigPath.value.trim() || "~/.kube/config",
        port
      );
      payload = {
        ...payload,
        ssh_tunnel_id: tunnelId,
        ssh_idle_protection: editSshIdleProtection.value ? true : null,
      };
      await kubeRemoveClient(env.id);
    }
    await envUpdate(payload);
    await loadList();
    closeEditModal();
  } catch (e) {
    editError.value = extractErrorMessage(e);
  } finally {
    editLoading.value = false;
  }
}

function strategyEnabled(envId: string): boolean {
  return Boolean(getNodeTerminalStrategy(envId)?.enabled);
}

function openStrategyModal(env: Environment) {
  strategyEnv.value = env;
  strategyError.value = "";
  strategyCredentialExists.value = false;
  strategyPasswordInput.value = "";
  strategyShowPassword.value = false;
  strategyShowPasswordInput.value = false;
  strategyPasswordMsg.value = null;
  strategyForm.value = {
    ...(getNodeTerminalStrategy(env.id) ?? {
      envId: env.id,
      enabled: false,
      nodeAddressTemplate: "{node}",
      steps: [createNodeTerminalStep("ssh", "root")],
      hasSavedPassword: false,
    }),
    envId: env.id,
  };
  showStrategyModal.value = true;
  void credentialExists(nodeTerminalSwitchUserCredentialId(env.id))
    .then((exists) => {
      strategyCredentialExists.value = exists;
      strategyForm.value = {
        ...strategyForm.value,
        hasSavedPassword: exists,
      };
      setNodeTerminalStrategy(env.id, { hasSavedPassword: exists });
    })
    .catch(() => {});
}

function closeStrategyModal() {
  showStrategyModal.value = false;
  strategyEnv.value = null;
  strategyError.value = "";
  strategyPasswordInput.value = "";
  strategyShowPassword.value = false;
  strategyShowPasswordInput.value = false;
  strategyPasswordMsg.value = null;
}

async function saveStrategy() {
  if (!strategyEnv.value) return;
  strategyError.value = "";
  strategyLoading.value = true;
  try {
    setNodeTerminalStrategy(strategyEnv.value.id, strategyForm.value);
    closeStrategyModal();
  } catch (e) {
    strategyError.value = extractErrorMessage(e);
  } finally {
    strategyLoading.value = false;
  }
}

function updateStrategyField<K extends keyof NodeTerminalStrategy>(
  key: K,
  value: NodeTerminalStrategy[K]
) {
  if (!strategyEnv.value) return;
  strategyForm.value = {
    ...strategyForm.value,
    envId: strategyEnv.value.id,
    [key]: value,
  };
}

function updateStrategyStep(stepId: string, patch: Partial<NodeTerminalStepConfig>) {
  strategyForm.value = {
    ...strategyForm.value,
    steps: strategyForm.value.steps.map((step) =>
      step.id === stepId ? { ...step, ...patch } : step
    ),
  };
}

function updateStrategyStepType(stepId: string, type: NodeTerminalStepType) {
  const current = strategyForm.value.steps.find((step) => step.id === stepId);
  updateStrategyStep(stepId, {
    type,
    user: current?.user?.trim() || "root",
  });
}

function addStrategyStep(type: NodeTerminalStepType) {
  strategyForm.value = {
    ...strategyForm.value,
    steps: [...strategyForm.value.steps, createNodeTerminalStep(type)],
  };
}

function removeStrategyStep(stepId: string) {
  const next = strategyForm.value.steps.filter((step) => step.id !== stepId);
  strategyForm.value = {
    ...strategyForm.value,
    steps: next.length ? next : [createNodeTerminalStep("ssh", "root")],
  };
}

async function handleSaveStrategyPassword() {
  if (!strategyEnv.value || !strategyPasswordInput.value) return;
  strategyPasswordLoading.value = true;
  strategyPasswordMsg.value = null;
  try {
    await credentialSave(
      nodeTerminalSwitchUserCredentialId(strategyEnv.value.id),
      strategyPasswordInput.value
    );
    strategyCredentialExists.value = true;
    strategyShowPasswordInput.value = false;
    strategyPasswordInput.value = "";
    strategyForm.value = {
      ...strategyForm.value,
      hasSavedPassword: true,
    };
    setNodeTerminalStrategy(strategyEnv.value.id, { hasSavedPassword: true });
    strategyPasswordMsg.value = { type: "ok", text: "切换用户密码已保存到当前凭证存储后端。" };
  } catch (e) {
    const handled = await guardedAction.handleError(e, () => void handleSaveStrategyPassword(), strategyPasswordMsg, {
      ...STRATEGY_CREDENTIAL_GUARD,
      lockedText: "需要先解锁 Stronghold，解锁后会自动继续保存。",
    });
    if (handled) return;
  } finally {
    strategyPasswordLoading.value = false;
  }
}

async function handleDeleteStrategyPassword() {
  if (!strategyEnv.value) return;
  strategyPasswordLoading.value = true;
  strategyPasswordMsg.value = null;
  try {
    await credentialDelete(nodeTerminalSwitchUserCredentialId(strategyEnv.value.id));
    strategyCredentialExists.value = false;
    strategyShowPasswordInput.value = false;
    strategyPasswordInput.value = "";
    strategyForm.value = {
      ...strategyForm.value,
      hasSavedPassword: false,
    };
    setNodeTerminalStrategy(strategyEnv.value.id, { hasSavedPassword: false });
    strategyPasswordMsg.value = { type: "ok", text: "已清除切换用户密码。" };
  } catch (e) {
    const handled = await guardedAction.handleError(e, () => void handleDeleteStrategyPassword(), strategyPasswordMsg, {
      ...STRATEGY_CREDENTIAL_GUARD,
      lockedText: "需要先解锁 Stronghold，解锁后会自动继续清除。",
    });
    if (handled) return;
  } finally {
    strategyPasswordLoading.value = false;
  }
}

async function switchContext(env: Environment, contextName: string) {
  try {
    await envSetCurrentContext(env.id, contextName);
    await kubeRemoveClient(env.id);
    await loadList();
    if (editingEnv.value?.id === env.id) {
      editingEnv.value = environments.value.find((e) => e.id === env.id) ?? editingEnv.value;
    }
  } catch (e) {
    console.error(e);
  }
}

async function removeEnv(id: string) {
  try {
    await storeRemoveEnv(id);
    await loadList();
    if (editingEnv.value?.id === id) closeEditModal();
  } catch (e) {
    console.error(e);
  }
}

function useEnvAndGoMain(env: Environment) {
  openEnv(env.id);
  emit("use-env");
}

function openEnvTerminal(env: Environment) {
  pendingOpen.value = {
    kind: "host",
    envId: env.id,
    envName: env.display_name,
    hostLabel: `${env.display_name} 主机`,
  };
  requestSwitchToShell();
}

const currentContextLabel = (env: Environment) => {
  const name = effectiveContext(env);
  if (!name) return "—";
  const ctx = env.contexts.find((c) => c.context_name === name);
  return ctx?.cluster_name ?? name;
};

onMounted(() => {
  loadList();
});
</script>

<template>
  <div class="env-manage">
    <header class="header">
      <h1>环境管理</h1>
      <button class="btn-primary" @click="openNewModal">
        <span class="btn-icon">+</span> 新建环境
      </button>
    </header>

    <div v-if="!listLoading && allTags.length" class="filter-bar">
      <span class="filter-label">按标签筛选：</span>
      <div class="filter-tags">
        <button
          v-for="tag in allTags"
          :key="tag"
          type="button"
          class="filter-tag"
          :class="{ active: selectedFilterTags.has(tag) }"
          @click="toggleFilterTag(tag)"
        >
          {{ tag }}
        </button>
        <button
          v-if="selectedFilterTags.size"
          type="button"
          class="filter-tag clear"
          @click="clearFilter"
        >
          清除
        </button>
      </div>
    </div>

    <div class="env-manage-body">
      <div v-if="listLoading" class="state-message loading">
        <span class="spinner" aria-hidden="true"></span>
        <span>加载中…</span>
      </div>
      <div v-else-if="filteredEnvironments.length" class="card-grid">
        <article
          v-for="env in filteredEnvironments"
          :key="env.id"
          class="env-card"
          @click="openEditModal(env)"
        >
          <div class="card-header">
            <span class="badge" :class="env.source">{{ env.source === "local_kubeconfig" ? "本地" : "SSH" }}</span>
            <h3 class="card-title">{{ env.display_name }}</h3>
          </div>
          <div class="card-tags" :class="{ empty: !(env.tags ?? []).length }">
            <span v-for="t in env.tags" :key="t" class="tag-chip">{{ t }}</span>
          </div>
          <div class="card-meta-group">
            <p v-if="env.source === 'local_kubeconfig'" class="card-meta current-ctx">{{ currentContextLabel(env) }}</p>
            <p v-if="env.source === 'local_kubeconfig'" class="card-meta count">{{ env.contexts.length }} 个 context</p>
            <template v-if="env.source === 'ssh_tunnel'">
              <p class="card-meta">Host: {{ getTunnelForEnv(env)?.ssh_host ?? '—' }}</p>
              <p class="card-meta">远程 kubeconfig: {{ getTunnelForEnv(env)?.remote_kubeconfig_path ?? '—' }}</p>
            </template>
            <p class="card-meta strategy" :class="{ enabled: strategyEnabled(env.id) }">
              节点终端策略：{{ strategyEnabled(env.id) ? "已启用" : "未配置" }}
            </p>
          </div>
          <div class="card-actions" @click.stop>
            <button type="button" class="btn-use" @click="useEnvAndGoMain(env)">使用</button>
            <button type="button" class="btn-terminal" @click="openEnvTerminal(env)">终端</button>
            <button type="button" class="btn-strategy" @click="openStrategyModal(env)">终端策略</button>
          </div>
        </article>
      </div>
      <div v-else class="state-message empty">
        <p class="empty-title">{{ selectedFilterTags.size ? '无匹配环境' : '暂无环境' }}</p>
        <p class="empty-desc">{{ selectedFilterTags.size ? '尝试清除标签筛选或新建环境。' : '点击「新建环境」添加本地 kubeconfig 或 SSH 隧道连接。' }}</p>
        <button v-if="selectedFilterTags.size" type="button" class="btn-secondary" @click="clearFilter">清除筛选</button>
        <button type="button" class="btn-primary" @click="openNewModal">新建环境</button>
      </div>
    </div>

    <!-- 新建环境弹窗 -->
    <Teleport to="body">
      <div v-if="showNewModal" class="modal-overlay" @click.self="closeNewModal">
        <div class="modal" role="dialog" aria-labelledby="new-env-title">
          <h2 id="new-env-title" class="modal-title">新建环境</h2>
          <div class="tabs">
            <button type="button" :class="{ active: newType === 'local' }" @click="newType = 'local'">本地 kubeconfig</button>
            <button type="button" :class="{ active: newType === 'ssh' }" @click="newType = 'ssh'">SSH 隧道</button>
          </div>
          <div v-if="newType === 'local'" class="form">
            <label>连接名称</label>
            <input v-model="newDisplayName" type="text" placeholder="例如：本地 Minikube" />
            <label>kubeconfig 路径</label>
            <div class="input-row">
              <input v-model="newKubeconfigPath" type="text" placeholder="~/.kube/config" />
              <button type="button" class="btn-discover" :disabled="discoverLoading" @click="discoverContexts">
                {{ discoverLoading ? "发现中…" : "发现 Context" }}
              </button>
            </div>
            <p v-if="discoverError" class="form-error">{{ discoverError }}</p>
            <label>标签</label>
            <div class="tag-input-wrap">
              <span v-for="t in newTags" :key="t" class="tag-chip removable">
                {{ t }}
                <button type="button" class="tag-remove" aria-label="移除" @click="removeNewTag(t)">×</button>
              </span>
              <input
                v-model="newTagDraft"
                type="text"
                class="tag-input-inner"
                placeholder="输入后按 Enter 或逗号添加"
                @keydown="onNewTagKeydown"
              />
            </div>
            <template v-if="discoveredContexts.length">
              <label>选择要加入的 Context</label>
              <ul class="context-list">
                <li v-for="c in discoveredContexts" :key="c.context_name">
                  <label class="checkbox-label">
                    <input
                      type="checkbox"
                      :checked="selectedContextKeys.has(c.context_name)"
                      @change="toggleContext(c.context_name)"
                    />
                    <span>{{ c.context_name }}</span>
                    <span class="cluster">{{ c.cluster_name }}</span>
                  </label>
                </li>
              </ul>
            </template>
          </div>
          <div v-else class="form">
            <label>连接名称</label>
            <input v-model="newDisplayName" type="text" placeholder="例如：生产跳板机" />
            <label>~/.ssh/config Host</label>
            <select v-model="newSshHost">
              <option value="">请选择</option>
              <option v-for="h in sshConfigHosts" :key="h" :value="h">{{ h }}</option>
            </select>
            <p v-if="newType === 'ssh' && !sshConfigHosts.length" class="form-hint form-warn">未检测到 ~/.ssh/config 中的 Host，请确认本机存在 ~/.ssh/config 且包含 Host 配置。</p>
            <label>远程 kubeconfig 路径</label>
            <input v-model="newRemoteKubeconfigPath" type="text" placeholder="~/.kube/config" />
            <label>本地端口</label>
            <input v-model="newLocalPort" type="text" placeholder="留空=自动分配" inputmode="numeric" />
            <label class="checkbox-row">
              <input v-model="newSshIdleProtection" type="checkbox" />
              <span>启用空闲保护</span>
            </label>
            <label>SSH 认证密码（可选）</label>
            <div class="password-row">
              <input
                :type="newShowPassword ? 'text' : 'password'"
                v-model="newPasswordInput"
                class="password-input"
                placeholder="创建后自动保存到安全存储"
                autocomplete="new-password"
              />
              <button
                type="button"
                class="toggle-vis"
                :title="newShowPassword ? '隐藏' : '显示'"
                @click="newShowPassword = !newShowPassword"
              >
                {{ newShowPassword ? "🙈" : "👁" }}
              </button>
            </div>
            <label>标签</label>
            <div class="tag-input-wrap">
              <span v-for="t in newTags" :key="t" class="tag-chip removable">
                {{ t }}
                <button type="button" class="tag-remove" aria-label="移除" @click="removeNewTag(t)">×</button>
              </span>
              <input
                v-model="newTagDraft"
                type="text"
                class="tag-input-inner"
                placeholder="输入后按 Enter 或逗号添加"
                @keydown="onNewTagKeydown"
              />
            </div>
            <p class="form-hint">选择本机 ~/.ssh/config 中的 Host，建立隧道后使用远程主机上的 kubeconfig；留空则自动分配端口，或填写 1–65535 指定固定端口。若填写 SSH 密码，创建成功后会一并保存到当前凭证存储后端。</p>
          </div>
          <p v-if="createError" class="form-error">{{ createError }}</p>
          <div class="modal-actions">
            <button type="button" class="btn-secondary" @click="closeNewModal">取消</button>
            <button type="button" class="btn-primary" :disabled="createLoading" @click="doCreate">{{ createLoading ? "创建中…" : "创建" }}</button>
          </div>
        </div>
      </div>
    </Teleport>

    <!-- 编辑环境弹窗 -->
    <Teleport to="body">
      <div v-if="showEditModal && editingEnv" class="modal-overlay" @click.self="closeEditModal">
        <div class="modal" role="dialog" aria-labelledby="edit-env-title">
          <h2 id="edit-env-title" class="modal-title">编辑环境</h2>
          <div class="form">
            <label>连接名称</label>
            <input v-model="editDisplayName" type="text" />
            <label>标签</label>
            <div class="tag-input-wrap">
              <span v-for="t in editTags" :key="t" class="tag-chip removable">
                {{ t }}
                <button type="button" class="tag-remove" aria-label="移除" @click="removeEditTag(t)">×</button>
              </span>
              <input
                v-model="editTagDraft"
                type="text"
                class="tag-input-inner"
                placeholder="输入后按 Enter 或逗号添加"
                @keydown="onEditTagKeydown"
              />
            </div>
            <template v-if="editingEnv.source === 'ssh_tunnel'">
              <label>~/.ssh/config Host</label>
              <select v-model="editSshHost">
                <option value="">请选择</option>
                <option v-for="h in sshConfigHosts" :key="h" :value="h">{{ h }}</option>
              </select>
              <label>远程 kubeconfig 路径</label>
              <input v-model="editRemoteKubeconfigPath" type="text" placeholder="~/.kube/config" />
              <label>本地端口</label>
              <input v-model="editLocalPort" type="text" placeholder="留空=自动分配" inputmode="numeric" />
              <label class="checkbox-row">
                <input v-model="editSshIdleProtection" type="checkbox" />
                <span>启用空闲保护</span>
              </label>

              <!-- SSH 认证密码持久化配置 -->
              <div class="credential-section">
                <div class="credential-header">
                  <span class="credential-title">SSH 认证密码</span>
                  <span v-if="editCredentialExists" class="credential-badge saved">已配置</span>
                  <span v-else class="credential-badge none">未配置</span>
                </div>
                <p class="form-hint" style="margin: 0 0 0.6rem;">保存后每次连接自动读取，无需手动输入。密码加密存储在安全存储中。</p>

                <!-- 已有密码：显示操作按钮 -->
                <template v-if="editCredentialExists && !editShowPasswordInput">
                  <div class="credential-actions">
                    <button type="button" class="btn-secondary btn-sm" :disabled="editPasswordLoading" @click="editShowPasswordInput = true; editPasswordInput = ''">修改密码</button>
                    <button type="button" class="btn-danger btn-sm" :disabled="editPasswordLoading" @click="handleDeletePassword">清除密码</button>
                  </div>
                </template>

                <!-- 输入密码：新建或修改 -->
                <template v-if="!editCredentialExists || editShowPasswordInput">
                  <div class="password-row">
                    <input
                      :type="editShowPassword ? 'text' : 'password'"
                      v-model="editPasswordInput"
                      class="password-input"
                      placeholder="输入 SSH 密码"
                      autocomplete="new-password"
                      @keyup.enter="handleSavePassword"
                    />
                    <button type="button" class="toggle-vis" :title="editShowPassword ? '隐藏' : '显示'" @click="editShowPassword = !editShowPassword">
                      {{ editShowPassword ? "🙈" : "👁" }}
                    </button>
                    <button type="button" class="btn-primary btn-sm" :disabled="editPasswordLoading || !editPasswordInput" @click="handleSavePassword">
                      {{ editPasswordLoading ? "保存中…" : "保存密码" }}
                    </button>
                    <button v-if="editCredentialExists && editShowPasswordInput" type="button" class="btn-secondary btn-sm" @click="editShowPasswordInput = false">取消</button>
                  </div>
                </template>

                <p v-if="editPasswordMsg" :class="['credential-msg', editPasswordMsg.type]">
                  {{ editPasswordMsg.text }}
                </p>
              </div>
            </template>
            <template v-else>
              <label>当前 Context</label>
              <select
                :value="effectiveContext(editingEnv)"
                @change="(e) => switchContext(editingEnv!, (e.target as HTMLSelectElement).value)"
              >
                <option v-for="c in editingEnv.contexts" :key="c.context_name" :value="c.context_name">
                  {{ c.context_name }} ({{ c.cluster_name ?? "-" }})
                </option>
              </select>
            </template>

          </div>
          <p v-if="editError" class="form-error">{{ editError }}</p>
          <div class="modal-actions">
            <button type="button" class="btn-danger" @click="removeEnv(editingEnv.id)">删除环境</button>
            <div class="modal-actions-right">
              <button type="button" class="btn-secondary" @click="closeEditModal">取消</button>
              <button type="button" class="btn-primary" :disabled="editLoading" @click="doUpdate">{{ editLoading ? "保存中…" : "保存" }}</button>
            </div>
          </div>
        </div>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-if="showStrategyModal && strategyEnv" class="modal-overlay" @click.self="closeStrategyModal">
        <div class="modal" role="dialog" aria-labelledby="strategy-modal-title">
          <h2 id="strategy-modal-title" class="modal-title">终端切换策略</h2>
          <p class="form-hint" style="margin: -0.5rem 0 1rem;">
            环境：{{ strategyEnv.display_name }}。右键 Node 或 Pod 打开节点终端时，会先进入该环境主机，再执行这里配置的切换命令。
          </p>
          <div class="form">
            <label class="checkbox-row">
              <input
                :checked="strategyForm.enabled"
                type="checkbox"
                @change="updateStrategyField('enabled', ($event.target as HTMLInputElement).checked)"
              />
              <span>启用节点终端切换策略</span>
            </label>
            <p class="form-hint" style="margin: 0.4rem 0 0.2rem;">{{ strategyModeHint }}</p>
            <div class="node-strategy-grid">
              <label class="node-strategy-wide">
                <span>节点地址模板</span>
                <input
                  :value="strategyForm.nodeAddressTemplate"
                  type="text"
                  placeholder="{node}"
                  @input="updateStrategyField('nodeAddressTemplate', ($event.target as HTMLInputElement).value)"
                />
              </label>
              <div class="node-strategy-wide strategy-steps">
                <div class="strategy-steps-header">
                  <span>步骤编排</span>
                  <div class="strategy-step-actions">
                    <button
                      type="button"
                      class="strategy-step-add"
                      title="添加步骤"
                      @click="addStrategyStep('ssh')"
                    >
                      +
                    </button>
                  </div>
                </div>
                <div class="strategy-step-list">
                  <div
                    v-for="(step, index) in strategyForm.steps"
                    :key="step.id"
                    class="strategy-step-card"
                  >
                    <div class="strategy-step-row">
                      <span class="strategy-step-index">{{ index + 1 }}</span>
                      <select
                        :value="step.type"
                        @change="updateStrategyStepType(step.id, ($event.target as HTMLSelectElement).value as NodeTerminalStepType)"
                      >
                        <option value="switch_user">switch_user</option>
                        <option value="ssh">ssh</option>
                      </select>
                      <input
                        :value="step.user"
                        type="text"
                        :placeholder="step.type === 'switch_user' ? '目标用户，例如 root / deploy' : 'SSH 用户，例如 root'"
                        @input="updateStrategyStep(step.id, { user: ($event.target as HTMLInputElement).value })"
                      />
                      <button
                        type="button"
                        class="strategy-step-remove"
                        title="删除步骤"
                        :disabled="strategyForm.steps.length <= 1"
                        @click="removeStrategyStep(step.id)"
                      >
                        ×
                      </button>
                    </div>
                    <div class="strategy-step-hint">{{ strategyStepHint(step.type) }}</div>
                  </div>
                </div>
              </div>
              <label v-if="strategyNeedsSwitchUserPassword(strategyForm)" class="node-strategy-wide">
                <span>切换用户密码</span>
                <div class="strategy-password-panel">
                  <div class="credential-header">
                    <span class="credential-title">切换用户密码</span>
                    <span :class="['credential-badge', strategyCredentialExists ? 'saved' : 'none']">
                      {{ strategyCredentialExists ? "已保存" : "未配置" }}
                    </span>
                  </div>
                  <div class="credential-actions">
                    <button
                      type="button"
                      class="btn-secondary btn-sm"
                      :disabled="!strategyNeedsSwitchUserPassword(strategyForm)"
                      @click="strategyShowPasswordInput = !strategyShowPasswordInput; strategyPasswordMsg = null"
                    >
                      {{ strategyCredentialExists ? "修改密码" : "设置密码" }}
                    </button>
                    <button
                      v-if="strategyCredentialExists"
                      type="button"
                      class="btn-secondary btn-sm"
                      :disabled="strategyPasswordLoading"
                      @click="handleDeleteStrategyPassword"
                    >
                      清除密码
                    </button>
                  </div>
                  <div v-if="strategyShowPasswordInput" class="password-row">
                    <input
                      :value="strategyPasswordInput"
                      :type="strategyShowPassword ? 'text' : 'password'"
                      :disabled="strategyPasswordLoading"
                      :placeholder="strategyCredentialExists ? '输入新密码' : '输入切换用户密码'"
                      class="password-input"
                      autocomplete="new-password"
                      @input="strategyPasswordInput = ($event.target as HTMLInputElement).value"
                      @keyup.enter="handleSaveStrategyPassword"
                    />
                    <button
                      type="button"
                      class="toggle-vis"
                      :title="strategyShowPassword ? '隐藏' : '显示'"
                      @click="strategyShowPassword = !strategyShowPassword"
                    >
                      {{ strategyShowPassword ? "🙈" : "👁" }}
                    </button>
                    <button
                      type="button"
                      class="btn-primary btn-sm"
                      :disabled="strategyPasswordLoading || !strategyPasswordInput || !strategyNeedsSwitchUserPassword(strategyForm)"
                      @click="handleSaveStrategyPassword"
                    >
                      {{ strategyPasswordLoading ? "保存中…" : "保存密码" }}
                    </button>
                    <button
                      v-if="strategyCredentialExists"
                      type="button"
                      class="btn-secondary btn-sm"
                      :disabled="strategyPasswordLoading"
                      @click="strategyShowPasswordInput = false"
                    >
                      取消
                    </button>
                  </div>
                  <p v-if="strategyPasswordMsg" :class="['credential-msg', strategyPasswordMsg.type]">
                    {{ strategyPasswordMsg.text }}
                  </p>
                  <div class="strategy-password-help">
                    仅当步骤中包含 `switch_user` 时需要配置。密码会保存在当前凭证存储后端，并由后端在切换用户提示出现时安全写入。
                  </div>
                </div>
              </label>
            </div>
            <div class="strategy-tip">
              节点地址模板当前支持 `{node}` 占位符。常见链路可以配置成：
              `ssh`
              或 `switch_user -> ssh`。
            </div>
            <div v-if="strategyPreview" class="strategy-preview">
              <div>预览地址：{{ strategyPreview.host }}</div>
              <pre class="strategy-preview-code">{{ strategyPreview.command }}</pre>
            </div>
            <div v-else class="strategy-preview strategy-preview-empty">
              当前策略未启用，或模板无法生成有效命令。
            </div>
          </div>
          <p v-if="strategyError" class="form-error">{{ strategyError }}</p>
          <div class="modal-actions">
            <div></div>
            <div class="modal-actions-right">
              <button type="button" class="btn-secondary" @click="closeStrategyModal">取消</button>
              <button type="button" class="btn-primary" :disabled="strategyLoading" @click="saveStrategy">
                {{ strategyLoading ? "保存中…" : "保存策略" }}
              </button>
            </div>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.env-manage {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  padding: 1.5rem 2rem;
  background: linear-gradient(180deg, #f8fafc 0%, #f1f5f9 100%);
}
.env-manage-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
}
.header {
  display: flex;
  align-items: center;
  gap: 1rem;
  margin-bottom: 1.75rem;
  flex-wrap: wrap;
  flex-shrink: 0;
}
.header h1 {
  font-size: 1.375rem;
  font-weight: 600;
  margin: 0;
  flex: 1;
  letter-spacing: -0.02em;
}
.filter-bar {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-bottom: 1rem;
  flex-shrink: 0;
  flex-wrap: wrap;
}
.filter-label {
  font-size: 0.8125rem;
  color: #64748b;
  flex-shrink: 0;
}
.filter-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
}
.filter-tag {
  padding: 0.25rem 0.6rem;
  font-size: 0.8125rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  background: #fff;
  color: #475569;
  cursor: pointer;
  transition: border-color 0.15s, background 0.15s;
}
.filter-tag:hover {
  border-color: #cbd5e1;
  background: #f8fafc;
}
.filter-tag.active {
  border-color: #2563eb;
  background: #eff6ff;
  color: #2563eb;
}
.filter-tag.clear {
  color: #64748b;
  border-style: dashed;
}
.card-tags {
  display: flex;
  flex-wrap: wrap;
  align-content: flex-start;
  gap: 0.35rem;
  margin-bottom: 0.5rem;
  min-height: 1.65rem;
}
.card-tags.empty {
  visibility: hidden;
}
.tag-chip {
  font-size: 0.7rem;
  padding: 0.15rem 0.5rem;
  border-radius: 4px;
  background: #dcfce7;
  color: #166534;
}
.tag-input-wrap .tag-chip.removable {
  background: #dcfce7;
  color: #166534;
}
.tag-input-wrap {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.4rem;
  min-height: 2.25rem;
  padding: 0.35rem 0.6rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  background: #fff;
  transition: border-color 0.15s;
}
.tag-input-wrap:focus-within {
  border-color: #2563eb;
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.12);
}
.tag-chip.removable {
  display: inline-flex;
  align-items: center;
  gap: 0.2rem;
  padding-right: 0.25rem;
}
.tag-remove {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0 0.2em;
  margin: 0;
  border: none;
  background: none;
  color: #64748b;
  font-size: 1em;
  line-height: 1;
  cursor: pointer;
  border-radius: 2px;
}
.tag-input-wrap .tag-remove:hover {
  color: #b91c1c;
  background: #fef2f2;
}
.tag-input-inner {
  flex: 1;
  min-width: 8rem;
  padding: 0.2rem 0;
  border: none;
  outline: none;
  font-size: 0.875rem;
  background: transparent;
}
.tag-input-inner::placeholder {
  color: #94a3b8;
}
.credential-section {
  grid-column: 1 / -1;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  padding: 0.875rem 1rem;
  margin-top: 0.25rem;
}
.credential-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.4rem;
}
.credential-title {
  font-size: 0.8125rem;
  font-weight: 600;
  color: #374151;
}
.credential-badge {
  font-size: 0.72rem;
  font-weight: 500;
  padding: 0.15rem 0.55rem;
  border-radius: 99px;
}
.credential-badge.saved {
  background: #dcfce7;
  color: #166534;
}
.credential-badge.none {
  background: #f1f5f9;
  color: #64748b;
}
.credential-actions {
  display: flex;
  gap: 0.5rem;
}
.password-row {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}
.password-input {
  flex: 1;
  padding: 0.4rem 0.6rem;
  border: 1px solid #d1d5db;
  border-radius: 6px;
  font-size: 0.875rem;
  outline: none;
  min-width: 0;
}
.password-input:focus {
  border-color: #2563eb;
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.12);
}
.toggle-vis {
  background: none;
  border: none;
  cursor: pointer;
  font-size: 1rem;
  opacity: 0.6;
  padding: 0.2rem;
  flex-shrink: 0;
}
.toggle-vis:hover { opacity: 1; }
.btn-sm {
  padding: 0.35rem 0.75rem;
  font-size: 0.8125rem;
  border-radius: 6px;
  flex-shrink: 0;
}
.node-strategy-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.75rem 1rem;
  margin-top: 0.75rem;
}
.node-strategy-grid label {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  font-size: 0.8125rem;
  color: #475569;
}
.node-strategy-grid input,
.node-strategy-grid textarea {
  width: 100%;
  padding: 0.55rem 0.7rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  background: #fff;
  font-size: 0.875rem;
  outline: none;
  resize: vertical;
}
.node-strategy-grid input:focus,
.node-strategy-grid textarea:focus {
  border-color: #2563eb;
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.12);
}
.node-strategy-wide {
  grid-column: 1 / -1;
}
.strategy-steps {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}
.strategy-steps-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  font-size: 0.8125rem;
  color: #475569;
}
.strategy-step-actions {
  display: flex;
  gap: 0.45rem;
  flex-wrap: wrap;
}
.strategy-step-add {
  width: 2rem;
  height: 2rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid #22c55e;
  border-radius: 999px;
  background: #f0fdf4;
  color: #16a34a;
  font-size: 1.1rem;
  line-height: 1;
  cursor: pointer;
}
.strategy-step-add:hover {
  background: #dcfce7;
  border-color: #16a34a;
  color: #15803d;
}
.strategy-step-list {
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
}
.strategy-step-card {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  padding: 0.55rem 0.6rem;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  background: #f8fafc;
}
.strategy-step-row {
  display: grid;
  grid-template-columns: auto minmax(120px, 160px) minmax(0, 1fr) auto;
  gap: 0.5rem;
  align-items: center;
}
.strategy-step-hint {
  margin-left: 1.9rem;
  font-size: 0.78rem;
  color: #64748b;
}
.strategy-step-index {
  width: 1.4rem;
  height: 1.4rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  background: #e2e8f0;
  color: #475569;
  font-size: 0.75rem;
  font-weight: 700;
}
.strategy-step-row select {
  width: 100%;
  padding: 0.55rem 0.7rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  background: #fff;
  font-size: 0.875rem;
  outline: none;
}
.strategy-step-row select:focus {
  border-color: #2563eb;
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.12);
}
.strategy-step-remove {
  width: 2rem;
  height: 2rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid #d1d5db;
  border-radius: 999px;
  background: #fff;
  color: #64748b;
  font-size: 1rem;
  line-height: 1;
  cursor: pointer;
}
.strategy-step-remove:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.strategy-step-remove:not(:disabled):hover {
  border-color: #cbd5e1;
  color: #334155;
}
.strategy-preview-code {
  margin: 0.45rem 0 0;
  padding: 0.7rem 0.8rem;
  border-radius: 10px;
  background: #0f172a;
  color: #e2e8f0;
  font-size: 0.82rem;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
}
.strategy-tip {
  margin-top: 0.65rem;
  font-size: 0.78rem;
  color: #64748b;
}
.strategy-preview {
  margin-top: 0.65rem;
  padding: 0.65rem 0.8rem;
  border-radius: 8px;
  background: #fff;
  border: 1px solid #e2e8f0;
  font-size: 0.8125rem;
  color: #334155;
}
.strategy-preview-empty {
  color: #64748b;
}
.strategy-password-disabled {
  padding: 0.65rem 0.75rem;
  border-radius: 8px;
  border: 1px dashed #cbd5e1;
  background: #f8fafc;
  font-size: 0.8125rem;
  line-height: 1.5;
  color: #64748b;
}
.strategy-password-panel {
  padding: 0.75rem 0.85rem;
  border-radius: 8px;
  border: 1px solid #e2e8f0;
  background: #fff;
}
.strategy-password-help {
  margin-top: 0.55rem;
  font-size: 0.78rem;
  line-height: 1.5;
  color: #64748b;
}
.credential-msg {
  margin: 0.5rem 0 0;
  font-size: 0.8125rem;
  padding: 0.35rem 0.6rem;
  border-radius: 6px;
}
.credential-msg.ok {
  background: #f0fdf4;
  color: #166534;
}
.credential-msg.err {
  background: #fef2f2;
  color: #dc2626;
}
.card-actions {
  margin-top: auto;
  padding-top: 1rem;
  border-top: 1px solid #f1f5f9;
  display: flex;
  gap: 0.6rem;
}
.btn-use {
  padding: 0.4rem 0.9rem;
  background: #2563eb;
  color: #fff;
  border: none;
  border-radius: 8px;
  font-size: 0.8125rem;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.15s;
}
.btn-use:hover {
  background: #1d4ed8;
}
.btn-terminal {
  padding: 0.4rem 0.9rem;
  background: #eef4ff;
  color: #1d4ed8;
  border: 1px solid #c7d7fe;
  border-radius: 8px;
  font-size: 0.8125rem;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s, color 0.15s;
}
.btn-terminal:hover {
  background: #dbeafe;
  border-color: #93c5fd;
  color: #1e40af;
}
.btn-strategy {
  padding: 0.4rem 0.9rem;
  background: #f8fafc;
  color: #334155;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  font-size: 0.8125rem;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s, color 0.15s;
}
.btn-strategy:hover {
  background: #eef2ff;
  border-color: #c7d2fe;
  color: #4338ca;
}
.btn-primary {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 1rem;
  background: #2563eb;
  color: #fff;
  border: none;
  border-radius: 8px;
  font-size: 0.875rem;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.15s, transform 0.05s;
}
.btn-primary:hover:not(:disabled) {
  background: #1d4ed8;
}
.btn-primary:active:not(:disabled) {
  transform: scale(0.98);
}
.btn-primary:disabled {
  opacity: 0.7;
  cursor: not-allowed;
}
.btn-icon {
  font-size: 1.1em;
  line-height: 1;
}
.state-message {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  padding: 3rem 1.5rem;
  color: #64748b;
  font-size: 0.875rem;
}
.state-message.loading {
  flex-direction: row;
}
.spinner {
  width: 20px;
  height: 20px;
  border: 2px solid #e2e8f0;
  border-top-color: #2563eb;
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}
@keyframes spin {
  to { transform: rotate(360deg); }
}
.empty-title {
  margin: 0;
  font-size: 1rem;
  font-weight: 500;
  color: #334155;
}
.empty-desc {
  margin: 0;
  text-align: center;
  max-width: 280px;
  line-height: 1.5;
}
.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 1.25rem;
}
.env-card {
  display: flex;
  flex-direction: column;
  border: 1px solid #e2e8f0;
  border-radius: 12px;
  padding: 1.25rem;
  background: #fff;
  cursor: pointer;
  transition: box-shadow 0.2s, border-color 0.2s, transform 0.15s;
}
.env-card:hover {
  border-color: #c7d2fe;
  box-shadow: 0 4px 12px rgba(37, 99, 235, 0.08);
  transform: translateY(-1px);
}
.card-header {
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
  margin-bottom: 0.75rem;
  min-height: 2.8rem;
}
.badge {
  font-size: 0.7rem;
  font-weight: 500;
  padding: 0.2rem 0.5rem;
  border-radius: 6px;
  text-transform: uppercase;
  letter-spacing: 0.03em;
}
.badge.local_kubeconfig {
  background: #dbeafe;
  color: #1d4ed8;
}
.badge.ssh_tunnel {
  background: #ffedd5;
  color: #c2410c;
}
.card-title {
  margin: 0;
  font-size: 1rem;
  font-weight: 600;
  color: #1e293b;
  letter-spacing: -0.01em;
  line-height: 1.4;
}
.card-meta-group {
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
  gap: 0.25rem;
  min-height: 3.35rem;
}
.card-meta {
  margin: 0;
  font-size: 0.8125rem;
  color: #64748b;
  line-height: 1.45;
}
.card-meta.count {
  font-size: 0.75rem;
  color: #94a3b8;
}
.card-meta.strategy.enabled {
  color: #166534;
}

/* 弹窗 */
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(15, 23, 42, 0.5);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  padding: 1rem;
}
.modal {
  background: #fff;
  border-radius: 16px;
  padding: 1.75rem;
  max-width: 440px;
  width: 100%;
  max-height: 90vh;
  overflow: auto;
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.25);
}
.modal-title {
  margin: 0 0 1.25rem 0;
  font-size: 1.25rem;
  font-weight: 600;
  color: #1e293b;
}
.tabs {
  display: flex;
  gap: 0.25rem;
  margin-bottom: 1.25rem;
  padding: 4px;
  background: #f1f5f9;
  border-radius: 10px;
}
.tabs button {
  flex: 1;
  padding: 0.5rem 0.75rem;
  border: none;
  background: transparent;
  border-radius: 8px;
  font-size: 0.8125rem;
  font-weight: 500;
  color: #64748b;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}
.tabs button.active {
  background: #fff;
  color: #1e293b;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}
.form label {
  display: block;
  font-size: 0.8125rem;
  font-weight: 500;
  color: #475569;
  margin-top: 1rem;
  margin-bottom: 0.35rem;
}
.form label:first-child {
  margin-top: 0;
}
.form label.checkbox-row {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  margin-top: 0.75rem;
  margin-bottom: 0;
  font-weight: 400;
  color: #334155;
  cursor: pointer;
}
.form label.checkbox-row input {
  width: auto;
  margin: 0;
}
.form input,
.form select {
  width: 100%;
  padding: 0.55rem 0.75rem;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  font-size: 0.875rem;
  transition: border-color 0.15s;
}
.form input:focus,
.form select:focus {
  outline: none;
  border-color: #2563eb;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.1);
}
.input-row {
  display: flex;
  gap: 0.5rem;
}
.input-row input {
  flex: 1;
}
.btn-discover {
  padding: 0.55rem 0.75rem;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  background: #f8fafc;
  font-size: 0.8125rem;
  font-weight: 500;
  color: #475569;
  cursor: pointer;
  white-space: nowrap;
  transition: background 0.15s, border-color 0.15s;
}
.btn-discover:hover:not(:disabled) {
  background: #f1f5f9;
  border-color: #cbd5e1;
}
.btn-discover:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.form-error {
  margin: 0.5rem 0 0 0;
  font-size: 0.8125rem;
  color: #dc2626;
}
.form-hint {
  margin: 0.5rem 0 0 0;
  font-size: 0.75rem;
  color: #94a3b8;
  line-height: 1.4;
}
.form-hint.form-warn {
  color: #d97706;
}
.context-list {
  list-style: none;
  padding: 0;
  margin: 0.5rem 0 0 0;
  max-height: 200px;
  overflow: auto;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  padding: 0.35rem;
}
.context-list li {
  padding: 0.2rem 0;
}
.checkbox-label {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.4rem 0.5rem;
  border-radius: 6px;
  cursor: pointer;
  font-size: 0.8125rem;
  transition: background 0.1s;
}
.checkbox-label:hover {
  background: #f8fafc;
}
.checkbox-label input {
  width: auto;
  margin: 0;
}
.checkbox-label .cluster {
  margin-left: auto;
  font-size: 0.75rem;
  color: #94a3b8;
}
.modal-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 0.75rem;
  margin-top: 1.5rem;
  padding-top: 1.25rem;
  border-top: 1px solid #f1f5f9;
}
.modal-actions-right {
  display: flex;
  gap: 0.5rem;
}
.btn-secondary {
  padding: 0.5rem 1rem;
  border: 1px solid #e2e8f0;
  background: #fff;
  border-radius: 8px;
  font-size: 0.875rem;
  font-weight: 500;
  color: #475569;
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s;
}
.btn-secondary:hover {
  background: #f8fafc;
  border-color: #cbd5e1;
}
.btn-danger {
  padding: 0.5rem 1rem;
  border: 1px solid #fecaca;
  background: #fff;
  border-radius: 8px;
  font-size: 0.875rem;
  font-weight: 500;
  color: #dc2626;
  cursor: pointer;
  transition: background 0.15s;
}
.btn-danger:hover {
  background: #fef2f2;
}
</style>
