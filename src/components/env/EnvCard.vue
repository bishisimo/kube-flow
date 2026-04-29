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
    ? { color: "#ffedd5", textColor: "#c2410c" }
    : { color: "#dbeafe", textColor: "#1d4ed8" }
);

const userTagColor = { color: "#dcfce7", textColor: "#166534" } as const;
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
  border: 1px solid #e2e8f0;
  border-radius: 12px;
  padding: 1.25rem;
  background: #fff;
  cursor: pointer;
  transition: box-shadow 0.2s, border-color 0.2s, transform 0.15s;
}
.env-card:hover {
  border-color: #c7d2fe;
  box-shadow: 0 4px 12px rgba(37, 99, 235, 0.08);
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
  color: #1e293b;
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
  color: #64748b;
  line-height: 1.45;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.card-meta.count {
  font-size: 0.75rem;
  color: #94a3b8;
}
.card-meta.strategy.enabled {
  color: #166534;
}
.card-actions {
  margin-top: auto;
  padding-top: 1rem;
  border-top: 1px solid #f1f5f9;
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
  --n-color: #eef4ff;
  --n-color-hover: #dbeafe;
  --n-color-pressed: #dbeafe;
  --n-color-focus: #dbeafe;
  --n-text-color: #1d4ed8;
  --n-text-color-hover: #1e40af;
  --n-text-color-pressed: #1e40af;
  --n-text-color-focus: #1e40af;
  --n-border: 1px solid #c7d7fe;
  --n-border-hover: 1px solid #93c5fd;
  --n-border-pressed: 1px solid #93c5fd;
  --n-border-focus: 1px solid #93c5fd;
}
.act-strategy {
  --n-color: #f8fafc;
  --n-color-hover: #eef2ff;
  --n-color-pressed: #e0e7ff;
  --n-color-focus: #eef2ff;
  --n-text-color: #334155;
  --n-text-color-hover: #4338ca;
  --n-text-color-pressed: #4338ca;
  --n-text-color-focus: #4338ca;
  --n-border: 1px solid #e2e8f0;
  --n-border-hover: 1px solid #c7d2fe;
  --n-border-pressed: 1px solid #c7d2fe;
  --n-border-focus: 1px solid #c7d2fe;
}
</style>
