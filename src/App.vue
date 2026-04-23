<script setup lang="ts">
import { RouterView } from "vue-router";
import { computed } from "vue";
import { darkTheme, NConfigProvider, NDialogProvider, NGlobalStyle, NMessageProvider } from "naive-ui";
import { appChromeIsDark } from "./stores/appChromeTheme";
import { buildNaiveThemeOverrides } from "./kf";
import { useSshAuthStore } from "./stores/sshAuth";
import { useStrongholdAuthStore } from "./stores/strongholdAuth";
import SshCredentialDialog from "./components/SshCredentialDialog.vue";
import StrongholdUnlockDialog from "./components/StrongholdUnlockDialog.vue";

const sshAuth = useSshAuthStore();
const strongholdAuth = useStrongholdAuthStore();
const authDialogVisible = computed(() => sshAuth.pending.value !== null);
const strongholdDialogVisible = computed(() => strongholdAuth.pending.value !== null);
const useDarkTheme = appChromeIsDark;

const naiveThemeOverrides = computed(() => buildNaiveThemeOverrides(appChromeIsDark.value));

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
  /* 与主色/边框混色时替代写死 #fff（须先于依赖它的 --wb-row-*） */
  --kf-mix-surface: #ffffff;
  /* Workbench：石色底上叠暖光（珊瑚/琥珀）与松石绿（表头/选中/焦点），与全局主色蓝分工 */
  --wb-accent-warm: #ea580c;
  --wb-accent-spring: #059669;
  --wb-accent-forest: #0f766e;
  --wb-teal-soft: color-mix(in srgb, var(--wb-accent-forest) 13%, #ffffff);
  --wb-warm-soft: color-mix(in srgb, var(--wb-accent-warm) 11%, #fff7ed);
  --wb-focus-ring: 0 0 0 3px
    color-mix(
      in srgb,
      var(--wb-accent-forest) 20%,
      color-mix(in srgb, var(--wb-accent-warm) 12%, transparent)
    );
  --wb-canvas:
    radial-gradient(ellipse 86% 56% at 100% -6%, color-mix(in srgb, var(--wb-accent-warm) 12%, transparent), transparent 54%),
    radial-gradient(ellipse 76% 50% at -6% 104%, color-mix(in srgb, var(--wb-accent-forest) 9%, transparent), transparent 50%),
    radial-gradient(ellipse 50% 34% at 86% 96%, color-mix(in srgb, var(--kf-warning) 9%, transparent), transparent 44%),
    linear-gradient(168deg, #e7e5e2 0%, #e9ebe7 36%, #eef0ec 100%);
  --wb-panel: #fafaf9;
  --wb-panel-soft: color-mix(in srgb, var(--wb-accent-forest) 3.5%, #f3f3f2);
  --wb-panel-elevated: #ffffff;
  --wb-line: rgba(68, 64, 60, 0.14);
  --wb-line-strong: rgba(41, 37, 36, 0.22);
  --wb-text-primary: #1c1917;
  --wb-text-secondary: #57534e;
  --wb-table-heading-text: #475569;
  --wb-text-meta: #6f6863;
  --wb-chip: color-mix(in srgb, var(--kf-warning) 24%, #fffbeb);
  --wb-chip-text: #9a3412;
  --wb-overlay: linear-gradient(180deg, rgba(255, 255, 255, 0.99), rgba(247, 246, 245, 0.98));
  /* 表格：科技灰表头 + 白底表体（无斑马）；悬停/选中仍为琥珀+松石 */
  --wb-table-header: linear-gradient(180deg, #f8fafc 0%, #eef2f7 42%, #e2e8f0 100%);
  --wb-table-body: #ffffff;
  --wb-table-body-stripe: var(--wb-table-body);
  --wb-table-border: rgba(68, 64, 60, 0.12);
  --wb-row-stripe: var(--wb-table-body);
  --wb-row-hover: color-mix(
    in srgb,
    var(--kf-warning) 10%,
    color-mix(in srgb, var(--wb-accent-forest) 4.8%, #fafaf9)
  );
  --wb-row-selected: color-mix(
    in srgb,
    var(--wb-accent-forest) 14%,
    color-mix(in srgb, var(--wb-accent-spring) 8%, #ecfdf5)
  );
  /* 内嵌终端 / 深色工作条：不随文色反转为浅色，保持类终端对比 */
  --kf-embed-t-canvas: #0f172a;
  --kf-embed-t-slate: #1e293b;
  --kf-embed-t-foreground: #f8fafc;
  --kf-embed-t-foreground-subtle: #cbd5e1;
  --kf-embed-t-warn-ink: #fef3c7;
  --kf-embed-t-warn-glow: #fde68a;
  --kf-embed-t-ok-ink: #dcfce7;
  --kf-embed-t-ok-glow: #bbf7d0;
  --kf-embed-t-blue-ink: #dbeafe;
}
:root[data-kf-chrome="dark"] {
  color-scheme: dark;
  color: #e2e8f0;
  background-color: #0b1220;
  --kf-text-primary: #f1f5f9;
  --kf-text-secondary: #94a3b8;
  --kf-text-muted: #64748b;
  --kf-border: rgba(148, 163, 184, 0.16);
  --kf-border-strong: rgba(148, 163, 184, 0.28);
  --kf-surface: rgba(30, 41, 59, 0.86);
  --kf-surface-strong: #1e293b;
  --kf-bg-soft: #0b1220;
  --kf-bg-elevated: #1e293b;
  --kf-primary: #3b82f6;
  --kf-primary-soft: rgba(59, 130, 246, 0.18);
  --kf-success-soft: rgba(16, 185, 129, 0.14);
  --kf-warning-soft: rgba(245, 158, 11, 0.14);
  --kf-danger-soft: rgba(248, 113, 113, 0.14);
  --kf-info-soft: rgba(34, 211, 238, 0.12);
  --kf-shadow-sm: 0 10px 24px rgba(0, 0, 0, 0.35);
  --kf-shadow-md: 0 16px 36px rgba(0, 0, 0, 0.45);
  --kf-mix-surface: #1e293b;
  --wb-accent-warm: #fb923c;
  --wb-accent-spring: #34d399;
  --wb-accent-forest: #14b8a6;
  --wb-teal-soft: color-mix(in srgb, var(--wb-accent-forest) 22%, #134e4a);
  --wb-warm-soft: color-mix(in srgb, var(--wb-accent-warm) 14%, #292524);
  --wb-focus-ring: 0 0 0 3px
    color-mix(
      in srgb,
      var(--wb-accent-forest) 22%,
      color-mix(in srgb, var(--wb-accent-warm) 14%, transparent)
    );
  --wb-canvas:
    radial-gradient(ellipse 82% 54% at 100% 0%, color-mix(in srgb, var(--wb-accent-warm) 14%, transparent), transparent 54%),
    radial-gradient(ellipse 70% 48% at 0% 100%, color-mix(in srgb, var(--wb-accent-forest) 11%, transparent), transparent 50%),
    radial-gradient(ellipse 46% 30% at 88% 92%, color-mix(in srgb, var(--kf-warning) 10%, transparent), transparent 40%),
    #0c1016;
  --wb-panel: #1e293b;
  --wb-panel-soft: color-mix(in srgb, var(--wb-accent-forest) 6%, #151b24);
  --wb-panel-elevated: color-mix(in srgb, var(--wb-accent-forest) 4.5%, #1e293b);
  --wb-line: rgba(214, 211, 209, 0.14);
  --wb-line-strong: rgba(231, 229, 228, 0.22);
  --wb-text-primary: #f1f5f9;
  --wb-text-secondary: #a8a29e;
  --wb-table-heading-text: #94a3b8;
  --wb-text-meta: #a6a09b;
  --wb-chip: color-mix(in srgb, var(--kf-warning) 24%, rgba(254, 243, 199, 0.14));
  --wb-chip-text: #fde68a;
  --wb-overlay: linear-gradient(180deg, rgba(15, 23, 42, 0.98), rgba(15, 23, 42, 0.95));
  --wb-table-header: linear-gradient(180deg, #1e293b 0%, #172554 52%, #151d2e 100%);
  --wb-table-body: #1e293b;
  --wb-table-body-stripe: var(--wb-table-body);
  --wb-table-border: rgba(148, 163, 184, 0.13);
  --wb-row-stripe: var(--wb-table-body);
  --wb-row-hover: color-mix(
    in srgb,
    var(--wb-accent-warm) 9%,
    color-mix(in srgb, var(--wb-accent-forest) 11%, var(--kf-mix-surface))
  );
  --wb-row-selected: color-mix(
    in srgb,
    var(--wb-accent-forest) 22%,
    color-mix(in srgb, var(--wb-accent-spring) 9%, #134e4a)
  );
  --kf-embed-t-canvas: #0f172a;
  --kf-embed-t-slate: #1e293b;
  --kf-embed-t-foreground: #f8fafc;
  --kf-embed-t-foreground-subtle: #cbd5e1;
  --kf-embed-t-warn-ink: #fef3c7;
  --kf-embed-t-warn-glow: #fde68a;
  --kf-embed-t-ok-ink: #dcfce7;
  --kf-embed-t-ok-glow: #bbf7d0;
  --kf-embed-t-blue-ink: #dbeafe;
}
* {
  box-sizing: border-box;
}
body, #app {
  margin: 0;
  min-height: 100vh;
  background:
    radial-gradient(ellipse 72% 52% at 14% -4%, color-mix(in srgb, var(--wb-accent-warm) 11%, transparent), transparent 48%),
    radial-gradient(ellipse 64% 46% at 94% 4%, color-mix(in srgb, var(--wb-accent-forest) 9%, transparent), transparent 44%),
    radial-gradient(circle at 24% 0%, #e6e4e1 0%, #f3f2f0 40%, #eeedea 100%);
}
:root[data-kf-chrome="dark"] body,
:root[data-kf-chrome="dark"] #app {
  background:
    radial-gradient(ellipse 68% 50% at 88% -6%, color-mix(in srgb, var(--wb-accent-warm) 12%, transparent), transparent 46%),
    radial-gradient(ellipse 58% 42% at 6% 4%, color-mix(in srgb, var(--wb-accent-forest) 10%, transparent), transparent 40%),
    radial-gradient(circle at 18% 0%, #0f1a18 0%, #0b1218 45%, #0a0f14 100%);
}
</style>
