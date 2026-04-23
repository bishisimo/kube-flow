<script setup lang="ts">
import { NAlert, NButton, NSkeleton, NSpace, NSpin } from "naive-ui";
import { kfSpace } from "../../kf";
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
      <NSpace v-bind="kfSpace.alertRow" class="alert-row">
        <div>
          <div class="disconnect-text">连接已断开</div>
          <div class="disconnect-detail">{{ envError }}</div>
        </div>
        <NButton type="warning" secondary size="small" @click="emit('reconnect')">重连</NButton>
      </NSpace>
    </NAlert>
    <div v-else-if="currentId && envState === 'connecting'" class="connection-stepper">
      <NSpace v-bind="kfSpace.inlineStatus" class="stepper-title">
        <NSpin size="small" />
        <span>连接中：{{ progress?.stage_label || "正在获取资源" }}</span>
      </NSpace>
      <div v-if="progress?.detail || listLoading" class="stepper-detail">
        {{ progress?.detail || "请稍候，正在同步当前视图数据…" }}
      </div>
    </div>
    <NAlert v-else-if="listError" type="error" class="state-alert" :show-icon="true">
      <NSpace v-bind="kfSpace.alertRow" class="alert-row">
        <span class="error-message">{{ listError }}</span>
        <NSpace v-bind="kfSpace.inlineStatus" class="alert-actions">
          <NButton v-if="currentId && reconnectOnListError" type="error" secondary size="small" @click="emit('reconnect')">
            重连
          </NButton>
          <NButton size="small" quaternary @click="emit('dismissError')">关闭</NButton>
        </NSpace>
      </NSpace>
    </NAlert>
    <div v-else-if="listLoading" class="loading-state">
      <div class="loading-state-head">
        <NSpace v-bind="kfSpace.inlineStatus" class="loading-state-title">
          <NSpin size="small" stroke="#2563eb" :stroke-width="18" />
          <span>
            {{ envSwitching ? `正在切换到 ${envSwitchingName || "目标环境"}` : "加载中…" }}
          </span>
        </NSpace>
        <div class="loading-state-detail">
          {{ envSwitching ? "旧环境数据已清空，正在拉取新环境资源。" : "正在同步当前环境下的资源列表。" }}
        </div>
      </div>
      <div class="loading-state-skeleton" aria-hidden="true">
        <div class="loading-skeleton-row loading-skeleton-header">
          <NSkeleton text width="20%" :sharp="false" />
          <NSkeleton text width="14%" :sharp="false" />
          <NSkeleton text width="14%" :sharp="false" />
          <NSkeleton text width="12%" :sharp="false" />
          <NSkeleton text width="10%" :sharp="false" />
        </div>
        <div v-for="n in 8" :key="n" class="loading-skeleton-row">
          <NSkeleton text width="24%" :sharp="false" />
          <NSkeleton text width="18%" :sharp="false" />
          <NSkeleton text width="16%" :sharp="false" />
          <NSkeleton text width="14%" :sharp="false" />
          <NSkeleton text width="10%" :sharp="false" />
        </div>
      </div>
    </div>
    <slot v-else />
  </section>
</template>

<style scoped>
.resource-frame {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.state-alert {
  margin: 0.55rem 1rem 0;
  border-radius: 12px;
  border: 1px solid var(--wb-line, rgba(148, 163, 184, 0.22));
}
.alert-row {
  width: 100%;
}
.alert-actions {
  flex-shrink: 0;
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
  margin: 0.55rem 1rem 0;
  padding: 0.75rem 0.9rem;
  background: linear-gradient(180deg, #f0f9ff, #f8fbff);
  color: #075985;
  border: 1px solid #bae6fd;
  border-radius: 12px;
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
.loading-state {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  margin: 0.65rem 1rem 1rem;
  padding: 1rem 1.15rem 1.2rem;
  background: var(--wb-panel-elevated, #ffffff);
  border: 1px solid var(--wb-line, rgba(148, 163, 184, 0.22));
  border-radius: 12px;
  box-shadow: var(--kf-shadow-sm, 0 10px 24px rgba(15, 23, 42, 0.08));
}
.loading-state-head {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  padding-bottom: 0.5rem;
  border-bottom: 1px dashed var(--wb-line, rgba(148, 163, 184, 0.22));
}
.loading-state-title {
  font-size: 0.9rem;
  font-weight: 650;
  color: var(--kf-text-primary, #0f172a);
}
.loading-state-detail {
  font-size: 0.8125rem;
  color: var(--kf-text-secondary, #64748b);
  line-height: 1.5;
}
.loading-state-skeleton {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.25rem 0.1rem 0.25rem;
}
.loading-skeleton-row {
  display: grid;
  grid-template-columns: 24% 18% 16% 14% 10% 1fr;
  gap: 0.8rem;
  align-items: center;
  padding: 0.35rem 0.2rem;
  border-bottom: 1px solid rgba(148, 163, 184, 0.1);
}
.loading-skeleton-row:last-child {
  border-bottom: none;
}
.loading-skeleton-header {
  padding: 0.4rem 0.2rem;
  border-bottom: 1px solid rgba(148, 163, 184, 0.24);
}
.loading-skeleton-header :deep(.n-skeleton) {
  --n-color-start: #e2e8f0;
  --n-color-end: #cbd5e1;
  opacity: 0.75;
}
</style>
