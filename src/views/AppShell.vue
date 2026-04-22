<script setup lang="ts">
import {
  defineAsyncComponent,
  h,
  ref,
  computed,
  onMounted,
  watch,
  markRaw,
  type Component,
  type VNodeChild,
} from "vue";
import { NIcon, NMenu } from "naive-ui";
import type { MenuOption } from "naive-ui";
import { useEnvStore } from "../stores/env";
import { useShellStore } from "../stores/shell";
import { useLogCenterStore } from "../stores/logCenter";
import { useOrchestratorStore } from "../stores/orchestrator";
import EnvManage from "./EnvManage.vue";

/** 非首屏视图懒加载：减少启动 JS parse/exec，EnvManage 作为默认 tab 保持同步导入，避免启动瞬间的白屏。 */
const Main = defineAsyncComponent(() => import("./Main.vue"));
const PodShellView = defineAsyncComponent(() => import("./PodShellView.vue"));
const Settings = defineAsyncComponent(() => import("./Settings.vue"));
const LogCenterView = defineAsyncComponent(() => import("./LogCenterView.vue"));
const ResourceOrchestratorView = defineAsyncComponent(() => import("./ResourceOrchestratorView.vue"));

/** 顶部导航 tab 的内部标识。`shell` 指终端中心（PodShellView），与 stores/shell.ts 的终端会话语义一致。 */
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

const canAccessMain = computed(() => openedEnvs.value.length > 0);
const canAccessShell = computed(() => environments.value.length > 0);
const canAccessOrchestrator = computed(() => environments.value.length > 0);
const canAccessLogCenter = computed(() => environments.value.length > 0);

watch(switchToShellRequested, () => {
  if (canAccessShell.value) setTab("shell");
});
watch(switchToOrchestratorRequested, () => {
  if (canAccessOrchestrator.value) setTab("orchestrator");
});
watch(switchToLogCenterRequested, () => {
  if (canAccessLogCenter.value) setTab("logCenter");
});

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

/** 构造一个返回 NIcon(内联 SVG) 的渲染函数，供 NMenu 的 icon 字段使用。 */
function svgIcon(children: VNodeChild): () => VNodeChild {
  return () =>
    h(
      NIcon,
      { size: 18 },
      {
        default: () =>
          h(
            "svg",
            {
              viewBox: "0 0 24 24",
              fill: "none",
              stroke: "currentColor",
              "stroke-width": "1.6",
              "stroke-linecap": "round",
              "stroke-linejoin": "round",
            },
            children as never,
          ),
      },
    );
}

const iconEnv = svgIcon([
  h("rect", { x: 3, y: 4, width: 18, height: 5, rx: 1 }),
  h("rect", { x: 3, y: 10, width: 18, height: 5, rx: 1 }),
  h("rect", { x: 3, y: 16, width: 18, height: 5, rx: 1 }),
  h("circle", { cx: 6.5, cy: 6.5, r: 0.7, fill: "currentColor", stroke: "none" }),
  h("circle", { cx: 6.5, cy: 12.5, r: 0.7, fill: "currentColor", stroke: "none" }),
  h("circle", { cx: 6.5, cy: 18.5, r: 0.7, fill: "currentColor", stroke: "none" }),
]);
const iconMain = svgIcon([
  h("rect", { x: 3, y: 4, width: 18, height: 16, rx: 2 }),
  h("line", { x1: 3, y1: 9, x2: 21, y2: 9 }),
  h("line", { x1: 10, y1: 9, x2: 10, y2: 20 }),
]);
const iconShell = svgIcon([
  h("rect", { x: 3, y: 5, width: 18, height: 14, rx: 2 }),
  h("polyline", { points: "7,10 10,12 7,14" }),
  h("line", { x1: 12, y1: 15, x2: 17, y2: 15 }),
]);
const iconLog = svgIcon([
  h("rect", { x: 5, y: 3, width: 14, height: 18, rx: 2 }),
  h("line", { x1: 8, y1: 8, x2: 16, y2: 8 }),
  h("line", { x1: 8, y1: 12, x2: 16, y2: 12 }),
  h("line", { x1: 8, y1: 16, x2: 13, y2: 16 }),
]);
const iconOrchestrator = svgIcon([
  h("circle", { cx: 12, cy: 5, r: 2 }),
  h("circle", { cx: 5, cy: 18, r: 2 }),
  h("circle", { cx: 19, cy: 18, r: 2 }),
  h("line", { x1: 11, y1: 6.5, x2: 6, y2: 16 }),
  h("line", { x1: 13, y1: 6.5, x2: 18, y2: 16 }),
]);
const iconSettings = svgIcon([
  h("circle", { cx: 12, cy: 12, r: 3 }),
  h("path", {
    d: "M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41",
  }),
]);

const menuOptions = computed<MenuOption[]>(() => [
  { key: "env", label: "环境管理", icon: iconEnv },
  { key: "main", label: "工作台", icon: iconMain, disabled: !canAccessMain.value },
  { key: "shell", label: "终端中心", icon: iconShell, disabled: !canAccessShell.value },
  { key: "logCenter", label: "日志中心", icon: iconLog, disabled: !canAccessLogCenter.value },
  { key: "orchestrator", label: "编排中心", icon: iconOrchestrator, disabled: !canAccessOrchestrator.value },
  { key: "settings", label: "设置", icon: iconSettings },
]);

function onMenuUpdate(key: string) {
  setTab(key as TabId);
}
</script>

<template>
  <div class="app-shell">
    <header class="app-shell-topbar">
      <div class="app-shell-brand">
        <span class="app-shell-brand-dot" aria-hidden="true" />
        <span class="app-shell-brand-name">Kube-Flow</span>
      </div>
      <NMenu
        mode="horizontal"
        responsive
        :value="currentTab"
        :options="menuOptions"
        class="app-shell-nav"
        @update:value="onMenuUpdate"
      />
    </header>
    <main class="app-shell-content">
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
.app-shell {
  display: flex;
  flex-direction: column;
  height: 100vh;
  min-height: 0;
  overflow: hidden;
}
.app-shell-topbar {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0 0.75rem 0 1rem;
  background: #ffffff;
  border-bottom: 1px solid var(--kf-border, #e2e8f0);
  flex-shrink: 0;
}
.app-shell-brand {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  padding: 0.45rem 0.25rem 0.45rem 0;
  color: var(--kf-text-primary, #0f172a);
  font-weight: 700;
  font-size: 0.92rem;
  letter-spacing: -0.01em;
  white-space: nowrap;
  flex-shrink: 0;
}
.app-shell-brand-dot {
  width: 10px;
  height: 10px;
  border-radius: 999px;
  background: linear-gradient(135deg, #3b82f6, #2563eb);
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
}
.app-shell-nav {
  flex: 1;
  min-width: 0;
  background: transparent;
}
.app-shell-nav :deep(.n-menu-item-content) {
  padding: 0 0.9rem;
}
.app-shell-nav :deep(.n-menu-item-content .n-icon) {
  margin-right: 0.4rem;
}
.app-shell-content {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.app-shell-content > * {
  flex: 1;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  display: flex;
}
</style>
