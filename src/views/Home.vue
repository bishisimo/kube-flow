<script setup lang="ts">
import { onMounted } from "vue";
import { useRouter } from "vue-router";
import { useEnvStore } from "../stores/env";
import { effectiveContext } from "../api/env";

const router = useRouter();
const { loadEnvironments, environments, openEnv } = useEnvStore();

function currentLabel(e: import("../api/env").Environment) {
  const name = effectiveContext(e);
  if (!name) return "—";
  const ctx = e.contexts.find((c) => c.context_name === name);
  return ctx?.cluster_name ?? name;
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
    <h1>Kube-Flow</h1>
    <p class="sub">Kubernetes 资源管理</p>
    <nav>
      <button class="primary" @click="goEnvManage">环境管理</button>
    </nav>
    <section v-if="environments.length" class="env-list">
      <h2>已配置环境</h2>
      <ul>
        <li v-for="e in environments" :key="e.id" class="env-item">
          <span class="name">{{ e.display_name }}</span>
          <span class="meta">{{ currentLabel(e) }}</span>
          <button @click="useEnv(e.id)">使用</button>
        </li>
      </ul>
    </section>
    <p v-else class="hint">暂无环境，请先在「环境管理」中新建环境。</p>
  </div>
</template>

<style scoped>
.home {
  padding: 2rem;
  max-width: 640px;
  margin: 0 auto;
}
h1 {
  font-size: 1.75rem;
  margin-bottom: 0.25rem;
}
.sub {
  color: var(--color-text-muted, #666);
  margin-bottom: 1.5rem;
}
nav {
  margin-bottom: 2rem;
}
.primary {
  padding: 0.5rem 1rem;
  background: #396cd8;
  color: #fff;
  border: none;
  border-radius: 6px;
  cursor: pointer;
}
.primary:hover {
  background: #2d5ac2;
}
.env-list h2 {
  font-size: 1rem;
  margin-bottom: 0.75rem;
}
.env-list ul {
  list-style: none;
  padding: 0;
  margin: 0;
  max-height: 60vh;
  overflow-y: auto;
  overflow-x: hidden;
}
.env-item {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.5rem 0;
  border-bottom: 1px solid #eee;
}
.env-item .name {
  font-weight: 500;
  min-width: 140px;
}
.env-item .meta {
  color: #666;
  font-size: 0.9rem;
  flex: 1;
}
.env-item button {
  padding: 0.35rem 0.75rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  cursor: pointer;
  background: #fff;
}
.env-item button:hover {
  background: #f5f5f5;
}
.hint {
  color: #666;
}
</style>
