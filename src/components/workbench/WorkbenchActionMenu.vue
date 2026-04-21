<script setup lang="ts">
import { computed, ref } from "vue";
import { NButton } from "naive-ui";
import {
  WORKBENCH_SHELL_WORKLOAD_KINDS,
  WORKBENCH_NODE_TERMINAL_RESOURCE_KINDS,
  WORKBENCH_IMAGE_PATCH_KINDS,
} from "../../features/workbench";

const menuEl = ref<HTMLElement | null>(null);
defineExpose({ menuEl });

type SelectedResourceRef = {
  kind: string;
  name: string;
  namespace: string | null;
  nodeName: string | null;
  dynamic?: { api_version: string; namespaced: boolean };
};

const props = defineProps<{
  visible: boolean;
  position: { x: number; y: number };
  selectedResource: SelectedResourceRef | null;
  canOpenNodeTerminal: boolean;
  canOpenPodDebug: boolean;
  nodeTerminalMenuLabel: string;
  nodeTerminalDisabledReason: string;
  podDebugDisabledReason: string;
  deleteActionArmed: boolean;
}>();

const quickTargetMeta = computed(() => {
  if (!props.selectedResource) return "";
  const ns = props.selectedResource.namespace ? `${props.selectedResource.namespace}/` : "";
  return `${props.selectedResource.kind} · ${ns}${props.selectedResource.name}`;
});

const emit = defineEmits<{
  close: [];
  openDetail: [];
  openTopology: [];
  openPodLogs: [];
  openPodShell: [];
  openNodeTerminal: [];
  openPodDebug: [];
  openEditConfig: [];
  openChangeImage: [];
  openSyncOrchestrator: [];
  handleDelete: [];
}>();
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="action-menu-backdrop" @click="emit('close')">
      <div
        ref="menuEl"
        class="action-menu-overlay"
        :style="{ left: position.x + 'px', top: position.y + 'px' }"
        role="menu"
        @click.stop
      >
        <header class="action-menu-header">
          <div class="action-menu-title">资源操作</div>
          <div class="action-menu-target" :title="quickTargetMeta">{{ quickTargetMeta }}</div>
        </header>
        <div class="action-menu-section action-menu-section-green">
          <div class="action-menu-section-title">查看与导航</div>
          <NButton type="default" quaternary class="action-menu-item" @click="emit('openDetail')">
            <span class="action-menu-main">查看详情</span>
            <span class="action-menu-tail">YAML</span>
          </NButton>
          <NButton type="default" quaternary class="action-menu-item" @click="emit('openTopology')">
            <span class="action-menu-main">关联资源</span>
            <span class="action-menu-tail">拓扑</span>
          </NButton>
        </div>
        <div class="action-menu-section action-menu-section-cyan">
          <div class="action-menu-section-title">运行与调试</div>
          <NButton
            v-if="selectedResource && ['Pod', 'Deployment', 'StatefulSet', 'DaemonSet'].includes(selectedResource.kind)"
            type="default"
            quaternary
            class="action-menu-item"
            @click="emit('openPodLogs')"
          >
            <span class="action-menu-main">打开日志中心</span>
            <span class="action-menu-tail">Logs</span>
          </NButton>
          <NButton
            v-if="selectedResource && WORKBENCH_SHELL_WORKLOAD_KINDS.has(selectedResource.kind)"
            type="default"
            quaternary
            class="action-menu-item"
            @click="emit('openPodShell')"
          >
            <span class="action-menu-main">打开 Shell</span>
            <span class="action-menu-tail">Exec</span>
          </NButton>
          <NButton
            v-if="selectedResource && WORKBENCH_NODE_TERMINAL_RESOURCE_KINDS.has(selectedResource.kind)"
            type="default"
            quaternary
            class="action-menu-item"
            :class="{ 'action-menu-item-disabled': !canOpenNodeTerminal }"
            :disabled="!canOpenNodeTerminal"
            :title="nodeTerminalDisabledReason || nodeTerminalMenuLabel"
            @click="emit('openNodeTerminal')"
          >
            <span class="action-menu-main">{{ nodeTerminalMenuLabel }}</span>
            <span class="action-menu-tail">Node</span>
          </NButton>
          <NButton
            v-if="selectedResource?.kind === 'Pod'"
            type="default"
            quaternary
            class="action-menu-item"
            :class="{ 'action-menu-item-disabled': !canOpenPodDebug }"
            :disabled="!canOpenPodDebug"
            :title="podDebugDisabledReason || '进入容器调试环境'"
            @click="emit('openPodDebug')"
          >
            <span class="action-menu-main">进入容器调试环境</span>
            <span class="action-menu-tail">Debug</span>
          </NButton>
        </div>
        <div class="action-menu-section action-menu-section-blue">
          <div class="action-menu-section-title">编排与变更</div>
          <NButton
            v-if="selectedResource && (selectedResource.kind === 'ConfigMap' || selectedResource.kind === 'Secret')"
            type="default"
            quaternary
            class="action-menu-item"
            @click="emit('openEditConfig')"
          >
            <span class="action-menu-main">修改配置</span>
            <span class="action-menu-tail">Config</span>
          </NButton>
          <NButton
            v-if="selectedResource && WORKBENCH_IMAGE_PATCH_KINDS.has(selectedResource.kind)"
            type="default"
            quaternary
            class="action-menu-item"
            @click="emit('openChangeImage')"
          >
            <span class="action-menu-main">修改镜像</span>
            <span class="action-menu-tail">Image</span>
          </NButton>
          <NButton type="default" quaternary class="action-menu-item" @click="emit('openSyncOrchestrator')">
            <span class="action-menu-main">编排中心</span>
            <span class="action-menu-tail">Flow</span>
          </NButton>
        </div>
        <div class="action-menu-section action-menu-section-danger">
          <div class="action-menu-section-title">危险操作</div>
          <NButton
            type="error"
            quaternary
            class="action-menu-item action-menu-item-danger"
            :class="{ 'action-menu-item-danger-armed': deleteActionArmed }"
            @click="emit('handleDelete')"
          >
            <span class="action-menu-main">{{ deleteActionArmed ? "再次点击确认删除" : "删除资源" }}</span>
            <span class="action-menu-tail action-menu-tail-danger">{{ deleteActionArmed ? "确认" : "Danger" }}</span>
          </NButton>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.action-menu-backdrop {
  position: fixed;
  inset: 0;
  z-index: 999;
}
.action-menu-overlay {
  position: fixed;
  z-index: 1000;
  width: min(312px, calc(100vw - 20px));
  max-width: calc(100vw - 20px);
  max-height: calc(100vh - 20px);
  padding: 0.38rem 0.38rem 0.42rem;
  background:
    radial-gradient(circle at top right, rgba(37, 99, 235, 0.08), transparent 42%),
    linear-gradient(180deg, rgba(255, 255, 255, 0.99), rgba(248, 250, 252, 0.98));
  border: 1px solid var(--kf-border, rgba(148, 163, 184, 0.3));
  border-radius: 12px;
  box-shadow:
    0 22px 52px rgba(15, 23, 42, 0.22),
    0 1px 0 rgba(255, 255, 255, 0.8) inset;
  backdrop-filter: blur(6px);
  overflow: auto;
  overscroll-behavior: contain;
}
.action-menu-header {
  padding: 0.35rem 0.44rem 0.5rem;
  border-bottom: 1px solid var(--kf-border, rgba(148, 163, 184, 0.2));
  margin-bottom: 0.28rem;
  background: linear-gradient(180deg, rgba(239, 246, 255, 0.72), rgba(255, 255, 255, 0.3));
  border-radius: 8px;
}
.action-menu-title {
  font-size: 0.76rem;
  font-weight: 700;
  color: #0f172a;
  letter-spacing: 0.02em;
}
.action-menu-target {
  margin-top: 0.16rem;
  font-size: 0.67rem;
  color: var(--kf-text-secondary, #64748b);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.action-menu-section {
  padding: 0;
  background: rgba(255, 255, 255, 0.66);
  border: 1px solid var(--kf-border, rgba(226, 232, 240, 0.8));
  border-radius: 9px;
  margin: 0 0.1rem;
}
.action-menu-section-green {
  background: linear-gradient(180deg, rgba(239, 246, 255, 0.78), rgba(255, 255, 255, 0.72));
  border-color: rgba(147, 197, 253, 0.5);
}
.action-menu-section-cyan {
  background: linear-gradient(180deg, rgba(236, 254, 255, 0.78), rgba(255, 255, 255, 0.72));
  border-color: rgba(103, 232, 249, 0.52);
}
.action-menu-section-blue {
  background: linear-gradient(180deg, rgba(239, 246, 255, 0.78), rgba(255, 255, 255, 0.72));
  border-color: rgba(147, 197, 253, 0.48);
}
.action-menu-section:not(:last-child) {
  margin-bottom: 0.2rem;
  padding-bottom: 0.22rem;
  border-bottom: 1px solid rgba(241, 245, 249, 0.9);
}
.action-menu-section-title {
  padding: 0.18rem 0.44rem;
  font-size: 0.64rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: #94a3b8;
}
.action-menu-section-green .action-menu-section-title {
  color: #15803d;
}
.action-menu-section-cyan .action-menu-section-title {
  color: #0e7490;
}
.action-menu-section-blue .action-menu-section-title {
  color: #1d4ed8;
}
.action-menu-item {
  width: 100%;
  min-height: 2rem;
  padding: 0.38rem 0.5rem;
  font-size: 0.8rem;
  text-align: left;
  color: #334155;
  border-radius: 8px;
}
.action-menu-item:deep(.n-button__content) {
  width: 100%;
  display: inline-flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.4rem;
  text-align: left;
}
.action-menu-item:deep(.n-button__state-border),
.action-menu-item:deep(.n-button__border),
.action-menu-item:deep(.n-button__ripple) {
  border-radius: 8px;
}
.action-menu-main {
  color: inherit;
  line-height: 1.2;
  font-weight: 520;
  min-width: 0;
}
.action-menu-tail {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  height: 1.15rem;
  padding: 0 0.38rem;
  border-radius: 999px;
  font-size: 0.62rem;
  font-weight: 650;
  letter-spacing: 0.02em;
  background: #e2e8f0;
  color: #475569;
}
.action-menu-section-green .action-menu-tail {
  background: #dcfce7;
  color: #15803d;
}
.action-menu-section-cyan .action-menu-tail {
  background: #cffafe;
  color: #0e7490;
}
.action-menu-section-blue .action-menu-tail {
  background: #dbeafe;
  color: #1d4ed8;
}
.action-menu-section-danger .action-menu-tail {
  background: #fee2e2;
  color: #b91c1c;
}
.action-menu-tail-danger {
  background: #fee2e2;
  color: #b91c1c;
}
.action-menu-item:hover {
  color: #1d4ed8;
  background: rgba(219, 234, 254, 0.6);
}
.action-menu-item:focus-visible {
  outline: none;
  box-shadow: inset 0 0 0 2px rgba(37, 99, 235, 0.32);
}
.action-menu-item:hover .action-menu-tail {
  background: #cbd5e1;
  color: #334155;
}
.action-menu-section-green .action-menu-item:hover .action-menu-tail {
  background: #bbf7d0;
  color: #166534;
}
.action-menu-section-cyan .action-menu-item:hover .action-menu-tail {
  background: #a5f3fc;
  color: #155e75;
}
.action-menu-section-blue .action-menu-item:hover .action-menu-tail {
  background: #bfdbfe;
  color: #1d4ed8;
}
.action-menu-section-danger .action-menu-item:hover .action-menu-tail {
  background: #fecaca;
  color: #991b1b;
}
.action-menu-item-disabled {
  cursor: not-allowed;
  opacity: 0.62;
}
.action-menu-item-disabled:hover {
  background: transparent;
  color: #334155;
}
.action-menu-item-disabled:hover .action-menu-tail {
  background: #f1f5f9;
  color: #64748b;
}
.action-menu-section-danger {
  margin-top: 0.12rem;
  background: linear-gradient(180deg, rgba(254, 242, 242, 0.68), rgba(255, 255, 255, 0.68));
  border-color: rgba(252, 165, 165, 0.45);
}
.action-menu-section-danger .action-menu-section-title {
  color: #dc2626;
}
.action-menu-item-danger:hover {
  background: #fef2f2;
  color: #dc2626;
}
.action-menu-item-danger:focus-visible {
  outline: none;
  box-shadow: inset 0 0 0 2px rgba(220, 38, 38, 0.32);
}
.action-menu-item-danger-armed {
  background: #fee2e2;
  color: #b91c1c;
  box-shadow: inset 0 0 0 1px #fca5a5;
}
@media (max-width: 640px) {
  .action-menu-overlay {
    width: min(292px, calc(100vw - 16px));
    max-width: calc(100vw - 16px);
    max-height: calc(100vh - 16px);
    padding: 0.32rem;
    border-radius: 10px;
  }
  .action-menu-item {
    min-height: 1.9rem;
    padding: 0.34rem 0.45rem;
  }
  .action-menu-main {
    font-size: 0.76rem;
  }
}
</style>
