/**
 * 工作台当前资源在命令面板中的动作候选项（与 WorkbenchActionMenu 能力对齐）。
 */
import type { TokenValueCandidate } from "../commandPalette/types";
import {
  WORKBENCH_IMAGE_PATCH_KINDS,
  WORKBENCH_NODE_TERMINAL_RESOURCE_KINDS,
  WORKBENCH_RESTART_KINDS,
  WORKBENCH_SHELL_WORKLOAD_KINDS,
  WORKBENCH_STOP_RESUME_KINDS,
} from "./constants";

const LOG_WORKLOAD_KINDS = new Set(["Pod", "Deployment", "StatefulSet", "DaemonSet"]);

export type ResourcePaletteActionCaps = {
  nodeTerminalMenuLabel: string;
  nodeTerminalDisabledReason: string;
  podDebugDisabledReason: string;
  /** Deploy/STS 当前期望副本数 */
  workloadReplicasDesired?: number | null;
  /** Deploy/STS 停止时保存的副本数 */
  workloadSavedReplicas?: number | null;
};

export function buildResourcePaletteValueCandidates(
  r: {
    kind: string;
    name: string;
    namespace: string | null;
    nodeName: string | null;
    dynamic?: { api_version: string; namespaced: boolean };
  } | null,
  caps: ResourcePaletteActionCaps | null,
): TokenValueCandidate[] {
  if (!r || !caps) {
    return [
      {
        value: "__please_select",
        title: "请先在工作台列表选中资源",
        subtitle: "↑↓ 选择行后按 Enter 打开命令面板",
        icon: "⚠️",
      },
    ];
  }

  const out: TokenValueCandidate[] = [];
  out.push({
    value: "openDetail",
    title: "查看详情",
    subtitle: "YAML · 抽屉",
    icon: "📄",
    keywords: ["detail", "yaml", "详情"],
  });
  out.push({
    value: "openTopology",
    title: "关联资源",
    subtitle: "拓扑",
    icon: "🔗",
    keywords: ["topology", "拓扑"],
  });

  if (LOG_WORKLOAD_KINDS.has(r.kind)) {
    out.push({
      value: "openPodLogs",
      title: "打开日志中心",
      subtitle: "Logs",
      icon: "📜",
      keywords: ["log", "日志"],
    });
  }
  if (WORKBENCH_SHELL_WORKLOAD_KINDS.has(r.kind)) {
    out.push({
      value: "openPodShell",
      title: "打开 Shell",
      subtitle: "Exec",
      icon: "💻",
      keywords: ["shell", "终端"],
    });
  }
  if (WORKBENCH_NODE_TERMINAL_RESOURCE_KINDS.has(r.kind) && !caps.nodeTerminalDisabledReason) {
    out.push({
      value: "openNodeTerminal",
      title: caps.nodeTerminalMenuLabel,
      subtitle: "Node",
      icon: "🖥️",
      keywords: ["node", "节点"],
    });
  }
  if (r.kind === "Pod" && !caps.podDebugDisabledReason) {
    out.push({
      value: "openPodDebug",
      title: "进入容器调试环境",
      subtitle: "Debug",
      icon: "🐛",
    });
  }
  if (r.kind === "ConfigMap" || r.kind === "Secret") {
    out.push({
      value: "openEditConfig",
      title: "编辑配置",
      subtitle: "KV / YAML",
      icon: "✏️",
    });
  }
  if (WORKBENCH_IMAGE_PATCH_KINDS.has(r.kind)) {
    out.push({
      value: "openChangeImage",
      title: "修改镜像",
      subtitle: "Image",
      icon: "📦",
    });
  }
  if (WORKBENCH_STOP_RESUME_KINDS.has(r.kind)) {
    const saved = caps.workloadSavedReplicas;
    const desired = caps.workloadReplicasDesired ?? 0;
    if (saved != null && saved > 0) {
      out.push({
        value: "resumeWorkload",
        title: "恢复",
        subtitle: `副本 → ${saved}`,
        icon: "▶️",
        keywords: ["resume", "恢复", "start"],
      });
    } else if (desired > 0) {
      out.push({
        value: "stopWorkload",
        title: "停止",
        subtitle: "Scale 0",
        icon: "⏹️",
        keywords: ["stop", "停止", "scale"],
      });
    }
  }
  if (WORKBENCH_RESTART_KINDS.has(r.kind)) {
    out.push({
      value: "restartWorkload",
      title: "重启",
      subtitle: "Rollout",
      icon: "🔄",
      keywords: ["restart", "重启", "rollout"],
    });
  }
  out.push({
    value: "openSyncOrchestrator",
    title: "编排中心",
    subtitle: "同步到编排",
    icon: "🔀",
    keywords: ["orchestrator", "编排"],
  });
  out.push({
    value: "handleDelete",
    title: "删除资源",
    subtitle: "危险操作",
    icon: "🗑️",
    keywords: ["delete", "删除"],
  });

  return out;
}
