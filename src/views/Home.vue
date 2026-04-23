<script setup lang="ts">
import { onMounted } from "vue";
import { useRouter } from "vue-router";
import { NButton, NCard, NEmpty, NList, NListItem, NScrollbar, NSpace, NTag } from "naive-ui";
import { kfSpace } from "../kf";
import { useEnvStore } from "../stores/env";
import { effectiveContext, type Environment } from "../api/env";

const router = useRouter();
const { loadEnvironments, environments, openEnv } = useEnvStore();

function currentLabel(env: Environment): string {
  const name = effectiveContext(env);
  if (!name) return "—";
  const ctx = env.contexts.find((c) => c.context_name === name);
  return ctx?.cluster_name ?? name;
}

function sourceLabel(env: Environment): string {
  return env.source === "ssh_tunnel" ? "SSH" : "本地";
}

function sourceType(env: Environment): "warning" | "info" {
  return env.source === "ssh_tunnel" ? "warning" : "info";
}

onMounted(async () => {
  await loadEnvironments();
});

function goEnvManage() {
  router.push("/env");
}

function useEnv(id: string) {
  openEnv(id);
  router.push("/main");
}
</script>

<template>
  <div class="home">
    <header class="home-header">
      <h1>Kube-Flow</h1>
      <p class="sub">Kubernetes 资源管理</p>
    </header>

    <nav class="home-nav">
      <NButton type="primary" size="medium" @click="goEnvManage">环境管理</NButton>
    </nav>

    <NCard title="已配置环境" size="small" class="env-card" :bordered="false">
      <NScrollbar style="max-height: 60vh;">
        <NList v-if="environments.length" hoverable>
          <NListItem v-for="env in environments" :key="env.id">
            <template #prefix>
              <NTag size="small" :type="sourceType(env)" round :bordered="false">
                {{ sourceLabel(env) }}
              </NTag>
            </template>
            <NSpace v-bind="kfSpace.homeEnvRow" class="env-row">
              <span class="env-name">{{ env.display_name }}</span>
              <span class="env-meta">{{ currentLabel(env) }}</span>
            </NSpace>
            <template #suffix>
              <NButton size="small" type="primary" ghost @click="useEnv(env.id)">使用</NButton>
            </template>
          </NListItem>
        </NList>
        <NEmpty v-else description="暂无环境，请先在「环境管理」中新建环境。" />
      </NScrollbar>
    </NCard>
  </div>
</template>

<style scoped>
.home {
  padding: 2.25rem;
  max-width: 760px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}
.home-header h1 {
  font-size: 1.75rem;
  font-weight: 650;
  margin: 0 0 0.25rem;
  letter-spacing: -0.02em;
}
.sub {
  margin: 0;
  color: #64748b;
}
.home-nav {
  display: flex;
  gap: 0.5rem;
}
.env-card {
  background: #fff;
  box-shadow: 0 1px 2px rgba(15, 23, 42, 0.04);
  border: 1px solid var(--kf-border, #e2e8f0);
  border-radius: 12px;
}
.env-row {
  min-width: 0;
  flex: 1;
}
.env-name {
  font-weight: 600;
  color: #0f172a;
  font-size: 0.9rem;
}
.env-meta {
  color: #64748b;
  font-size: 0.8125rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
