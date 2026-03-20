<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { useEnvStore } from "../stores/env";
import { useShellStore } from "../stores/shell";
import { useOrchestratorStore } from "../stores/orchestrator";
import EnvManage from "./EnvManage.vue";
import Main from "./Main.vue";
import PodShellView from "./PodShellView.vue";
import Settings from "./Settings.vue";
import LogView from "./LogView.vue";
import ResourceOrchestratorView from "./ResourceOrchestratorView.vue";

type TabId = "env" | "main" | "orchestrator" | "shell" | "settings" | "log";

const currentTab = ref<TabId>("env");
const { environments, openedEnvs, loadEnvironments } = useEnvStore();
const { switchToShellRequested } = useShellStore();
const { switchToOrchestratorRequested } = useOrchestratorStore();

watch(switchToShellRequested, () => {
  if (canAccessMain.value) setTab("shell");
});
watch(switchToOrchestratorRequested, () => {
  if (canAccessOrchestrator.value) setTab("orchestrator");
});

const canAccessMain = computed(() => openedEnvs.value.length > 0);
const canAccessShell = computed(() => openedEnvs.value.length > 0);
const canAccessOrchestrator = computed(() => environments.value.length > 0);

function setTab(tab: TabId) {
  if ((tab === "main" || tab === "shell") && !canAccessMain.value) return;
  if (tab === "orchestrator" && !canAccessOrchestrator.value) return;
  currentTab.value = tab;
}

function onUseEnv() {
  currentTab.value = "main";
}

onMounted(async () => {
  await loadEnvironments();
});
</script>

<template>
  <div class="shell">
    <header class="top-tabs">
      <button
        type="button"
        class="tab"
        :class="{ active: currentTab === 'env' }"
        @click="setTab('env')"
      >
        环境管理
      </button>
      <button
        type="button"
        class="tab"
        :class="{ active: currentTab === 'main', disabled: !canAccessMain }"
        :title="canAccessMain ? '工作台' : '请先在环境管理中打开至少一个环境'"
        :disabled="!canAccessMain"
        @click="setTab('main')"
      >
        工作台
      </button>
      <button
        type="button"
        class="tab"
        :class="{ active: currentTab === 'orchestrator', disabled: !canAccessOrchestrator }"
        :title="canAccessOrchestrator ? '编排中心' : '请先创建至少一个环境'"
        :disabled="!canAccessOrchestrator"
        @click="setTab('orchestrator')"
      >
        编排中心
      </button>
      <button
        type="button"
        class="tab"
        :class="{ active: currentTab === 'shell', disabled: !canAccessShell }"
        :title="canAccessShell ? 'Pod Shell' : '请先在环境管理中打开至少一个环境'"
        :disabled="!canAccessShell"
        @click="setTab('shell')"
      >
        Pod Shell
      </button>
      <button
        type="button"
        class="tab"
        :class="{ active: currentTab === 'settings' }"
        @click="setTab('settings')"
      >
        设置
      </button>
      <button
        type="button"
        class="tab"
        :class="{ active: currentTab === 'log' }"
        @click="setTab('log')"
      >
        日志
      </button>
    </header>
    <main class="shell-content">
      <EnvManage v-show="currentTab === 'env'" @use-env="onUseEnv" />
      <Main v-show="currentTab === 'main'" />
      <ResourceOrchestratorView v-show="currentTab === 'orchestrator'" />
      <PodShellView v-show="currentTab === 'shell'" />
      <Settings v-show="currentTab === 'settings'" />
      <LogView v-show="currentTab === 'log'" :visible="currentTab === 'log'" />
    </main>
  </div>
</template>

<style scoped>
.shell {
  display: flex;
  flex-direction: column;
  height: 100vh;
  min-height: 0;
  overflow: hidden;
}
.top-tabs {
  display: flex;
  gap: 0;
  padding: 0 0.5rem;
  background: #fff;
  border-bottom: 1px solid #e2e8f0;
  flex-shrink: 0;
}
.tab {
  padding: 0.75rem 1.25rem;
  border: none;
  background: transparent;
  font-size: 0.875rem;
  font-weight: 500;
  color: #64748b;
  cursor: pointer;
  position: relative;
  transition: color 0.15s;
}
.tab:hover:not(.disabled) {
  color: #1e293b;
}
.tab.active {
  color: #2563eb;
}
.tab.active::after {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  bottom: -1px;
  height: 2px;
  background: #2563eb;
}
.tab.disabled {
  cursor: not-allowed;
  opacity: 0.6;
}
.shell-content {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.shell-content > * {
  flex: 1;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  display: flex;
}
</style>
