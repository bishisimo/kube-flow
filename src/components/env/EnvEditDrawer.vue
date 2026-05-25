<script setup lang="ts">
/**
 * 编辑环境抽屉：基础配置与节点终端策略分 Tab 展示，替代原独立弹窗。
 */
import { ref, computed, watch, unref, nextTick } from "vue";
import { NButton, NDrawer, NDrawerContent, NPopconfirm, NSpace, NTab, NTabs } from "naive-ui";
import { kfSpace } from "../../kf";
import type { Environment } from "../../api/env";
import { strongholdAdjacentModalTrapFocusEnabled } from "../../stores/strongholdAuth";
import { createStorage } from "../../utils/storage";
import EnvBasicConfigPanel from "./EnvBasicConfigPanel.vue";
import EnvTerminalStrategyPanel from "./EnvTerminalStrategyPanel.vue";
import EnvWorkbenchStatePanel from "./EnvWorkbenchStatePanel.vue";

type EnvEditTab = "basic" | "terminal" | "workbench";

const DRAWER_WIDTH_KEY = "kube-flow:env-edit-drawer-width";
const DRAWER_MIN = 420;
const DRAWER_MAX = 920;
const DRAWER_DEFAULT = 560;

const drawerWidthStorage = createStorage<number>({
  key: DRAWER_WIDTH_KEY,
  version: 1,
  fallback: DRAWER_DEFAULT,
  migrate: (old) => {
    const n = parseInt(String(old), 10);
    return Number.isNaN(n) ? DRAWER_DEFAULT : Math.min(DRAWER_MAX, Math.max(DRAWER_MIN, n));
  },
});

const drawerWidth = ref(
  Math.min(DRAWER_MAX, Math.max(DRAWER_MIN, drawerWidthStorage.read()))
);

function onDrawerWidthUpdate(value: number) {
  const w = Math.min(DRAWER_MAX, Math.max(DRAWER_MIN, Math.round(value)));
  drawerWidth.value = w;
  drawerWidthStorage.write(w);
}

const props = defineProps<{
  visible: boolean;
  env: Environment | null;
}>();

const emit = defineEmits<{
  (e: "update:visible", value: boolean): void;
  (e: "saved"): void;
  (e: "removed"): void;
  (e: "context-switch", env: Environment, contextName: string): void;
}>();

const activeTab = ref<EnvEditTab>("basic");
const basicPanelRef = ref<InstanceType<typeof EnvBasicConfigPanel> | null>(null);
const terminalPanelRef = ref<InstanceType<typeof EnvTerminalStrategyPanel> | null>(null);
const workbenchPanelRef = ref<InstanceType<typeof EnvWorkbenchStatePanel> | null>(null);

const isEditableTab = computed(() => activeTab.value === "basic" || activeTab.value === "terminal");

const drawerTitle = computed(() =>
  props.env ? `编辑环境 · ${props.env.display_name}` : "编辑环境"
);

const saving = computed(
  () =>
    unref(basicPanelRef.value?.loading) ||
    unref(terminalPanelRef.value?.loading)
);

watch(
  () => [props.visible, props.env?.id] as const,
  ([open]) => {
    if (!open) return;
    activeTab.value = "basic";
    nextTick(() => workbenchPanelRef.value?.reload());
  },
  { immediate: true }
);

function close() {
  emit("update:visible", false);
}

function onDrawerShowUpdate(show: boolean) {
  if (!show) close();
}

async function submitCurrentTab() {
  if (!isEditableTab.value) return;
  const panel = activeTab.value === "basic" ? basicPanelRef.value : terminalPanelRef.value;
  if (!panel) return;
  const ok = await panel.submit();
  if (ok) close();
}

function onTabChange(tab: string | number) {
  const next = String(tab) as EnvEditTab;
  activeTab.value = next;
  if (next === "workbench") {
    nextTick(() => workbenchPanelRef.value?.reload());
  }
}

function requestRemove() {
  if (!props.env) return;
  emit("removed");
}

function onContextSwitch(env: Environment, contextName: string) {
  emit("context-switch", env, contextName);
}

function onPanelSaved() {
  emit("saved");
}
</script>

<template>
  <NDrawer
    :show="visible"
    placement="right"
    :width="drawerWidth"
    :min-width="DRAWER_MIN"
    :max-width="DRAWER_MAX"
    :default-width="DRAWER_DEFAULT"
    resizable
    :mask-closable="!saving"
    :close-on-esc="!saving"
    :auto-focus="false"
    :trap-focus="strongholdAdjacentModalTrapFocusEnabled"
    :block-scroll="false"
    @update:show="onDrawerShowUpdate"
    @update:width="onDrawerWidthUpdate"
  >
    <NDrawerContent
      class="env-edit-drawer"
      closable
      :native-scrollbar="true"
      body-class="env-edit-drawer-body"
      body-content-class="env-edit-drawer-body-content"
      body-style="padding: 0; overflow: hidden;"
      body-content-style="display: flex; flex-direction: column; height: 100%; min-height: 0;"
    >
      <template #header>
        <NSpace v-bind="kfSpace.drawerTitle" class="drawer-header">
          <span class="drawer-title">{{ drawerTitle }}</span>
        </NSpace>
      </template>

      <div v-if="env" class="drawer-shell">
        <div class="drawer-tabs">
          <NTabs :value="activeTab" type="line" animated size="small" @update:value="onTabChange">
            <NTab name="basic" tab="基础配置" />
            <NTab name="terminal" tab="终端策略" />
            <NTab name="workbench" tab="工作台" />
          </NTabs>
        </div>

        <div class="drawer-panel">
          <EnvBasicConfigPanel
            v-show="activeTab === 'basic'"
            ref="basicPanelRef"
            :env="env"
            @saved="onPanelSaved"
            @context-switch="onContextSwitch"
          />
          <EnvTerminalStrategyPanel
            v-show="activeTab === 'terminal'"
            ref="terminalPanelRef"
            :env="env"
            @saved="onPanelSaved"
          />
          <EnvWorkbenchStatePanel
            v-show="activeTab === 'workbench'"
            ref="workbenchPanelRef"
            :env="env"
          />
        </div>

        <footer class="drawer-footer">
          <NPopconfirm v-if="activeTab === 'basic'" @positive-click="requestRemove">
            <template #trigger>
              <NButton type="error" ghost :disabled="saving">删除环境</NButton>
            </template>
            删除后该环境将从列表中移除，确认删除？
          </NPopconfirm>
          <div v-else class="footer-spacer" />
          <div class="footer-actions">
            <NButton :disabled="saving" @click="close">{{ isEditableTab ? '取消' : '关闭' }}</NButton>
            <NButton
              v-if="isEditableTab"
              type="primary"
              :loading="saving"
              @click="submitCurrentTab"
            >
              {{ activeTab === 'basic' ? '保存配置' : '保存策略' }}
            </NButton>
          </div>
        </footer>
      </div>
    </NDrawerContent>
  </NDrawer>
</template>

<style scoped>
.drawer-header {
  width: 100%;
}
.drawer-header :deep(.n-space-item:first-child) {
  flex: 1;
  min-width: 0;
}
.drawer-title {
  font-size: 1rem;
  font-weight: 600;
  color: var(--kf-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.drawer-shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
.drawer-tabs {
  flex-shrink: 0;
  padding: 0 1rem;
  border-bottom: 1px solid var(--kf-border);
}
.drawer-panel {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 1rem 1.25rem;
}
.drawer-footer {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.85rem 1.25rem;
  border-top: 1px solid var(--kf-border);
  background: var(--kf-surface-strong);
}
.footer-spacer {
  flex: 1;
}
.footer-actions {
  display: flex;
  gap: 0.5rem;
  margin-left: auto;
}
</style>
