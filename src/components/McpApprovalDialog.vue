<script setup lang="ts">
/**
 * MCP 写/破坏操作人工确认对话框。
 */
import { computed, onMounted, onUnmounted, ref } from "vue";
import { NButton, NCheckbox, NModal, NSpace } from "naive-ui";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  mcpApprove,
  mcpDeny,
  type McpApprovalRequest,
} from "../api/mcp";
import { extractErrorMessage } from "../utils/errorMessage";

const visible = ref(false);
const current = ref<McpApprovalRequest | null>(null);
const grantSession = ref(false);
const busy = ref(false);
const error = ref<string | null>(null);
let unlisten: UnlistenFn | null = null;

const title = computed(() =>
  current.value?.force ? "MCP 危险操作确认" : "MCP 写操作确认"
);

async function onApprove() {
  if (!current.value) return;
  busy.value = true;
  error.value = null;
  try {
    await mcpApprove(
      current.value.request_id,
      !current.value.force && grantSession.value
    );
    visible.value = false;
    current.value = null;
  } catch (e) {
    error.value = extractErrorMessage(e);
  } finally {
    busy.value = false;
  }
}

async function onDeny() {
  if (!current.value) return;
  busy.value = true;
  error.value = null;
  try {
    await mcpDeny(current.value.request_id);
    visible.value = false;
    current.value = null;
  } catch (e) {
    error.value = extractErrorMessage(e);
  } finally {
    busy.value = false;
  }
}

onMounted(async () => {
  unlisten = await listen<McpApprovalRequest>("mcp-approval-request", (event) => {
    current.value = event.payload;
    grantSession.value = false;
    error.value = null;
    visible.value = true;
  });
});

onUnmounted(() => {
  if (unlisten) {
    unlisten();
    unlisten = null;
  }
});
</script>

<template>
  <NModal
    v-model:show="visible"
    preset="card"
    :title="title"
    style="width: 520px"
    :mask-closable="false"
    :closable="false"
  >
    <NSpace v-if="current" vertical :size="12">
      <div><strong>环境</strong>：{{ current.env_id }}</div>
      <div><strong>能力</strong>：{{ current.capability }}</div>
      <div><strong>方法</strong>：{{ current.method }}</div>
      <div style="font-size: 12px; white-space: pre-wrap; word-break: break-all; max-height: 180px; overflow: auto">
        {{ current.summary }}
      </div>
      <NCheckbox
        v-if="current.can_grant_session"
        v-model:checked="grantSession"
      >
        同时授予短时 SessionGrant（同类写操作免重复确认）
      </NCheckbox>
      <div v-if="current.force" style="color: var(--n-error-color); font-size: 12px">
        破坏类操作每次都必须确认，不能使用 SessionGrant。
      </div>
      <div v-if="error" style="color: var(--n-error-color)">{{ error }}</div>
      <NSpace justify="end">
        <NButton :disabled="busy" @click="onDeny">拒绝</NButton>
        <NButton type="primary" :loading="busy" @click="onApprove">批准</NButton>
      </NSpace>
    </NSpace>
  </NModal>
</template>
