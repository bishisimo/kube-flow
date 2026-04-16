<script setup lang="ts">
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
  <div v-if="currentId && envState === 'disconnected'" class="disconnect-banner">
    <span class="disconnect-text">连接已断开</span>
    <span class="disconnect-detail">{{ envError }}</span>
    <button type="button" class="btn-reconnect" @click="emit('reconnect')">重连</button>
  </div>
  <div v-else-if="currentId && envState === 'connecting' && progress" class="connection-stepper">
    <div class="stepper-title">连接中：{{ progress.stage_label }}</div>
    <div v-if="progress.detail" class="stepper-detail">
      {{ progress.detail }}
    </div>
  </div>
  <div v-else-if="listError" class="error-banner">
    {{ listError }}
    <button v-if="currentId && reconnectOnListError" type="button" class="btn-reconnect" @click="emit('reconnect')">
      重连
    </button>
    <button type="button" class="error-dismiss" @click="emit('dismissError')">关闭</button>
  </div>
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
.disconnect-banner {
  padding: 0.75rem 1rem;
  background: #fef3c7;
  color: #b45309;
  border-bottom: 1px solid #fde68a;
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex-wrap: wrap;
}
.disconnect-text {
  font-weight: 500;
}
.disconnect-detail {
  flex: 1;
  min-width: 0;
  font-size: 0.875rem;
  opacity: 0.9;
  overflow: hidden;
  text-overflow: ellipsis;
}
.btn-reconnect {
  flex-shrink: 0;
  padding: 0.25rem 0.75rem;
  background: #b45309;
  color: #fff;
  border: none;
  border-radius: 4px;
  font-size: 0.875rem;
  cursor: pointer;
}
.btn-reconnect:hover {
  background: #92400e;
}
.connection-stepper {
  padding: 0.75rem 1rem;
  background: #f0f9ff;
  color: #0369a1;
  border-bottom: 1px solid #bae6fd;
  font-size: 0.875rem;
}
.stepper-title {
  font-weight: 500;
}
.stepper-detail {
  margin-top: 0.25rem;
  font-size: 0.8125rem;
  opacity: 0.9;
}
.error-banner {
  padding: 0.75rem 1rem;
  background: #fef2f2;
  color: #dc2626;
  font-size: 0.875rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  border-bottom: 1px solid #fecaca;
}
.error-dismiss {
  flex-shrink: 0;
  padding: 0.25rem 0.5rem;
  border: 1px solid #fca5a5;
  border-radius: 4px;
  background: #fff;
  color: #dc2626;
  font-size: 0.8125rem;
  cursor: pointer;
}
.error-dismiss:hover {
  background: #fef2f2;
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
