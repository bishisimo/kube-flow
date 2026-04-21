<script setup lang="ts">
import { ref } from "vue";
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

defineProps<{
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
    <div
      v-if="visible"
      class="action-menu-backdrop"
      @click="emit('close')"
    >
      <div
        ref="menuEl"
        class="action-menu-overlay"
        :style="{ left: position.x + 'px', top: position.y + 'px' }"
        role="menu"
        @click.stop
      >
        <div class="action-menu-section">
          <div class="action-menu-section-title">查看与导航</div>
          <button type="button" class="action-menu-item" @click="emit('openDetail')">
            <span class="action-menu-icon" aria-hidden="true">📄</span>
            <span class="action-menu-text">
              <span class="action-menu-main">查看详情</span>
              <span class="action-menu-sub">打开 YAML、Describe 与编辑面板</span>
            </span>
          </button>
          <button type="button" class="action-menu-item" @click="emit('openTopology')">
            <span class="action-menu-icon" aria-hidden="true">🧭</span>
            <span class="action-menu-text">
              <span class="action-menu-main">关联资源</span>
              <span class="action-menu-sub">查看上下游资源拓扑并快速跳转</span>
            </span>
          </button>
          <button
            v-if="selectedResource && ['Pod', 'Deployment', 'StatefulSet', 'DaemonSet'].includes(selectedResource.kind)"
            type="button"
            class="action-menu-item"
            @click="emit('openPodLogs')"
          >
            <span class="action-menu-icon" aria-hidden="true">📜</span>
            <span class="action-menu-text">
              <span class="action-menu-main">打开日志中心</span>
              <span class="action-menu-sub">集中查看 Pod 或工作负载日志</span>
            </span>
          </button>
          <button
            v-if="selectedResource && WORKBENCH_SHELL_WORKLOAD_KINDS.has(selectedResource.kind)"
            type="button"
            class="action-menu-item"
            @click="emit('openPodShell')"
          >
            <span class="action-menu-icon" aria-hidden="true">⌨</span>
            <span class="action-menu-text">
              <span class="action-menu-main">打开 Shell</span>
              <span class="action-menu-sub">进入容器执行命令与排障</span>
            </span>
          </button>
          <button
            v-if="selectedResource && WORKBENCH_NODE_TERMINAL_RESOURCE_KINDS.has(selectedResource.kind)"
            type="button"
            class="action-menu-item"
            :class="{ 'action-menu-item-disabled': !canOpenNodeTerminal }"
            :disabled="!canOpenNodeTerminal"
            :title="nodeTerminalDisabledReason || nodeTerminalMenuLabel"
            @click="emit('openNodeTerminal')"
          >
            <span class="action-menu-icon" aria-hidden="true">🖥</span>
            <span class="action-menu-text">
              <span class="action-menu-main">{{ nodeTerminalMenuLabel }}</span>
              <span class="action-menu-sub">通过环境入口快速切换到目标节点主机</span>
            </span>
          </button>
          <button
            v-if="selectedResource?.kind === 'Pod'"
            type="button"
            class="action-menu-item"
            :class="{ 'action-menu-item-disabled': !canOpenPodDebug }"
            :disabled="!canOpenPodDebug"
            :title="podDebugDisabledReason || '进入容器调试环境'"
            @click="emit('openPodDebug')"
          >
            <span class="action-menu-icon" aria-hidden="true">🧪</span>
            <span class="action-menu-text">
              <span class="action-menu-main">进入容器调试环境</span>
              <span class="action-menu-sub">通过 nsenter 进入目标容器的网络或完整隔离空间</span>
            </span>
          </button>
        </div>
        <div class="action-menu-section">
          <div class="action-menu-section-title">编辑与变更</div>
          <button
            v-if="selectedResource && (selectedResource.kind === 'ConfigMap' || selectedResource.kind === 'Secret')"
            type="button"
            class="action-menu-item"
            @click="emit('openEditConfig')"
          >
            <span class="action-menu-icon" aria-hidden="true">⚙</span>
            <span class="action-menu-text">
              <span class="action-menu-main">修改配置</span>
              <span class="action-menu-sub">编辑 ConfigMap / Secret 内容</span>
            </span>
          </button>
          <button
            v-if="selectedResource && WORKBENCH_IMAGE_PATCH_KINDS.has(selectedResource.kind)"
            type="button"
            class="action-menu-item"
            @click="emit('openChangeImage')"
          >
            <span class="action-menu-icon" aria-hidden="true">🧩</span>
            <span class="action-menu-text">
              <span class="action-menu-main">修改镜像</span>
              <span class="action-menu-sub">更新工作负载容器镜像版本</span>
            </span>
          </button>
          <button type="button" class="action-menu-item" @click="emit('openSyncOrchestrator')">
            <span class="action-menu-icon" aria-hidden="true">🧱</span>
            <span class="action-menu-text">
              <span class="action-menu-main">编排中心</span>
              <span class="action-menu-sub">同步到编排中心并统一维护 YAML</span>
            </span>
          </button>
        </div>
        <div class="action-menu-section action-menu-section-danger">
          <div class="action-menu-section-title">危险操作</div>
          <button
            type="button"
            class="action-menu-item action-menu-item-danger"
            :class="{ 'action-menu-item-danger-armed': deleteActionArmed }"
            @click="emit('handleDelete')"
          >
            <span class="action-menu-icon" aria-hidden="true">🗑</span>
            <span class="action-menu-text">
              <span class="action-menu-main">
                {{ deleteActionArmed ? "再次点击确认删除" : "删除" }}
              </span>
              <span class="action-menu-sub">
                {{
                  deleteActionArmed
                    ? "将打开删除确认弹窗，避免误操作"
                    : "高风险操作，资源删除后通常不可恢复"
                }}
              </span>
            </span>
          </button>
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
  width: min(320px, calc(100vw - 20px));
  max-width: calc(100vw - 20px);
  max-height: calc(100vh - 20px);
  padding: 0.5rem 0;
  background: #fff;
  border: 1px solid #e2e8f0;
  border-radius: 12px;
  box-shadow: 0 14px 32px rgba(15, 23, 42, 0.18);
  overflow: auto;
  overscroll-behavior: contain;
}
.action-menu-section {
  padding: 0 0.25rem;
}
.action-menu-section:not(:last-child) {
  margin-bottom: 0.35rem;
  padding-bottom: 0.35rem;
  border-bottom: 1px solid #f1f5f9;
}
.action-menu-section-title {
  padding: 0.25rem 0.5rem;
  font-size: 0.6875rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: #94a3b8;
}
.action-menu-item {
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
  width: 100%;
  padding: 0.45rem 0.75rem;
  border: none;
  background: none;
  font-size: 0.8125rem;
  text-align: left;
  color: #334155;
  cursor: pointer;
  border-radius: 4px;
}
.action-menu-icon {
  width: 1rem;
  text-align: center;
  opacity: 0.9;
  margin-top: 0.1rem;
}
.action-menu-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.action-menu-main {
  color: inherit;
  line-height: 1.2;
}
.action-menu-sub {
  margin-top: 0.12rem;
  font-size: 0.72rem;
  line-height: 1.25;
  color: #94a3b8;
  word-break: break-word;
}
.action-menu-item:hover .action-menu-sub {
  color: #64748b;
}
.action-menu-item:hover {
  background: #f1f5f9;
  color: #2563eb;
}
.action-menu-item-disabled {
  cursor: not-allowed;
  opacity: 0.72;
}
.action-menu-item-disabled:hover {
  background: none;
  color: #334155;
}
.action-menu-item-disabled:hover .action-menu-sub {
  color: #94a3b8;
}
.action-menu-section-danger {
  border-left: 2px solid #fecaca;
  margin-left: 0.25rem;
  padding-left: 0.25rem;
}
.action-menu-item-danger:hover {
  background: #fef2f2;
  color: #dc2626;
}
.action-menu-item-danger:hover .action-menu-sub {
  color: #b91c1c;
}
.action-menu-item-danger-armed {
  background: #fee2e2;
  color: #b91c1c;
  box-shadow: inset 2px 0 0 #dc2626;
}
.action-menu-item-danger-armed .action-menu-sub {
  color: #b91c1c;
}
.action-menu-loading {
  padding: 0.4rem 0.75rem;
  font-size: 0.8125rem;
  color: #94a3b8;
}
@media (max-width: 640px) {
  .action-menu-overlay {
    width: min(300px, calc(100vw - 16px));
    max-width: calc(100vw - 16px);
    max-height: calc(100vh - 16px);
    padding: 0.35rem 0;
    border-radius: 10px;
  }
  .action-menu-section {
    padding: 0 0.18rem;
  }
  .action-menu-item {
    gap: 0.42rem;
    padding: 0.42rem 0.62rem;
  }
  .action-menu-main {
    font-size: 0.78rem;
  }
  .action-menu-sub {
    font-size: 0.68rem;
  }
}
</style>
