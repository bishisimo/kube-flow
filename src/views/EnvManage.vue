<script setup lang="ts">
/**
 * 环境管理视图：负责列表布局与弹窗编排；列表/筛选/增删由 useEnvManage 驱动，
 * 各对话框的表单逻辑下沉到对应子组件，保持视图层只做组合。
 */
import { ref } from "vue";
import { NButton, NEmpty, NSpace, NSpin } from "naive-ui";
import { kfSpace } from "../kf";
import type { Environment } from "../api/env";
import { useEnvManage } from "../features/env/useEnvManage";
import EnvCard from "../components/env/EnvCard.vue";
import EnvFilterBar from "../components/env/EnvFilterBar.vue";
import EnvCreateDialog from "../components/env/EnvCreateDialog.vue";
import EnvEditDrawer from "../components/env/EnvEditDrawer.vue";

defineOptions({ name: "EnvManage" });

const emit = defineEmits<{
  (e: "use-env"): void;
  (e: "open-ssh-settings"): void;
}>();

const manage = useEnvManage();
const {
  environments,
  listLoading,
  allTags,
  selectedFilterTags,
  searchQuery,
  hasActiveFilter,
  filteredEnvironments,
  loadList,
  getTunnelForEnv,
  currentContextLabel,
  strategyEnabled,
  toggleFilterTag,
  clearFilter,
  switchContext,
  removeEnv,
  useEnvAndEmit,
  openEnvTerminal,
} = manage;

function onSearchQueryUpdate(value: string) {
  searchQuery.value = value ?? "";
}

const showCreate = ref(false);
const showEdit = ref(false);
const editingEnv = ref<Environment | null>(null);

function openCreate() {
  showCreate.value = true;
}

function onEditEnv(env: Environment) {
  editingEnv.value = env;
  showEdit.value = true;
}

async function onCreated() {
  await loadList();
}

async function onSaved() {
  await loadList();
}

async function onRemoved() {
  if (!editingEnv.value) return;
  const id = editingEnv.value.id;
  showEdit.value = false;
  editingEnv.value = null;
  await removeEnv(id);
}

async function onContextSwitch(env: Environment, ctx: string) {
  await switchContext(env, ctx);
}
</script>

<template>
  <div class="env-manage">
    <header class="header">
      <NSpace v-bind="kfSpace.pageTitle" class="header-row">
        <h1 class="header-title">环境管理</h1>
        <NButton type="primary" @click="openCreate">+ 新建环境</NButton>
      </NSpace>
    </header>

    <EnvFilterBar
      :query="searchQuery"
      :tags="allTags"
      :selected="selectedFilterTags"
      @update:query="onSearchQueryUpdate"
      @toggle="toggleFilterTag"
      @clear="clearFilter"
    />
    <p v-if="!listLoading && hasActiveFilter" class="filter-summary">
      显示 {{ filteredEnvironments.length }} / {{ environments.length }} 个环境
    </p>

    <div class="body">
      <NSpace v-if="listLoading" v-bind="kfSpace.centered" class="state-loading">
        <NSpin size="small" />
        <span>加载中…</span>
      </NSpace>

      <div v-else-if="filteredEnvironments.length" class="card-grid">
        <EnvCard
          v-for="env in filteredEnvironments"
          :key="env.id"
          :env="env"
          :tunnel="getTunnelForEnv(env)"
          :current-context-label="currentContextLabel(env)"
          :strategy-enabled="strategyEnabled(env.id)"
          @edit="onEditEnv"
          @use="(e) => useEnvAndEmit(e, () => emit('use-env'))"
          @terminal="openEnvTerminal"
        />
      </div>

      <NEmpty
        v-else
        class="state-empty"
        :description="hasActiveFilter ? '无匹配环境' : '暂无环境'"
      >
        <template #extra>
          <p class="empty-desc">
            {{
              hasActiveFilter
                ? "尝试调整搜索词、清除标签筛选或新建环境。"
                : "点击「新建环境」添加本地 kubeconfig 或 SSH 隧道连接。"
            }}
          </p>
          <NSpace v-bind="kfSpace.centeredActions" class="empty-actions">
            <NButton v-if="hasActiveFilter" @click="clearFilter">清除筛选</NButton>
            <NButton type="primary" @click="openCreate">新建环境</NButton>
          </NSpace>
        </template>
      </NEmpty>
    </div>

    <EnvCreateDialog
      v-model:visible="showCreate"
      @created="onCreated"
      @open-ssh-settings="emit('open-ssh-settings')"
    />
    <EnvEditDrawer
      v-model:visible="showEdit"
      :env="editingEnv"
      @saved="onSaved"
      @removed="onRemoved"
      @context-switch="onContextSwitch"
    />
  </div>
</template>

<style scoped>
.env-manage {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  padding: 1.5rem 2rem;
  background: linear-gradient(180deg, var(--kf-bg-soft) 0%, var(--kf-bg-elevated) 100%);
}
.header {
  margin-bottom: 1.5rem;
  flex-shrink: 0;
}
.header-row {
  width: 100%;
}
.header-title {
  font-size: 1.375rem;
  font-weight: 600;
  margin: 0;
  letter-spacing: -0.02em;
  min-width: 0;
  flex: 1;
}
.header-row :deep(.n-space-item:first-child) {
  flex: 1;
  min-width: 12rem;
}
.filter-summary {
  margin: -0.35rem 0 0.85rem;
  font-size: 0.75rem;
  color: var(--kf-text-secondary);
}
.body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
}
.state-loading {
  width: 100%;
  padding: 3rem 0;
  color: var(--kf-text-secondary);
  font-size: 0.875rem;
}
.state-empty {
  padding: 3rem 1.5rem;
}
.empty-desc {
  margin: 0 0 0.85rem;
  text-align: center;
  max-width: 320px;
  color: var(--kf-text-secondary);
  line-height: 1.5;
}
.empty-actions {
  width: 100%;
}
.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 1.25rem;
}
</style>
