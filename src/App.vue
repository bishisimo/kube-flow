<script setup lang="ts">
import { RouterView } from "vue-router";
import { computed } from "vue";
import type { GlobalThemeOverrides } from "naive-ui";
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

/** Naive UI 全局主题微调：与 Workbench 数据台风格对齐，提升密度与层级一致性。 */
const naiveThemeOverrides = computed<GlobalThemeOverrides>(() => ({
  common: {
    primaryColor: "#2563eb",
    primaryColorHover: "#1d4ed8",
    primaryColorPressed: "#1e40af",
    borderRadius: "10px",
    fontSize: "13px",
    fontSizeSmall: "12px",
    fontSizeMedium: "13px",
    fontSizeLarge: "15px",
    heightMedium: "34px",
    lineHeight: "1.45",
  },
  Button: {
    borderRadiusMedium: "10px",
    heightMedium: "32px",
    paddingMedium: "0 14px",
    fontSizeMedium: "13px",
  },
  Input: {
    borderRadius: "10px",
    heightMedium: "34px",
    fontSizeMedium: "13px",
  },
  Select: {
    peers: {
      InternalSelection: {
        borderRadius: "10px",
        heightMedium: "34px",
        fontSizeMedium: "13px",
      },
    },
  },
  Card: {
    borderRadius: "14px",
    paddingMedium: "18px",
  },
  DataTable: {
    borderColor: "rgba(148, 163, 184, 0.35)",
    thColor: "#e8eff8",
    thColorHover: "#e0e9f5",
    tdColorHover: "#eff6ff",
    tdColorStriped: "rgba(248, 250, 252, 0.82)",
    thTextColor: "#64748b",
    tdTextColor: "#0f172a",
    thFontWeight: "650",
    borderRadius: "12px",
    tdPaddingSmall: "8px 12px",
    thPaddingSmall: "8px 12px",
  },
  Checkbox: {
    sizeMedium: "16px",
  },
}));

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
  <NConfigProvider :theme="useDarkTheme ? darkTheme : null" :theme-overrides="naiveThemeOverrides">
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
  --kf-text-secondary: #66768f;
  --kf-text-muted: #8a98ac;
  --kf-border: rgba(148, 163, 184, 0.24);
  --kf-border-strong: rgba(100, 116, 139, 0.34);
  --kf-surface: rgba(255, 255, 255, 0.92);
  --kf-surface-strong: #ffffff;
  --kf-bg-soft: #f3f6fb;
  --kf-bg-elevated: #eef3fa;
  --kf-primary: #2563eb;
  --kf-primary-soft: #e7efff;
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
  --kf-shadow-sm: 0 10px 24px rgba(15, 23, 42, 0.07);
  --kf-shadow-md: 0 16px 36px rgba(15, 23, 42, 0.11);
  /* Workbench 主列表 */
  --wb-canvas: #edf2f9;
  --wb-panel: #ffffff;
  --wb-table-header: linear-gradient(180deg, #e9f0f9 0%, #dde7f4 100%);
  --wb-table-border: rgba(148, 163, 184, 0.3);
  --wb-row-stripe: rgba(248, 250, 252, 0.7);
  --wb-row-hover: rgba(239, 246, 255, 0.95);
  --wb-row-selected: rgba(219, 234, 254, 0.55);
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
