<script setup lang="ts">
import { ref, onMounted } from "vue";
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
    // 静默处理
  }
});
</script>

<template>
  <!-- 凭证存储后端 -->
  <section class="card">
    <h2 class="card-title">凭证存储后端</h2>
    <p class="card-desc">选择 SSH 密码的持久化方式。切换后端不会迁移已有凭证。</p>
    <div class="level-options">
      <button
        type="button"
        class="level-btn"
        :class="{ active: securityCfg.credential_store === 'stronghold' }"
        :disabled="securityLoading"
        @click="handleSetCredentialStore('stronghold')"
      >
        本地加密文件（默认）
      </button>
      <button
        type="button"
        class="level-btn"
        :class="{ active: securityCfg.credential_store === 'os_keychain' }"
        :disabled="securityLoading"
        @click="handleSetCredentialStore('os_keychain')"
      >
        系统钥匙串
      </button>
    </div>
    <p class="backend-desc">
      <template v-if="securityCfg.credential_store === 'stronghold'">
        使用 AES-256-GCM 加密的本地文件，主密码通过 Argon2id 派生密钥，跨平台无依赖。
      </template>
      <template v-else>
        使用操作系统凭证管理器（macOS Keychain / Windows Credential Manager / libsecret），无需主密码。
      </template>
    </p>
  </section>

  <!-- Stronghold 配置（仅 stronghold 模式显示） -->
  <template v-if="securityCfg.credential_store === 'stronghold'">
    <section class="card">
      <h2 class="card-title">
        Stronghold 加密文件
        <span
          class="status-badge"
          :class="{
            'badge-init': strongholdStatus === 'uninitialized',
            'badge-locked': strongholdStatus === 'locked',
            'badge-unlocked': strongholdStatus === 'unlocked',
          }"
        >
          {{
            strongholdStatus === "uninitialized"
              ? "未初始化"
              : strongholdStatus === "locked"
              ? "已锁定"
              : "已解锁"
          }}
        </span>
      </h2>

      <!-- 文件路径 -->
      <div class="path-row">
        <span class="path-label">快照路径</span>
        <template v-if="!editingStrongholdPath">
          <span class="path-value">
            {{ securityCfg.stronghold_snapshot_path || "{app_data_dir}/credentials.hold（默认）" }}
          </span>
          <button
            type="button"
            class="link-btn"
            @click="() => { editingStrongholdPath = true; tempStrongholdPath = securityCfg.stronghold_snapshot_path; }"
          >
            修改
          </button>
        </template>
        <template v-else>
          <input
            v-model="tempStrongholdPath"
            class="path-input"
            placeholder="留空使用默认路径"
          />
          <button type="button" class="link-btn" @click="handleSaveStrongholdPath">保存</button>
          <button type="button" class="link-btn" @click="editingStrongholdPath = false">取消</button>
        </template>
      </div>

      <!-- 自动锁定 -->
      <div class="option-group" style="margin-top: 1rem">
        <label class="option-label">自动锁定（分钟，0 = 运行期间不自动锁定）</label>
        <div class="option-buttons">
          <button
            v-for="opt in [0, 15, 30, 60]"
            :key="opt"
            type="button"
            class="level-btn"
            :class="{ active: securityCfg.auto_lock_minutes === opt }"
            @click="handleAutoLockChange(opt)"
          >
            {{ opt === 0 ? "不锁定" : `${opt} 分钟` }}
          </button>
        </div>
      </div>

      <!-- 初始化表单 -->
      <template v-if="strongholdStatus === 'uninitialized'">
        <p class="card-desc" style="margin-top: 1rem">首次使用需设置主密码以创建加密存储。</p>
        <div class="master-form">
          <input
            :type="showMasterPassword ? 'text' : 'password'"
            v-model="masterPasswordInput"
            class="master-input"
            placeholder="主密码"
          />
          <input
            :type="showMasterPassword ? 'text' : 'password'"
            v-model="masterPasswordConfirm"
            class="master-input"
            placeholder="确认主密码"
          />
          <label class="checkbox-row">
            <input type="checkbox" v-model="showMasterPassword" />
            显示密码
          </label>
          <button
            type="button"
            class="btn-action"
            :disabled="securityLoading"
            @click="handleStrongholdInit"
          >
            初始化 Stronghold
          </button>
        </div>
      </template>

      <!-- 解锁表单 -->
      <template v-else-if="strongholdStatus === 'locked'">
        <p class="card-desc" style="margin-top: 1rem">输入主密码解锁以管理凭证。</p>
        <div class="master-form">
          <input
            :type="showMasterPassword ? 'text' : 'password'"
            v-model="masterPasswordInput"
            class="master-input"
            placeholder="主密码"
            @keydown.enter="handleStrongholdUnlock"
          />
          <label class="checkbox-row">
            <input type="checkbox" v-model="showMasterPassword" />
            显示密码
          </label>
          <button
            type="button"
            class="btn-action"
            :disabled="securityLoading"
            @click="handleStrongholdUnlock"
          >
            解锁
          </button>
        </div>
      </template>

      <!-- 已解锁操作 -->
      <template v-else>
        <button
          type="button"
          class="btn-action btn-secondary-action"
          style="margin-top: 1rem"
          @click="handleStrongholdLock"
        >
          锁定 Stronghold
        </button>
      </template>

      <p v-if="securityMsg" class="message" :class="{ error: securityMsgIsError }">
        <span v-if="!securityMsgIsError" class="message-icon">✓</span>
        {{ securityMsg }}
      </p>
    </section>
  </template>

  <!-- 已保存凭证管理 -->
  <section class="card" v-if="strongholdStatus === 'unlocked' || securityCfg.credential_store === 'os_keychain'">
    <h2 class="card-title">已保存的凭证</h2>
    <p class="card-desc">以下为持久化后端中已保存的 SSH 隧道密码。</p>
    <template v-if="savedCredentials.length === 0">
      <p class="empty-tip">暂无已保存的凭证。</p>
    </template>
    <ul v-else class="credential-list">
      <li v-for="cred in savedCredentials" :key="cred.tunnel_id" class="credential-item">
        <span class="cred-name">{{ cred.tunnel_id }}</span>
        <span class="cred-store">{{ cred.store }}</span>
        <button
          type="button"
          class="cred-delete"
          :disabled="securityLoading"
          @click="handleDeleteCredential(cred.tunnel_id)"
        >
          删除
        </button>
      </li>
    </ul>
    <p v-if="securityMsg" class="message" :class="{ error: securityMsgIsError }">
      <span v-if="!securityMsgIsError" class="message-icon">✓</span>
      {{ securityMsg }}
    </p>
  </section>

  <!-- OS 钥匙串说明（无法枚举） -->
  <section class="card" v-if="securityCfg.credential_store === 'os_keychain'">
    <h2 class="card-title">系统钥匙串</h2>
    <p class="card-desc">
      凭证存储于操作系统的凭证管理器中，无需主密码，由系统负责加密保护。<br />
      可通过各隧道配置页的「清除已保存的密码」按钮单独删除。
    </p>
    <p v-if="securityMsg" class="message" :class="{ error: securityMsgIsError }">
      <span v-if="!securityMsgIsError" class="message-icon">✓</span>
      {{ securityMsg }}
    </p>
  </section>
</template>
