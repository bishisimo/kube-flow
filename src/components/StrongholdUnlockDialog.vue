<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NAlert, NButton, NInput } from "naive-ui";
import BaseModal from "./base/BaseModal.vue";

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
  <BaseModal :visible="visible" :title="title" width="500px" @close="handleCancel">
    <p class="dialog-subtitle">主密码解锁后会自动继续当前操作</p>
    <p class="dialog-desc">{{ description }}</p>
    <label class="field-label" for="stronghold-password">主密码</label>
    <NInput
      id="stronghold-password"
      v-model:value="password"
      class="kf-password-input"
      :type="inputType"
      placeholder="输入 Stronghold 主密码"
      show-password-on="click"
      :disabled="loading"
      @keydown="handleKeydown"
    />
    <NAlert v-if="localError || error" class="error-msg" type="error" :show-icon="false">
      {{ localError || error }}
    </NAlert>
    <template #footer>
      <NButton secondary :disabled="loading" @click="handleCancel">取消</NButton>
      <NButton type="primary" :loading="loading" @click="handleConfirm">
        {{ loading ? "解锁中…" : "解锁并继续" }}
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

.field-label {
  display: block;
  margin-bottom: 0.4rem;
  font-size: 0.8125rem;
  font-weight: 600;
  color: #334155;
}

.error-msg {
  margin: 0.7rem 0 0;
}

</style>
