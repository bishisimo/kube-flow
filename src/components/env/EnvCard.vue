<script setup lang="ts">
/**
 * 环境卡片：在环境管理列表中展示单个环境的摘要信息与操作入口。
 *
 * 卡片主体点击 = 编辑；底部按钮触发使用/终端/策略，这些按钮内部已阻止冒泡。
 */
import { computed } from "vue";
import { NButton, NTag } from "naive-ui";
import type { Environment, SshTunnel } from "../../api/env";

const props = defineProps<{
  env: Environment;
  tunnel?: SshTunnel;
  currentContextLabel: string;
  strategyEnabled: boolean;
}>();

const emit = defineEmits<{
  (e: "edit", env: Environment): void;
  (e: "use", env: Environment): void;
  (e: "terminal", env: Environment): void;
  (e: "strategy", env: Environment): void;
}>();

const isSsh = computed(() => props.env.source === "ssh_tunnel");
const sourceLabel = computed(() => (isSsh.value ? "SSH" : "本地"));

const sourceTagColor = computed(() =>
  isSsh.value
    ? {
        color: "color-mix(in srgb, var(--kf-warning) 18%, var(--kf-surface-strong))",
        textColor: "color-mix(in srgb, var(--kf-warning) 78%, var(--kf-text-primary))",
      }
    : {
        color: "color-mix(in srgb, var(--kf-info) 18%, var(--kf-surface-strong))",
        textColor: "color-mix(in srgb, var(--kf-info) 80%, var(--kf-text-primary))",
      }
);

const userTagColor = {
  color: "color-mix(in srgb, var(--kf-success) 18%, var(--kf-surface-strong))",
  textColor: "color-mix(in srgb, var(--kf-success) 78%, var(--kf-text-primary))",
} as const;
</script>

<template>
  <article class="env-card" @click="emit('edit', env)">
    <header class="card-header">
      <NTag
        :color="sourceTagColor"
        :bordered="false"
        size="small"
        round
        class="source-chip"
      >
        {{ sourceLabel }}
      </NTag>
      <h3 class="card-title">{{ env.display_name }}</h3>
    </header>

    <div class="card-tags" :class="{ empty: !(env.tags ?? []).length }">
      <NTag
        v-for="t in env.tags"
        :key="t"
        :color="userTagColor"
        :bordered="false"
        size="small"
        round
      >
        {{ t }}
      </NTag>
    </div>

    <div class="card-meta-group">
      <template v-if="!isSsh">
        <p class="card-meta current-ctx">{{ currentContextLabel }}</p>
        <p class="card-meta count">{{ env.contexts.length }} 个 context</p>
      </template>
      <template v-else>
        <p class="card-meta">Host: {{ tunnel?.ssh_host ?? '—' }}</p>
        <p class="card-meta">远程 kubeconfig: {{ tunnel?.remote_kubeconfig_path ?? '—' }}</p>
      </template>
      <p class="card-meta strategy" :class="{ enabled: strategyEnabled }">
        节点终端策略：{{ strategyEnabled ? "已启用" : "未配置" }}
      </p>
    </div>

    <div class="card-actions" @click.stop>
      <NButton type="primary" size="small" class="act-use" @click="emit('use', env)">
        使用
      </NButton>
      <NButton size="small" class="act-terminal" @click="emit('terminal', env)">
        终端
      </NButton>
      <NButton size="small" class="act-strategy" @click="emit('strategy', env)">
        终端策略
      </NButton>
    </div>
  </article>
</template>

<style scoped>
.env-card {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--kf-border);
  border-radius: 12px;
  padding: 1.25rem;
  background: var(--kf-surface-strong);
  cursor: pointer;
  transition: box-shadow 0.2s, border-color 0.2s, transform 0.15s;
}
.env-card:hover {
  border-color: color-mix(in srgb, var(--kf-primary) 42%, var(--kf-border));
  box-shadow: 0 4px 12px color-mix(in srgb, var(--kf-primary) 20%, transparent);
  transform: translateY(-1px);
}
.card-header {
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
  margin-bottom: 0.75rem;
  min-height: 2.8rem;
}
.source-chip {
  text-transform: uppercase;
  letter-spacing: 0.03em;
  font-weight: 600;
  flex-shrink: 0;
}
.card-title {
  margin: 0;
  font-size: 1rem;
  font-weight: 600;
  color: var(--kf-text-primary);
  letter-spacing: -0.01em;
  line-height: 1.4;
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  white-space: normal;
  word-break: break-word;
}
.card-tags {
  display: flex;
  flex-wrap: wrap;
  align-content: flex-start;
  gap: 0.35rem;
  margin-bottom: 0.5rem;
  min-height: 1.65rem;
}
.card-tags.empty {
  visibility: hidden;
}
.card-meta-group {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  min-height: 3.35rem;
}
.card-meta {
  margin: 0;
  font-size: 0.8125rem;
  color: var(--kf-text-secondary);
  line-height: 1.45;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.card-meta.count {
  font-size: 0.75rem;
  color: var(--kf-text-muted);
}
.card-meta.strategy.enabled {
  color: var(--kf-success);
}
.card-actions {
  margin-top: auto;
  padding-top: 1rem;
  border-top: 1px solid var(--kf-border);
  display: flex;
  gap: 0.6rem;
  flex-wrap: wrap;
}

/* ---- Naive UI 按钮主题覆盖：对齐旧版蓝 / 淡蓝 / 灰白三态视觉 ---- */
.act-use {
  --n-color: #2563eb;
  --n-color-hover: #1d4ed8;
  --n-color-pressed: #1e40af;
  --n-color-focus: #1d4ed8;
  --n-text-color: #ffffff;
  --n-text-color-hover: #ffffff;
  --n-text-color-pressed: #ffffff;
  --n-text-color-focus: #ffffff;
  --n-border: 1px solid transparent;
  --n-border-hover: 1px solid transparent;
  --n-border-pressed: 1px solid transparent;
  --n-border-focus: 1px solid transparent;
}
.act-terminal {
  --n-color: color-mix(in srgb, var(--kf-info) 16%, var(--kf-surface-strong));
  --n-color-hover: color-mix(in srgb, var(--kf-info) 24%, var(--kf-surface-strong));
  --n-color-pressed: color-mix(in srgb, var(--kf-info) 26%, var(--kf-surface-strong));
  --n-color-focus: color-mix(in srgb, var(--kf-info) 24%, var(--kf-surface-strong));
  --n-text-color: color-mix(in srgb, var(--kf-info) 82%, var(--kf-text-primary));
  --n-text-color-hover: color-mix(in srgb, var(--kf-info) 88%, var(--kf-text-primary));
  --n-text-color-pressed: color-mix(in srgb, var(--kf-info) 88%, var(--kf-text-primary));
  --n-text-color-focus: color-mix(in srgb, var(--kf-info) 88%, var(--kf-text-primary));
  --n-border: 1px solid color-mix(in srgb, var(--kf-info) 40%, var(--kf-border));
  --n-border-hover: 1px solid color-mix(in srgb, var(--kf-info) 52%, var(--kf-border));
  --n-border-pressed: 1px solid color-mix(in srgb, var(--kf-info) 52%, var(--kf-border));
  --n-border-focus: 1px solid color-mix(in srgb, var(--kf-info) 52%, var(--kf-border));
}
.act-strategy {
  --n-color: color-mix(in srgb, var(--kf-bg-soft) 84%, var(--kf-surface-strong));
  --n-color-hover: color-mix(in srgb, var(--kf-primary) 14%, var(--kf-surface-strong));
  --n-color-pressed: color-mix(in srgb, var(--kf-primary) 18%, var(--kf-surface-strong));
  --n-color-focus: color-mix(in srgb, var(--kf-primary) 14%, var(--kf-surface-strong));
  --n-text-color: var(--kf-text-primary);
  --n-text-color-hover: color-mix(in srgb, var(--kf-primary) 82%, var(--kf-text-primary));
  --n-text-color-pressed: color-mix(in srgb, var(--kf-primary) 82%, var(--kf-text-primary));
  --n-text-color-focus: color-mix(in srgb, var(--kf-primary) 82%, var(--kf-text-primary));
  --n-border: 1px solid var(--kf-border);
  --n-border-hover: 1px solid color-mix(in srgb, var(--kf-primary) 46%, var(--kf-border));
  --n-border-pressed: 1px solid color-mix(in srgb, var(--kf-primary) 46%, var(--kf-border));
  --n-border-focus: 1px solid color-mix(in srgb, var(--kf-primary) 46%, var(--kf-border));
}
</style>
