<script setup lang="ts">
import { RouterView } from "vue-router";
import { computed } from "vue";
import { darkTheme, NConfigProvider, NDialogProvider, NGlobalStyle, NMessageProvider } from "naive-ui";
import { useSshAuthStore } from "./stores/sshAuth";
import { useStrongholdAuthStore } from "./stores/strongholdAuth";
import SshCredentialDialog from "./components/SshCredentialDialog.vue";
import StrongholdUnlockDialog from "./components/StrongholdUnlockDialog.vue";

const sshAuth = useSshAuthStore();
const strongholdAuth = useStrongholdAuthStore();
const authDialogVisible = computed(() => sshAuth.pending.value !== null);
const strongholdDialogVisible = computed(() => strongholdAuth.pending.value !== null);
const useDarkTheme = computed(() => false);

async function onCredentialConfirm(password: string) {
  await sshAuth.confirm(password);
}

function onCredentialCancel() {
  sshAuth.cancel();
}

async function onStrongholdConfirm(password: string) {
  await strongholdAuth.confirm(password);
}

function onStrongholdCancel() {
  strongholdAuth.cancel();
}
</script>

<template>
  <NConfigProvider :theme="useDarkTheme ? darkTheme : null">
    <NGlobalStyle />
    <NDialogProvider>
      <NMessageProvider>
        <RouterView />

        <!-- 全局 SSH 密码输入弹窗：任意命令触发 SSH_AUTH_REQUIRED 时弹出 -->
        <SshCredentialDialog
          v-if="sshAuth.pending.value"
          :tunnel-id="sshAuth.pending.value.tunnelId"
          :tunnel-name="sshAuth.pending.value.tunnelName"
          :ssh-host="sshAuth.pending.value.sshHost"
          :visible="authDialogVisible"
          @confirm="onCredentialConfirm"
          @cancel="onCredentialCancel"
        />

        <StrongholdUnlockDialog
          v-if="strongholdAuth.pending.value"
          :visible="strongholdDialogVisible"
          :title="strongholdAuth.pending.value.title"
          :description="strongholdAuth.pending.value.description"
          :loading="strongholdAuth.loading.value"
          :error="strongholdAuth.error.value"
          @confirm="onStrongholdConfirm"
          @cancel="onStrongholdCancel"
        />
      </NMessageProvider>
    </NDialogProvider>
  </NConfigProvider>
</template>

<style>
:root {
  font-family: Inter, system-ui, -apple-system, Segoe UI, Roboto, sans-serif;
  font-size: 14px;
  line-height: 1.5;
  color: #0f172a;
  background-color: #f3f6fb;
  --kf-text-primary: #0f172a;
  --kf-text-secondary: #64748b;
  --kf-border: rgba(148, 163, 184, 0.26);
  --kf-surface: rgba(255, 255, 255, 0.9);
  --kf-surface-strong: #ffffff;
  --kf-bg-soft: #f4f7fc;
  --kf-primary: #2563eb;
  --kf-primary-soft: #e8f0ff;
  --kf-success: #16a34a;
  --kf-success-soft: #f0fdf4;
  --kf-warning: #d97706;
  --kf-warning-soft: #fff7ed;
  --kf-danger: #dc2626;
  --kf-danger-soft: #fef2f2;
  --kf-info: #0891b2;
  --kf-info-soft: #ecfeff;
  --kf-radius-sm: 10px;
  --kf-radius-md: 14px;
  --kf-shadow-sm: 0 10px 24px rgba(15, 23, 42, 0.08);
  --kf-shadow-md: 0 16px 36px rgba(15, 23, 42, 0.12);
}
* {
  box-sizing: border-box;
}
body, #app {
  margin: 0;
  min-height: 100vh;
  background: radial-gradient(circle at 20% 0%, #e8f0ff 0%, #f7f9fd 40%, #f3f6fb 100%);
}
</style>
