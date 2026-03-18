<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import type { Environment, KubeContextInfo, SshTunnel } from "../api/env";
import { useEnvStore } from "../stores/env";
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
const { openEnv, removeEnv: storeRemoveEnv } = useEnvStore();
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
const createLoading = ref(false);
const createError = ref("");
const newTags = ref<string[]>([]);
const newTagDraft = ref("");

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
const editTagDraft = ref("");

function addNewTagFromDraft() {
  const t = newTagDraft.value.trim();
  if (t && !newTags.value.includes(t)) {
    newTags.value = [...newTags.value, t];
    newTagDraft.value = "";
  } else if (t) {
    newTagDraft.value = "";
  }
}

function removeNewTag(tag: string) {
  newTags.value = newTags.value.filter((t) => t !== tag);
}

function addEditTagFromDraft() {
  const t = editTagDraft.value.trim();
  if (t && !editTags.value.includes(t)) {
    editTags.value = [...editTags.value, t];
    editTagDraft.value = "";
  } else if (t) {
    editTagDraft.value = "";
  }
}

function removeEditTag(tag: string) {
  editTags.value = editTags.value.filter((t) => t !== tag);
}

function onNewTagKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" || e.key === "," || e.key === "，") {
    e.preventDefault();
    addNewTagFromDraft();
  }
}

function onEditTagKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" || e.key === "," || e.key === "，") {
    e.preventDefault();
    addEditTagFromDraft();
  }
}

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
}

async function discoverContexts() {
  discoverError.value = "";
  discoverLoading.value = true;
  try {
    const list = await envListContextsFromKubeconfig(newKubeconfigPath.value);
    discoveredContexts.value = list;
    selectedContextKeys.value = new Set(list.map((c) => c.context_name));
  } catch (e: unknown) {
    discoverError.value = e instanceof Error ? e.message : String(e);
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
      await envCreateSshWithHost(
        newDisplayName.value.trim(),
        newSshHost.value.trim(),
        newRemoteKubeconfigPath.value.trim() || "~/.kube/config",
        port,
        [],
        newTags.value,
        newSshIdleProtection.value ? true : null
      );
    }
    await loadList();
    closeNewModal();
  } catch (e: unknown) {
    createError.value = e instanceof Error ? e.message : String(e);
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
      const [tunnels, hosts] = await Promise.all([
        envListSshTunnels(),
        envListSshConfigHosts(),
        credentialExists(env.ssh_tunnel_id).then((v) => { editCredentialExists.value = v; }),
      ]);
      sshConfigHosts.value = hosts;
      const tunnel = tunnels.find((t) => t.id === env.ssh_tunnel_id);
      if (tunnel) {
        editSshHost.value = tunnel.ssh_host;
        editRemoteKubeconfigPath.value = tunnel.remote_kubeconfig_path || "~/.kube/config";
        editLocalPort.value = tunnel.local_port != null ? String(tunnel.local_port) : "";
      } else if (hosts.length) {
        editSshHost.value = hosts[0];
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
    editPasswordMsg.value = { type: "err", text: String(e) };
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
    editPasswordMsg.value = { type: "err", text: String(e) };
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
  } catch (e: unknown) {
    editError.value = e instanceof Error ? e.message : String(e);
  } finally {
    editLoading.value = false;
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
  } catch (e: unknown) {
    console.error(e);
  }
}

async function removeEnv(id: string) {
  try {
    await storeRemoveEnv(id);
    await loadList();
    if (editingEnv.value?.id === id) closeEditModal();
  } catch (e: unknown) {
    console.error(e);
  }
}

function useEnvAndGoMain(env: Environment) {
  openEnv(env.id);
  emit("use-env");
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
          <div v-if="(env.tags ?? []).length" class="card-tags">
            <span v-for="t in env.tags" :key="t" class="tag-chip">{{ t }}</span>
          </div>
          <p v-if="env.source === 'local_kubeconfig'" class="card-meta current-ctx">{{ currentContextLabel(env) }}</p>
          <p v-if="env.source === 'local_kubeconfig'" class="card-meta count">{{ env.contexts.length }} 个 context</p>
          <template v-if="env.source === 'ssh_tunnel'">
            <p class="card-meta">Host: {{ getTunnelForEnv(env)?.ssh_host ?? '—' }}</p>
            <p class="card-meta">远程 kubeconfig: {{ getTunnelForEnv(env)?.remote_kubeconfig_path ?? '—' }}</p>
          </template>
          <div class="card-actions" @click.stop>
            <button type="button" class="btn-use" @click="useEnvAndGoMain(env)">使用</button>
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
            <p class="form-hint">选择本机 ~/.ssh/config 中的 Host，建立隧道后使用远程主机上的 kubeconfig；留空则自动分配端口，或填写 1–65535 指定固定端口。映射方式在设置中统一配置。</p>
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
  gap: 0.35rem;
  margin-bottom: 0.5rem;
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
  margin-top: 1rem;
  padding-top: 1rem;
  border-top: 1px solid #f1f5f9;
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
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.75rem;
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
}
.card-meta {
  margin: 0 0 0.25rem 0;
  font-size: 0.8125rem;
  color: #64748b;
}
.card-meta.count {
  font-size: 0.75rem;
  color: #94a3b8;
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
