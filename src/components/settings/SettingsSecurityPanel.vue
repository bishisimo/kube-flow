<script setup lang="ts">
import { ref, onMounted } from "vue";
import {
  NAlert,
  NButton,
  NCard,
  NCheckbox,
  NInput,
  NSpace,
  NTag,
} from "naive-ui";
import { useStrongholdStatusStore } from "../../stores/strongholdStatus";
import {
  securityGetSettings,
  securitySetCredentialStore,
  securitySetAutoLockMinutes,
  securitySetStrongholdPath,
  strongholdGetStatus,
  strongholdInitialize,
  strongholdUnlock,
  strongholdLock,
  credentialList,
  credentialDelete,
  type SecurityConfig,
  type CredentialInfo,
  type CredentialStoreKind,
} from "../../api/credential";
import { extractErrorMessage } from "../../utils/errorMessage";

const { strongholdStatus, refreshStrongholdStatus, setStrongholdStatus } = useStrongholdStatusStore();

const securityCfg = ref<SecurityConfig>({
  credential_store: "stronghold",
  stronghold_snapshot_path: "",
  auto_lock_minutes: 30,
});
const savedCredentials = ref<CredentialInfo[]>([]);
const masterPasswordInput = ref("");
const masterPasswordConfirm = ref("");
const showMasterPassword = ref(false);
const securityMsg = ref<string | null>(null);
const securityMsgIsError = ref(false);
const securityLoading = ref(false);
const editingStrongholdPath = ref(false);
const tempStrongholdPath = ref("");

function showSecurityMsg(msg: string, isError = false) {
  securityMsg.value = msg;
  securityMsgIsError.value = isError;
  setTimeout(() => (securityMsg.value = null), 3000);
}

async function handleSetCredentialStore(store: CredentialStoreKind) {
  securityLoading.value = true;
  try {
    await securitySetCredentialStore(store);
    securityCfg.value.credential_store = store;
    await refreshStrongholdStatus();
    showSecurityMsg("已保存");
  } catch (e) {
    showSecurityMsg(extractErrorMessage(e), true);
  } finally {
    securityLoading.value = false;
  }
}

async function handleAutoLockChange(minutes: number) {
  try {
    await securitySetAutoLockMinutes(minutes);
    securityCfg.value.auto_lock_minutes = minutes;
    showSecurityMsg("已保存");
  } catch (e) {
    showSecurityMsg(extractErrorMessage(e), true);
  }
}

async function handleSaveStrongholdPath() {
  if (!tempStrongholdPath.value.trim()) return;
  securityLoading.value = true;
  try {
    await securitySetStrongholdPath(tempStrongholdPath.value.trim());
    securityCfg.value.stronghold_snapshot_path = tempStrongholdPath.value.trim();
    await refreshStrongholdStatus();
    editingStrongholdPath.value = false;
    showSecurityMsg("路径已更新");
  } catch (e) {
    showSecurityMsg(extractErrorMessage(e), true);
  } finally {
    securityLoading.value = false;
  }
}

async function handleStrongholdInit() {
  if (!masterPasswordInput.value || masterPasswordInput.value !== masterPasswordConfirm.value) {
    showSecurityMsg("两次密码不一致", true);
    return;
  }
  securityLoading.value = true;
  try {
    await strongholdInitialize(masterPasswordInput.value);
    setStrongholdStatus("unlocked");
    masterPasswordInput.value = "";
    masterPasswordConfirm.value = "";
    showSecurityMsg("Stronghold 已初始化并解锁");
  } catch (e) {
    showSecurityMsg(extractErrorMessage(e), true);
  } finally {
    securityLoading.value = false;
  }
}

async function handleStrongholdUnlock() {
  if (!masterPasswordInput.value) {
    showSecurityMsg("请输入主密码", true);
    return;
  }
  securityLoading.value = true;
  try {
    await strongholdUnlock(masterPasswordInput.value);
    setStrongholdStatus("unlocked");
    masterPasswordInput.value = "";
    savedCredentials.value = await credentialList();
    showSecurityMsg("已解锁");
  } catch (e) {
    showSecurityMsg(extractErrorMessage(e), true);
  } finally {
    securityLoading.value = false;
  }
}

async function handleStrongholdLock() {
  await strongholdLock();
  setStrongholdStatus("locked");
  savedCredentials.value = [];
  showSecurityMsg("已锁定");
}

async function handleDeleteCredential(tunnelId: string) {
  securityLoading.value = true;
  try {
    await credentialDelete(tunnelId);
    savedCredentials.value = savedCredentials.value.filter((c) => c.tunnel_id !== tunnelId);
    showSecurityMsg("凭证已删除");
  } catch (e) {
    showSecurityMsg(extractErrorMessage(e), true);
  } finally {
    securityLoading.value = false;
  }
}

onMounted(async () => {
  try {
    const [cfg, status, credentials] = await Promise.all([
      securityGetSettings(),
      strongholdGetStatus(),
      credentialList(),
    ]);
    securityCfg.value = cfg;
    setStrongholdStatus(status);
    savedCredentials.value = credentials;
  } catch {
    // 静默：首次进入可能失败
  }
});
</script>

<template>
  <NCard title="凭证存储后端" size="small" class="sec-card" :bordered="true">
    <p class="card-desc">选择 SSH 密码的持久化方式。切换后端不会迁移已有凭证。</p>
    <NSpace wrap>
      <NButton
        :type="securityCfg.credential_store === 'stronghold' ? 'primary' : 'default'"
        :secondary="securityCfg.credential_store !== 'stronghold'"
        :disabled="securityLoading"
        @click="handleSetCredentialStore('stronghold')"
      >本地加密文件（默认）</NButton>
      <NButton
        :type="securityCfg.credential_store === 'os_keychain' ? 'primary' : 'default'"
        :secondary="securityCfg.credential_store !== 'os_keychain'"
        :disabled="securityLoading"
        @click="handleSetCredentialStore('os_keychain')"
      >系统钥匙串</NButton>
    </NSpace>
    <p class="backend-desc">
      <template v-if="securityCfg.credential_store === 'stronghold'">
        使用 AES-256-GCM 加密的本地文件，主密码通过 Argon2id 派生密钥，跨平台无依赖。
      </template>
      <template v-else>
        使用操作系统凭证管理器（macOS Keychain / Windows Credential Manager / libsecret），无需主密码。
      </template>
    </p>
  </NCard>

  <NCard
    v-if="securityCfg.credential_store === 'stronghold'"
    class="sec-card"
    :bordered="true"
  >
    <template #header>
      <div class="sec-card-head">
        <span>Stronghold 加密文件</span>
        <NTag
          v-if="strongholdStatus === 'uninitialized'"
          size="small"
          round
          :bordered="false"
        >未初始化</NTag>
        <NTag
          v-else-if="strongholdStatus === 'locked'"
          size="small"
          round
          type="warning"
          :bordered="false"
        >已锁定</NTag>
        <NTag
          v-else
          size="small"
          round
          type="success"
          :bordered="false"
        >已解锁</NTag>
      </div>
    </template>
    <div class="path-row">
      <span class="path-label">快照路径</span>
      <template v-if="!editingStrongholdPath">
        <span class="path-value">
          {{ securityCfg.stronghold_snapshot_path || "{app_data_dir}/credentials.hold（默认）" }}
        </span>
        <NButton text type="primary" @click="editingStrongholdPath = true; tempStrongholdPath = securityCfg.stronghold_snapshot_path">修改</NButton>
      </template>
      <template v-else>
        <NInput
          v-model:value="tempStrongholdPath"
          class="path-input"
          placeholder="留空使用默认路径"
        />
        <NButton quaternary @click="handleSaveStrongholdPath">保存</NButton>
        <NButton quaternary @click="editingStrongholdPath = false">取消</NButton>
      </template>
    </div>

    <div class="lock-row">
      <div class="option-label">自动锁定（分钟，0 = 运行期间不自动锁定）</div>
      <NSpace wrap>
        <NButton
          v-for="opt in [0, 15, 30, 60]"
          :key="opt"
          :type="securityCfg.auto_lock_minutes === opt ? 'primary' : 'default'"
          :secondary="securityCfg.auto_lock_minutes !== opt"
          @click="handleAutoLockChange(opt)"
        >{{ opt === 0 ? "不锁定" : `${opt} 分钟` }}</NButton>
      </NSpace>
    </div>

    <template v-if="strongholdStatus === 'uninitialized'">
      <p class="card-desc" style="margin-top: 1rem">首次使用需设置主密码以创建加密存储。</p>
      <NSpace vertical :size="10" style="max-width: 400px; margin-top: 0.5rem;">
        <NInput
          v-model:value="masterPasswordInput"
          :type="showMasterPassword ? 'text' : 'password'"
          placeholder="主密码"
        />
        <NInput
          v-model:value="masterPasswordConfirm"
          :type="showMasterPassword ? 'text' : 'password'"
          placeholder="确认主密码"
        />
        <NCheckbox v-model:checked="showMasterPassword">显示密码</NCheckbox>
        <NButton
          type="primary"
          :disabled="securityLoading"
          @click="handleStrongholdInit"
        >初始化 Stronghold</NButton>
      </NSpace>
    </template>

    <template v-else-if="strongholdStatus === 'locked'">
      <p class="card-desc" style="margin-top: 1rem">输入主密码解锁以管理凭证。</p>
      <NSpace vertical :size="10" style="max-width: 400px; margin-top: 0.5rem;">
        <NInput
          v-model:value="masterPasswordInput"
          :type="showMasterPassword ? 'text' : 'password'"
          placeholder="主密码"
          @keydown.enter="handleStrongholdUnlock"
        />
        <NCheckbox v-model:checked="showMasterPassword">显示密码</NCheckbox>
        <NButton type="primary" :disabled="securityLoading" @click="handleStrongholdUnlock">解锁</NButton>
      </NSpace>
    </template>

    <NButton
      v-else
      style="margin-top: 1rem"
      :disabled="securityLoading"
      @click="handleStrongholdLock"
    >锁定 Stronghold</NButton>

    <NAlert
      v-if="securityMsg"
      class="msg-alert"
      :type="securityMsgIsError ? 'error' : 'success'"
      :show-icon="true"
    >{{ securityMsg }}</NAlert>
  </NCard>

  <NCard
    v-if="strongholdStatus === 'unlocked' || securityCfg.credential_store === 'os_keychain'"
    title="已保存的凭证"
    size="small"
    class="sec-card"
    :bordered="true"
  >
    <p class="card-desc">以下为持久化后端中已保存的 SSH 隧道密码。</p>
    <p v-if="savedCredentials.length === 0" class="empty-tip">暂无已保存的凭证。</p>
    <div
      v-for="cred in savedCredentials"
      :key="cred.tunnel_id"
      class="cred-row"
    >
      <span class="cred-name">{{ cred.tunnel_id }}</span>
      <NTag size="small" :bordered="false">{{ cred.store }}</NTag>
      <NButton
        quaternary
        type="error"
        size="small"
        :disabled="securityLoading"
        @click="handleDeleteCredential(cred.tunnel_id)"
      >删除</NButton>
    </div>
    <NAlert
      v-if="securityMsg"
      class="msg-alert"
      :type="securityMsgIsError ? 'error' : 'success'"
      :show-icon="true"
    >{{ securityMsg }}</NAlert>
  </NCard>

  <NCard
    v-if="securityCfg.credential_store === 'os_keychain'"
    title="系统钥匙串"
    size="small"
    class="sec-card"
    :bordered="true"
  >
    <p class="card-desc">
      凭证存储于操作系统的凭证管理器中，无需主密码，由系统负责加密保护。<br />
      可通过各隧道配置页的「清除已保存的密码」按钮单独删除。
    </p>
  </NCard>
</template>

<style scoped>
.sec-card {
  max-width: 520px;
  margin-bottom: 1rem;
}
.card-desc {
  margin: 0 0 0.75rem;
  font-size: 0.875rem;
  color: #64748b;
  line-height: 1.55;
}
.backend-desc {
  margin: 0.75rem 0 0;
  font-size: 0.8125rem;
  color: #94a3b8;
  line-height: 1.5;
}
.sec-card-head {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
}
.path-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.875rem;
  flex-wrap: wrap;
}
.path-label {
  font-weight: 500;
  color: #475569;
  flex-shrink: 0;
}
.path-value {
  flex: 1;
  color: #64748b;
  font-family: ui-monospace, monospace;
  font-size: 0.8125rem;
  word-break: break-all;
  min-width: 0;
}
.path-input {
  flex: 1;
  min-width: 0;
  font-family: ui-monospace, monospace;
}
.lock-row {
  margin-top: 1rem;
}
.option-label {
  font-size: 0.8125rem;
  font-weight: 500;
  color: #64748b;
  margin-bottom: 0.5rem;
}
.cred-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.5rem 0.75rem;
  background: #f8fafc;
  border-radius: 8px;
  border: 1px solid #e2e8f0;
  font-size: 0.875rem;
  margin-bottom: 0.5rem;
}
.cred-name {
  flex: 1;
  font-family: ui-monospace, monospace;
  color: #1e293b;
  font-size: 0.8125rem;
  min-width: 0;
  word-break: break-all;
}
.empty-tip {
  font-size: 0.875rem;
  color: #94a3b8;
  margin: 0 0 0.5rem;
}
.msg-alert {
  margin-top: 0.75rem;
}
</style>
