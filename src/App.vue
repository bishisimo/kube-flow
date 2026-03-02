<script setup lang="ts">
import { RouterView } from "vue-router";
import { computed } from "vue";
import { useSshAuthStore } from "./stores/sshAuth";
import SshCredentialDialog from "./components/SshCredentialDialog.vue";

const sshAuth = useSshAuthStore();
const authDialogVisible = computed(() => sshAuth.pending !== null);

async function onCredentialConfirm(password: string) {
  await sshAuth.confirm(password);
}

function onCredentialCancel() {
  sshAuth.cancel();
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
