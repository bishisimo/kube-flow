<script setup lang="ts">
import { ref, computed, onMounted, watch, markRaw, type Component } from "vue";
import { useEnvStore } from "../stores/env";
import { useShellStore } from "../stores/shell";
import { useLogCenterStore } from "../stores/logCenter";
import { useOrchestratorStore } from "../stores/orchestrator";
import EnvManage from "./EnvManage.vue";
import Main from "./Main.vue";
import PodShellView from "./PodShellView.vue";
import Settings from "./Settings.vue";
import LogCenterView from "./LogCenterView.vue";
import ResourceOrchestratorView from "./ResourceOrchestratorView.vue";

type TabId = "env" | "main" | "orchestrator" | "shell" | "settings" | "logCenter";

const VIEW_MAP: Record<TabId, Component> = {
  env: markRaw(EnvManage),
  main: markRaw(Main),
  orchestrator: markRaw(ResourceOrchestratorView),
  shell: markRaw(PodShellView),
  logCenter: markRaw(LogCenterView),
  settings: markRaw(Settings),
};

const KEEP_ALIVE_VIEWS = ["EnvManage", "Main", "ResourceOrchestratorView", "PodShellView", "LogCenterView"];

const currentTab = ref<TabId>("env");
const currentView = computed(() => VIEW_MAP[currentTab.value]);
const { environments, openedEnvs, loadEnvironments } = useEnvStore();
const { switchToShellRequested } = useShellStore();
const { switchToLogCenterRequested } = useLogCenterStore();
const { switchToOrchestratorRequested } = useOrchestratorStore();

watch(switchToShellRequested, () => {
  if (canAccessShell.value) setTab("shell");
});
watch(switchToOrchestratorRequested, () => {
  if (canAccessOrchestrator.value) setTab("orchestrator");
});
watch(switchToLogCenterRequested, () => {
  if (canAccessLogCenter.value) setTab("logCenter");
});

const canAccessMain = computed(() => openedEnvs.value.length > 0);
const canAccessShell = computed(() => environments.value.length > 0);
const canAccessOrchestrator = computed(() => environments.value.length > 0);
const canAccessLogCenter = computed(() => environments.value.length > 0);

function setTab(tab: TabId) {
  if (tab === "main" && !canAccessMain.value) return;
  if (tab === "shell" && !canAccessShell.value) return;
  if (tab === "orchestrator" && !canAccessOrchestrator.value) return;
  if (tab === "logCenter" && !canAccessLogCenter.value) return;
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
        :class="{ active: currentTab === 'shell', disabled: !canAccessShell }"
        :title="canAccessShell ? '终端中心' : '请先创建至少一个环境'"
        :disabled="!canAccessShell"
        @click="setTab('shell')"
      >
        终端中心
      </button>
      <button
        type="button"
        class="tab"
        :class="{ active: currentTab === 'logCenter', disabled: !canAccessLogCenter }"
        :title="canAccessLogCenter ? '日志中心' : '请先创建至少一个环境'"
        :disabled="!canAccessLogCenter"
        @click="setTab('logCenter')"
      >
        日志中心
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
        :class="{ active: currentTab === 'settings' }"
        @click="setTab('settings')"
      >
        设置
      </button>
    </header>
    <main class="shell-content">
      <KeepAlive :include="KEEP_ALIVE_VIEWS">
        <component
          :is="currentView"
          :key="currentTab"
          @use-env="onUseEnv"
        />
      </KeepAlive>
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
