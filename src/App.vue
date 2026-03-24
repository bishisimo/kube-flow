<script setup lang="ts">
import { RouterView } from "vue-router";
import { computed } from "vue";
import { useSshAuthStore } from "./stores/sshAuth";
import { useStrongholdAuthStore } from "./stores/strongholdAuth";
import SshCredentialDialog from "./components/SshCredentialDialog.vue";
import StrongholdUnlockDialog from "./components/StrongholdUnlockDialog.vue";

const sshAuth = useSshAuthStore();
const strongholdAuth = useStrongholdAuthStore();
const authDialogVisible = computed(() => sshAuth.pending.value !== null);
const strongholdDialogVisible = computed(() => strongholdAuth.pending.value !== null);

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
</template>

<style>
:root {
  font-family: system-ui, -apple-system, Segoe UI, Roboto, sans-serif;
  font-size: 14px;
  line-height: 1.5;
  color: #1a1a1a;
  background-color: #fff;
}
* {
  box-sizing: border-box;
}
body, #app {
  margin: 0;
  min-height: 100vh;
}
</style>
