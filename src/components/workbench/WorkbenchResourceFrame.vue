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
  <section class="resource-frame">
    <NAlert v-if="currentId && envState === 'disconnected'" type="warning" class="state-alert" :show-icon="true">
      <div class="alert-row">
        <div>
          <div class="disconnect-text">连接已断开</div>
          <div class="disconnect-detail">{{ envError }}</div>
        </div>
        <NButton type="warning" secondary size="small" @click="emit('reconnect')">重连</NButton>
      </div>
    </NAlert>
    <div v-else-if="currentId && envState === 'connecting'" class="connection-stepper">
      <div class="stepper-title">
        <NSpin size="small" />
        <span>连接中：{{ progress?.stage_label || "正在获取资源" }}</span>
      </div>
      <div v-if="progress?.detail || listLoading" class="stepper-detail">
        {{ progress?.detail || "请稍候，正在同步当前视图数据…" }}
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
      <NSpin size="large" class="loading-state-spin" />
      <div class="loading-state-title">
        {{ envSwitching ? `正在切换到 ${envSwitchingName || "目标环境"}` : "加载中…" }}
      </div>
      <div class="loading-state-detail">
        {{ envSwitching ? "旧环境数据已清空，正在拉取新环境资源。" : "正在同步当前环境下的资源列表。" }}
      </div>
    </div>
    <div v-else class="resource-frame-body">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.resource-frame {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.resource-frame-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
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
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.75rem;
  margin: 0.75rem 1rem 1.25rem;
  padding: 2rem 1.5rem;
  text-align: center;
  color: var(--kf-text-secondary, #64748b);
  font-size: 0.875rem;
  background: var(--wb-panel, #ffffff);
  border: 1px solid var(--kf-border, rgba(148, 163, 184, 0.26));
  border-radius: 14px;
  box-shadow: var(--kf-shadow-sm, 0 10px 24px rgba(15, 23, 42, 0.08));
}
.loading-state-spin {
  margin-bottom: 0.15rem;
}
.loading-state-title {
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--kf-text-primary, #334155);
}
.loading-state-detail {
  margin-top: 0;
  font-size: 0.8125rem;
  color: var(--kf-text-secondary, #64748b);
  max-width: 22rem;
  line-height: 1.45;
}
</style>
