<script setup lang="ts">
import { computed, ref, watch } from "vue";

interface Props {
  visible: boolean;
  title?: string;
  description?: string;
  loading?: boolean;
  error?: string | null;
}

const props = withDefaults(defineProps<Props>(), {
  title: "解锁 Stronghold",
  description: "当前操作需要访问已保存凭证，请输入主密码后继续。",
  loading: false,
  error: null,
});

const emit = defineEmits<{
  confirm: [password: string];
  cancel: [];
}>();

const password = ref("");
const showPassword = ref(false);
const localError = ref<string | null>(null);
const inputType = computed(() => (showPassword.value ? "text" : "password"));

watch(
  () => props.visible,
  (visible) => {
    if (visible) {
      password.value = "";
      localError.value = null;
      showPassword.value = false;
    }
  }
);

function handleConfirm() {
  if (!password.value) {
    localError.value = "请输入主密码";
    return;
  }
  localError.value = null;
  emit("confirm", password.value);
}

function handleCancel() {
  password.value = "";
  localError.value = null;
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
            <h2 class="dialog-title">{{ title }}</h2>
            <p class="dialog-subtitle">主密码解锁后会自动继续当前操作</p>
          </div>
        </header>

        <div class="dialog-body">
          <p class="dialog-desc">{{ description }}</p>

          <label class="field-label" for="stronghold-password">主密码</label>
          <div class="password-wrap">
            <input
              id="stronghold-password"
              v-model="password"
              :type="inputType"
              class="password-input"
              placeholder="输入 Stronghold 主密码"
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

          <p v-if="localError || error" class="error-msg">{{ localError || error }}</p>
        </div>

        <footer class="dialog-footer">
          <button type="button" class="btn btn-secondary" :disabled="loading" @click="handleCancel">
            取消
          </button>
          <button type="button" class="btn btn-primary" :disabled="loading" @click="handleConfirm">
            <span v-if="loading" class="spinner" />
            {{ loading ? "解锁中…" : "解锁并继续" }}
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

.field-label {
  display: block;
  margin-bottom: 0.4rem;
  font-size: 0.8125rem;
  font-weight: 600;
  color: #334155;
}

.password-wrap {
  display: flex;
  align-items: stretch;
  gap: 0.5rem;
}

.password-input {
  flex: 1;
  min-width: 0;
  padding: 0.7rem 0.85rem;
  border: 1px solid #cbd5e1;
  border-radius: 10px;
  font-size: 0.875rem;
  outline: none;
}

.password-input:focus {
  border-color: #3b82f6;
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.12);
}

.toggle-btn {
  border: 1px solid #cbd5e1;
  background: #fff;
  border-radius: 10px;
  padding: 0 0.8rem;
  cursor: pointer;
}

.error-msg {
  margin: 0.7rem 0 0;
  font-size: 0.8125rem;
  color: #dc2626;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.75rem;
  padding: 1rem 1.5rem 1.25rem;
}

.btn {
  border: none;
  border-radius: 10px;
  padding: 0.7rem 1rem;
  font-size: 0.875rem;
  font-weight: 600;
  cursor: pointer;
}

.btn-secondary {
  background: #e2e8f0;
  color: #334155;
}

.btn-primary {
  background: #2563eb;
  color: #fff;
}

.spinner {
  display: inline-block;
  width: 0.9rem;
  height: 0.9rem;
  margin-right: 0.35rem;
  border: 2px solid rgba(255, 255, 255, 0.5);
  border-top-color: #fff;
  border-radius: 999px;
  vertical-align: -2px;
  animation: spin 0.9s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
