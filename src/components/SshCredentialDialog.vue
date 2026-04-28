<script setup lang="ts">
/**
 * SSH 认证凭证弹窗：在连接时无密码配置时弹出，让用户输入密码。
 * 密码缓存由父级 store 负责，此组件只负责 UI 与 emit。
 * 若用户需要持久化，应前往隧道设置页手动保存。
 */
import { ref, computed } from "vue";
import { NAlert, NButton, NInput } from "naive-ui";
import BaseModal from "./base/BaseModal.vue";

interface Props {
  /** 对应的 SSH 隧道 ID */
  tunnelId: string;
  /** 隧道显示名称，用于弹窗标题 */
  tunnelName: string;
  /** 服务器主机名，显示给用户 */
  sshHost: string;
  /** 弹窗可见性 */
  visible: boolean;
}

defineProps<Props>();
const emit = defineEmits<{
  /** 用户确认输入，附带密码（父组件负责缓存与重试） */
  confirm: [password: string];
  /** 用户取消 */
  cancel: [];
}>();

const password = ref("");
const loading = ref(false);
const error = ref<string | null>(null);
const showPassword = ref(false);

const inputType = computed(() => (showPassword.value ? "text" : "password"));

function handleConfirm() {
  if (!password.value) {
    error.value = "请输入密码";
    return;
  }
  error.value = null;
  loading.value = true;
  emit("confirm", password.value);
  password.value = "";
  loading.value = false;
}

function handleCancel() {
  password.value = "";
  error.value = null;
  emit("cancel");
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Enter") handleConfirm();
  if (e.key === "Escape") handleCancel();
}
</script>

<template>
  <BaseModal :visible="visible" :title="`SSH 认证 · ${tunnelName}`" width="500px" @close="handleCancel">
    <p class="dialog-subtitle">{{ sshHost }}</p>
    <p class="dialog-desc">
      连接此隧道需要密码认证，本次输入仅保存到内存（应用重启后清除）。<br />
      <span class="hint">如需永久保存，请在隧道设置中配置密码。</span>
    </p>
    <label class="field-label" for="ssh-password">密码</label>
    <NInput
      id="ssh-password"
      v-model:value="password"
      class="kf-password-input"
      :type="inputType"
      placeholder="输入 SSH 密码"
      show-password-on="click"
      :disabled="loading"
      @keydown="handleKeydown"
    />
    <NAlert v-if="error" type="error" :show-icon="false" class="error-msg">
      {{ error }}
    </NAlert>
    <template #footer>
      <NButton secondary :disabled="loading" @click="handleCancel">取消</NButton>
      <NButton type="primary" :loading="loading" @click="handleConfirm">
        {{ loading ? "连接中…" : "确认" }}
      </NButton>
    </template>
  </BaseModal>
</template>

<style scoped>
.dialog-subtitle {
  margin: 0 0 0.5rem;
  font-size: 0.8125rem;
  color: #64748b;
}

.dialog-desc {
  margin: 0 0 0.9rem;
  font-size: 0.875rem;
  color: #475569;
  line-height: 1.55;
}

.hint {
  color: #94a3b8;
  font-size: 0.8125rem;
}

.field-label {
  display: block;
  font-size: 0.8125rem;
  font-weight: 500;
  color: #374151;
  margin-bottom: 0.4rem;
}

.error-msg {
  margin-top: 0.7rem;
}

</style>
