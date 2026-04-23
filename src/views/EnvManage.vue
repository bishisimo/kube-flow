<script setup lang="ts">
/**
 * 环境管理视图：负责列表布局与弹窗编排；列表/筛选/增删由 useEnvManage 驱动，
 * 各对话框的表单逻辑下沉到对应子组件，保持视图层只做组合。
 */
import { ref } from "vue";
import { NButton, NEmpty, NSpin } from "naive-ui";
import type { Environment } from "../api/env";
import { useEnvManage } from "../features/env/useEnvManage";
import EnvCard from "../components/env/EnvCard.vue";
import EnvFilterBar from "../components/env/EnvFilterBar.vue";
import EnvCreateDialog from "../components/env/EnvCreateDialog.vue";
import EnvEditDialog from "../components/env/EnvEditDialog.vue";
import NodeTerminalStrategyDialog from "../components/env/NodeTerminalStrategyDialog.vue";

defineOptions({ name: "EnvManage" });

const emit = defineEmits<{ (e: "use-env"): void }>();

const {
  listLoading,
  allTags,
  selectedFilterTags,
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
} = useEnvManage();

const showCreate = ref(false);
const showEdit = ref(false);
const showStrategy = ref(false);
const editingEnv = ref<Environment | null>(null);
const strategyEnv = ref<Environment | null>(null);

function openCreate() {
  showCreate.value = true;
}

function onEditEnv(env: Environment) {
  editingEnv.value = env;
  showEdit.value = true;
}

function onStrategyEnv(env: Environment) {
  strategyEnv.value = env;
  showStrategy.value = true;
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
      <h1>环境管理</h1>
      <NButton type="primary" @click="openCreate">+ 新建环境</NButton>
    </header>

    <EnvFilterBar
      v-if="!listLoading"
      :tags="allTags"
      :selected="selectedFilterTags"
      @toggle="toggleFilterTag"
      @clear="clearFilter"
    />

    <div class="body">
      <div v-if="listLoading" class="state-loading">
        <NSpin size="small" />
        <span>加载中…</span>
      </div>

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
          @strategy="onStrategyEnv"
        />
      </div>

      <NEmpty
        v-else
        class="state-empty"
        :description="selectedFilterTags.size ? '无匹配环境' : '暂无环境'"
      >
        <template #extra>
          <p class="empty-desc">
            {{
              selectedFilterTags.size
                ? "尝试清除标签筛选或新建环境。"
                : "点击「新建环境」添加本地 kubeconfig 或 SSH 隧道连接。"
            }}
          </p>
          <div class="empty-actions">
            <NButton v-if="selectedFilterTags.size" @click="clearFilter">清除筛选</NButton>
            <NButton type="primary" @click="openCreate">新建环境</NButton>
          </div>
        </template>
      </NEmpty>
    </div>

    <EnvCreateDialog v-model:visible="showCreate" @created="onCreated" />
    <EnvEditDialog
      v-model:visible="showEdit"
      :env="editingEnv"
      @saved="onSaved"
      @removed="onRemoved"
      @context-switch="onContextSwitch"
    />
    <NodeTerminalStrategyDialog
      v-model:visible="showStrategy"
      :env="strategyEnv"
      @saved="onSaved"
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
  background: linear-gradient(180deg, #f8fafc 0%, #f1f5f9 100%);
}
.header {
  display: flex;
  align-items: center;
  gap: 1rem;
  margin-bottom: 1.5rem;
  flex-wrap: wrap;
  flex-shrink: 0;
}
.header h1 {
  font-size: 1.375rem;
  font-weight: 600;
  margin: 0;
  flex: 1;
  letter-spacing: -0.02em;
}
.body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
}
.state-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  padding: 3rem 0;
  color: #64748b;
  font-size: 0.875rem;
}
.state-empty {
  padding: 3rem 1.5rem;
}
.empty-desc {
  margin: 0 0 0.85rem;
  text-align: center;
  max-width: 320px;
  color: #64748b;
  line-height: 1.5;
}
.empty-actions {
  display: flex;
  gap: 0.5rem;
  justify-content: center;
}
.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 1.25rem;
}
</style>
