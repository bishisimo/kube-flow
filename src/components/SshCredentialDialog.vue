<script setup lang="ts">
/**
 * SSH 认证凭证弹窗：在连接时无密码配置时弹出，让用户输入密码。
 * 密码缓存由父级 store 负责，此组件只负责 UI 与 emit。
 * 若用户需要持久化，应前往隧道设置页手动保存。
 */
import { ref, computed } from "vue";

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
  <Teleport to="body">
    <div v-if="visible" class="dialog-overlay" @click.self="handleCancel">
      <div class="dialog" role="dialog" aria-modal="true">
        <header class="dialog-header">
          <span class="dialog-icon">🔐</span>
          <div>
            <h2 class="dialog-title">SSH 认证</h2>
            <p class="dialog-subtitle">{{ tunnelName }} · {{ sshHost }}</p>
          </div>
        </header>

        <div class="dialog-body">
          <p class="dialog-desc">
            连接此隧道需要密码认证，本次输入仅保存到内存（应用重启后清除）。<br />
            <span class="hint">如需永久保存，请在隧道设置中配置密码。</span>
          </p>

          <label class="field-label" for="ssh-password">密码</label>
          <div class="password-wrap">
            <input
              id="ssh-password"
              :type="inputType"
              v-model="password"
              class="password-input"
              placeholder="输入 SSH 密码"
              autocomplete="current-password"
              autofocus
              :disabled="loading"
              @keydown="handleKeydown"
            />
            <button
              type="button"
              class="toggle-btn"
              :title="showPassword ? '隐藏密码' : '显示密码'"
              @click="showPassword = !showPassword"
            >
              {{ showPassword ? "🙈" : "👁" }}
            </button>
          </div>

          <p v-if="error" class="error-msg">{{ error }}</p>
        </div>

        <footer class="dialog-footer">
          <button type="button" class="btn btn-secondary" :disabled="loading" @click="handleCancel">
            取消
          </button>
          <button type="button" class="btn btn-primary" :disabled="loading" @click="handleConfirm">
            <span v-if="loading" class="spinner" />
            {{ loading ? "连接中…" : "确认" }}
          </button>
        </footer>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  background: rgba(15, 23, 42, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.dialog {
  background: #fff;
  border-radius: 14px;
  width: 420px;
  max-width: calc(100vw - 2rem);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.18), 0 4px 16px rgba(0, 0, 0, 0.1);
  overflow: hidden;
}

.dialog-header {
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 1.25rem 1.5rem 0;
}

.dialog-icon {
  font-size: 1.5rem;
  line-height: 1;
  margin-top: 2px;
}

.dialog-title {
  margin: 0;
  font-size: 1.0625rem;
  font-weight: 600;
  color: #0f172a;
}

.dialog-subtitle {
  margin: 0.2rem 0 0;
  font-size: 0.8125rem;
  color: #64748b;
}

.dialog-body {
  padding: 1rem 1.5rem 0.75rem;
}

.dialog-desc {
  margin: 0 0 1rem;
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
  margin-bottom: 0.375rem;
}

.password-wrap {
  position: relative;
  display: flex;
}

.password-input {
  flex: 1;
  padding: 0.5rem 2.75rem 0.5rem 0.75rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  font-size: 0.9375rem;
  outline: none;
  transition: border-color 0.15s, box-shadow 0.15s;
}

.password-input:focus {
  border-color: #2563eb;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
}

.password-input:disabled {
  background: #f1f5f9;
  color: #94a3b8;
}

.toggle-btn {
  position: absolute;
  right: 0.5rem;
  top: 50%;
  transform: translateY(-50%);
  background: none;
  border: none;
  cursor: pointer;
  font-size: 1rem;
  padding: 0.25rem;
  opacity: 0.6;
  transition: opacity 0.15s;
}

.toggle-btn:hover {
  opacity: 1;
}

.error-msg {
  margin: 0.5rem 0 0;
  font-size: 0.8125rem;
  color: #dc2626;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  padding: 0.75rem 1.5rem 1.25rem;
  border-top: 1px solid #f1f5f9;
}

.btn {
  padding: 0.5rem 1.25rem;
  border-radius: 8px;
  font-size: 0.875rem;
  font-weight: 500;
  cursor: pointer;
  border: none;
  transition: background 0.15s, opacity 0.15s;
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.btn-secondary {
  background: #f1f5f9;
  color: #475569;
}

.btn-secondary:hover:not(:disabled) {
  background: #e2e8f0;
}

.btn-primary {
  background: #2563eb;
  color: #fff;
}

.btn-primary:hover:not(:disabled) {
  background: #1d4ed8;
}

.spinner {
  width: 14px;
  height: 14px;
  border: 2px solid rgba(255, 255, 255, 0.4);
  border-top-color: #fff;
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
  flex-shrink: 0;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
