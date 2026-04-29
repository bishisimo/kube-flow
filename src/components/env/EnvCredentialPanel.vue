<script setup lang="ts">
/**
 * 凭证面板：展示"已配置/未配置"徽标、提供保存与清除入口。
 *
 * 自身只负责 UI 与输入态管理；凭证读写由 `state` 驱动（通常来自 useEnvCredential）。
 */
import { ref, computed } from "vue";
import { NAlert, NButton, NInput, NTag } from "naive-ui";
import type { EnvCredentialState } from "../../features/env/useEnvCredential";

const props = defineProps<{
  state: EnvCredentialState;
  title?: string;
  description?: string;
  savePlaceholder?: string;
  modifyPlaceholder?: string;
  disabled?: boolean;
}>();

const showInput = ref(false);
const draft = ref("");

const { exists, loading, message } = props.state;

const canToggleInput = computed(() => !props.disabled && !loading.value);

function onToggleInput() {
  if (!canToggleInput.value) return;
  showInput.value = !showInput.value;
  draft.value = "";
  message.value = null;
}

async function save() {
  const ok = await props.state.save(draft.value);
  if (ok) {
    draft.value = "";
    showInput.value = false;
  }
}

async function remove() {
  await props.state.remove();
}
</script>

<template>
  <div class="env-credential-panel">
    <div class="cred-header">
      <span class="cred-title">{{ title ?? "凭证" }}</span>
      <NTag size="small" round :bordered="false" :type="exists ? 'success' : 'default'">
        {{ exists ? "已保存" : "未配置" }}
      </NTag>
    </div>
    <p v-if="description" class="cred-desc">{{ description }}</p>

    <div v-if="exists && !showInput" class="cred-actions">
      <NButton size="small" :disabled="disabled || loading" @click="onToggleInput">
        修改密码
      </NButton>
      <NButton size="small" type="error" ghost :disabled="disabled" :loading="loading" @click="remove">
        清除密码
      </NButton>
    </div>

    <div v-if="!exists || showInput" class="cred-input-row">
      <NInput
        v-model:value="draft"
        type="password"
        show-password-on="click"
        class="cred-input kf-password-input"
        :placeholder="exists ? (modifyPlaceholder ?? '输入新密码') : (savePlaceholder ?? '输入密码')"
        :disabled="disabled || loading"
        @keyup.enter="save"
      />
      <NButton
        type="primary"
        size="small"
        :loading="loading"
        :disabled="disabled || !draft"
        @click="save"
      >
        保存密码
      </NButton>
      <NButton v-if="exists" size="small" :disabled="loading" @click="onToggleInput">
        取消
      </NButton>
    </div>

    <NAlert
      v-if="message"
      class="cred-msg"
      size="small"
      :type="message.type === 'ok' ? 'success' : 'error'"
      :show-icon="false"
    >
      {{ message.text }}
    </NAlert>
  </div>
</template>

<style scoped>
.env-credential-panel {
  padding: 0.75rem 0.85rem;
  border-radius: 10px;
  border: 1px solid var(--kf-border, #e2e8f0);
  background: #f8fafc;
}
.cred-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
.cred-title {
  font-size: 0.8125rem;
  font-weight: 600;
  color: #374151;
}
.cred-desc {
  margin: 0.35rem 0 0.6rem;
  font-size: 0.75rem;
  color: #64748b;
  line-height: 1.5;
}
.cred-actions,
.cred-input-row {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  flex-wrap: wrap;
  margin-top: 0.35rem;
}
.cred-input {
  flex: 1;
  min-width: 12rem;
}

.cred-msg {
  margin-top: 0.55rem;
}
</style>
