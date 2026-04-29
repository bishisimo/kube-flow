<script setup lang="ts">
import { computed, h } from "vue";
import { NDropdown } from "naive-ui";
import type { DropdownOption } from "naive-ui";
import {
  WORKBENCH_SHELL_WORKLOAD_KINDS,
  WORKBENCH_NODE_TERMINAL_RESOURCE_KINDS,
  WORKBENCH_IMAGE_PATCH_KINDS,
} from "../../features/workbench";

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

type ActionEmitKey =
  | "openDetail"
  | "openTopology"
  | "openPodLogs"
  | "openPodShell"
  | "openNodeTerminal"
  | "openPodDebug"
  | "openEditConfig"
  | "openChangeImage"
  | "openSyncOrchestrator"
  | "handleDelete";

const emitMap: Record<ActionEmitKey, () => void> = {
  openDetail: () => emit("openDetail"),
  openTopology: () => emit("openTopology"),
  openPodLogs: () => emit("openPodLogs"),
  openPodShell: () => emit("openPodShell"),
  openNodeTerminal: () => emit("openNodeTerminal"),
  openPodDebug: () => emit("openPodDebug"),
  openEditConfig: () => emit("openEditConfig"),
  openChangeImage: () => emit("openChangeImage"),
  openSyncOrchestrator: () => emit("openSyncOrchestrator"),
  handleDelete: () => emit("handleDelete"),
};

type WBOption = DropdownOption & {
  tail?: string;
  tone?: "navi" | "run" | "flow" | "danger";
  emit?: ActionEmitKey;
};

const quickTargetMeta = computed(() => {
  if (!props.selectedResource) return "";
  const ns = props.selectedResource.namespace ? `${props.selectedResource.namespace}/` : "";
  return `${props.selectedResource.kind} · ${ns}${props.selectedResource.name}`;
});

function renderHeader() {
  return h("div", { class: "wb-act-header" }, [
    h("div", { class: "wb-act-title" }, "资源操作"),
    h("div", { class: "wb-act-target", title: quickTargetMeta.value }, quickTargetMeta.value),
  ]);
}

function renderGroup(text: string, tone: WBOption["tone"]) {
  return h("div", { class: ["wb-act-group", `wb-act-group-${tone}`] }, text);
}

const menuOptions = computed<WBOption[]>(() => {
  const opts: WBOption[] = [];
  if (quickTargetMeta.value) {
    opts.push({ key: "__header", type: "render", render: renderHeader });
    opts.push({ type: "divider", key: "__d0" });
  }

  opts.push({ key: "__g_navi", type: "render", render: () => renderGroup("查看与导航", "navi") });
  opts.push({ key: "openDetail", label: "查看详情", tail: "YAML", tone: "navi", emit: "openDetail" });
  opts.push({ key: "openTopology", label: "关联资源", tail: "拓扑", tone: "navi", emit: "openTopology" });

  const r = props.selectedResource;
  const runItems: WBOption[] = [];
  if (r && ["Pod", "Deployment", "StatefulSet", "DaemonSet"].includes(r.kind)) {
    runItems.push({ key: "openPodLogs", label: "打开日志中心", tail: "Logs", tone: "run", emit: "openPodLogs" });
  }
  if (r && WORKBENCH_SHELL_WORKLOAD_KINDS.has(r.kind)) {
    runItems.push({ key: "openPodShell", label: "打开 Shell", tail: "Exec", tone: "run", emit: "openPodShell" });
  }
  if (r && WORKBENCH_NODE_TERMINAL_RESOURCE_KINDS.has(r.kind)) {
    runItems.push({
      key: "openNodeTerminal",
      label: props.nodeTerminalMenuLabel,
      tail: "Node",
      tone: "run",
      emit: "openNodeTerminal",
      disabled: !props.canOpenNodeTerminal,
      props: props.nodeTerminalDisabledReason ? { title: props.nodeTerminalDisabledReason } : undefined,
    });
  }
  if (r?.kind === "Pod") {
    runItems.push({
      key: "openPodDebug",
      label: "进入容器调试环境",
      tail: "Debug",
      tone: "run",
      emit: "openPodDebug",
      disabled: !props.canOpenPodDebug,
      props: props.podDebugDisabledReason ? { title: props.podDebugDisabledReason } : undefined,
    });
  }
  if (runItems.length) {
    opts.push({ type: "divider", key: "__d1" });
    opts.push({ key: "__g_run", type: "render", render: () => renderGroup("运行与调试", "run") });
    opts.push(...runItems);
  }

  const flowItems: WBOption[] = [];
  if (r && (r.kind === "ConfigMap" || r.kind === "Secret")) {
    flowItems.push({ key: "openEditConfig", label: "修改配置", tail: "Config", tone: "flow", emit: "openEditConfig" });
  }
  if (r && WORKBENCH_IMAGE_PATCH_KINDS.has(r.kind)) {
    flowItems.push({ key: "openChangeImage", label: "修改镜像", tail: "Image", tone: "flow", emit: "openChangeImage" });
  }
  flowItems.push({ key: "openSyncOrchestrator", label: "编排中心", tail: "Flow", tone: "flow", emit: "openSyncOrchestrator" });
  opts.push({ type: "divider", key: "__d2" });
  opts.push({ key: "__g_flow", type: "render", render: () => renderGroup("编排与变更", "flow") });
  opts.push(...flowItems);

  opts.push({ type: "divider", key: "__d3" });
  opts.push({ key: "__g_danger", type: "render", render: () => renderGroup("危险操作", "danger") });
  opts.push({
    key: "handleDelete",
    label: props.deleteActionArmed ? "再次点击确认删除" : "删除资源",
    tail: props.deleteActionArmed ? "确认" : "Danger",
    tone: "danger",
    emit: "handleDelete",
  });

  return opts;
});

function onSelect(_key: string | number, option: DropdownOption) {
  const o = option as WBOption;
  if (o.disabled) return;
  if (o.emit) emitMap[o.emit]();
}

function onClickoutside() {
  if (props.visible) emit("close");
}

function renderLabel(option: DropdownOption) {
  const o = option as WBOption;
  const main = h("span", { class: "wb-act-main" }, String(o.label ?? ""));
  const tail = o.tail
    ? h("span", { class: ["wb-act-tail", o.tone ? `wb-act-tail-${o.tone}` : ""] }, o.tail)
    : null;
  const rowClasses = ["wb-act-row"];
  if (o.tone === "danger") {
    rowClasses.push(props.deleteActionArmed ? "wb-act-row-danger-armed" : "wb-act-row-danger");
  }
  return h("span", { class: rowClasses }, [main, tail]);
}
</script>

<template>
  <NDropdown
    :show="visible"
    trigger="manual"
    :x="position.x"
    :y="position.y"
    :options="menuOptions"
    :show-arrow="false"
    placement="bottom-start"
    size="medium"
    :render-label="renderLabel"
    :on-clickoutside="onClickoutside"
    :keyboard="true"
    @select="onSelect"
  />
</template>

<style>
/* NDropdown 渲染到 body，scoped 不生效，这里走全局命名空间。 */
.wb-act-row {
  display: inline-flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  gap: 0.6rem;
  min-width: 220px;
}
.wb-act-main {
  font-size: 0.82rem;
  color: inherit;
  min-width: 0;
  font-weight: 500;
}
.wb-act-tail {
  display: inline-flex;
  align-items: center;
  height: 18px;
  padding: 0 0.42rem;
  border-radius: 999px;
  font-size: 0.66rem;
  font-weight: 650;
  letter-spacing: 0.02em;
  background: #e2e8f0;
  color: #475569;
  flex-shrink: 0;
}
.wb-act-tail-navi { background: #dbeafe; color: #1d4ed8; }
.wb-act-tail-run { background: #cffafe; color: #0e7490; }
.wb-act-tail-flow { background: #ede9fe; color: #5b21b6; }
.wb-act-tail-danger { background: #fee2e2; color: #b91c1c; }
.wb-act-row-danger .wb-act-main { color: #b91c1c; }
.wb-act-row-danger-armed .wb-act-main { color: #b91c1c; font-weight: 650; }
.wb-act-header {
  padding: 0.45rem 0.6rem 0.5rem;
  border-bottom: 1px solid rgba(148, 163, 184, 0.22);
  margin-bottom: 0.18rem;
}
.wb-act-title {
  font-size: 0.78rem;
  font-weight: 700;
  color: #0f172a;
  letter-spacing: 0.02em;
}
.wb-act-target {
  margin-top: 0.18rem;
  font-size: 0.7rem;
  color: #64748b;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 280px;
}
.wb-act-group {
  padding: 0.36rem 0.65rem 0.18rem;
  font-size: 0.66rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: #94a3b8;
}
.wb-act-group-navi { color: #1d4ed8; }
.wb-act-group-run { color: #0e7490; }
.wb-act-group-flow { color: #5b21b6; }
.wb-act-group-danger { color: #dc2626; }
</style>
