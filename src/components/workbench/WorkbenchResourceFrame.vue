<script setup lang="ts">
import { NAlert, NButton, NSpin } from "naive-ui";
import type { ConnectionProgressPayload } from "../../stores/connection";

defineProps<{
  currentId: string | null;
  envState: string | null;
  envError: string | undefined;
  progress: ConnectionProgressPayload | undefined | null;
  listError: string | null;
  listLoading: boolean;
  envSwitching: boolean;
  envSwitchingName: string;
  reconnectOnListError: boolean;
}>();

const emit = defineEmits<{
  reconnect: [];
  dismissError: [];
}>();
</script>

<template>
  <NAlert v-if="currentId && envState === 'disconnected'" type="warning" class="state-alert" :show-icon="true">
    <div class="alert-row">
      <div>
        <div class="disconnect-text">连接已断开</div>
        <div class="disconnect-detail">{{ envError }}</div>
      </div>
      <NButton type="warning" secondary size="small" @click="emit('reconnect')">重连</NButton>
    </div>
  </NAlert>
  <div v-else-if="currentId && envState === 'connecting' && progress" class="connection-stepper">
    <div class="stepper-title">
      <NSpin size="small" />
      <span>连接中：{{ progress.stage_label }}</span>
    </div>
    <div v-if="progress.detail" class="stepper-detail">
      {{ progress.detail }}
    </div>
  </div>
  <NAlert v-else-if="listError" type="error" class="state-alert" :show-icon="true">
    <div class="alert-row">
      <span class="error-message">{{ listError }}</span>
      <div class="alert-actions">
        <NButton v-if="currentId && reconnectOnListError" type="error" secondary size="small" @click="emit('reconnect')">
          重连
        </NButton>
        <NButton size="small" quaternary @click="emit('dismissError')">关闭</NButton>
      </div>
    </div>
  </NAlert>
  <div v-else-if="listLoading" class="loading-state">
    <div class="loading-state-title">
      {{ envSwitching ? `正在切换到 ${envSwitchingName || "目标环境"}` : "加载中…" }}
    </div>
    <div class="loading-state-detail">
      {{ envSwitching ? "旧环境数据已清空，正在拉取新环境资源。" : "正在同步当前环境下的资源列表。" }}
    </div>
  </div>
  <slot v-else />
</template>

<style scoped>
.state-alert {
  border-radius: 0;
  border-left: none;
  border-right: none;
}
.alert-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.9rem;
}
.alert-actions {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
}
.error-message {
  min-width: 0;
}
.disconnect-text {
  font-weight: 500;
}
.disconnect-detail {
  font-size: 0.875rem;
  opacity: 0.92;
}
.connection-stepper {
  padding: 0.75rem 1rem;
  background: #f0f9ff;
  color: #0369a1;
  border-bottom: 1px solid #bae6fd;
  font-size: 0.875rem;
}
.stepper-title {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  font-weight: 500;
}
.stepper-detail {
  margin-top: 0.25rem;
  font-size: 0.8125rem;
  opacity: 0.9;
}
.loading-state {
  padding: 2rem 1.5rem;
  text-align: center;
  color: #64748b;
  font-size: 0.875rem;
  background: #fff;
}
.loading-state-title {
  font-size: 0.95rem;
  font-weight: 600;
  color: #334155;
}
.loading-state-detail {
  margin-top: 0.45rem;
  font-size: 0.8125rem;
  color: #64748b;
}
</style>
