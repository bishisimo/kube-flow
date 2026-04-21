import type { WorkloadPodRollup } from "../../../api/kube";

export type PodRollupBadgeTone = "running" | "pending" | "succeeded" | "failed" | "abnormal";
export type PodRollupBadge = { key: string; value: number; tone: PodRollupBadgeTone };

/**
 * 工作台表格展示辅助：状态色、Pod 态势徽标与节点分配色阶。
 */
export function useWorkbenchTableDecorators() {
  function normalizeStatus(raw: unknown): string {
    const v = String(raw ?? "-").trim();
    return v || "-";
  }

  function statusTone(statusValue: unknown): "ok" | "warn" | "error" | "neutral" {
    const v = normalizeStatus(statusValue).toLowerCase();
    if (!v || v === "-") return "neutral";
    if (v.includes("notready") || v.includes("not ready") || v.includes("unready")) return "error";
    if (v.includes("running") || v.includes("ready") || v.includes("bound") || v.includes("active")) return "ok";
    if (
      v.includes("pending") ||
      v.includes("containercreating") ||
      v.includes("terminating") ||
      v.includes("unknown")
    ) {
      return "warn";
    }
    if (
      v.includes("failed") ||
      v.includes("error") ||
      v.includes("crashloopbackoff") ||
      v.includes("imagepullbackoff")
    ) {
      return "error";
    }
    return "neutral";
  }

  function isStatusColumn(colKey: string): boolean {
    return colKey === "phase" || colKey === "status" || colKey === "containerStatus";
  }

  function isPodRollupColumn(colKey: string): boolean {
    return colKey === "podRollup";
  }

  function buildPodRollupBadges(v: unknown): PodRollupBadge[] {
    const r = (v ?? {}) as WorkloadPodRollup;
    const out: PodRollupBadge[] = [];
    const push = (key: string, tone: PodRollupBadgeTone, n: unknown) => {
      const m = Number(n ?? 0);
      if (Number.isFinite(m) && m > 0) out.push({ key, value: m, tone });
    };
    push("running", "running", r.running_ready);
    push("pending", "pending", r.pending);
    push("succeeded", "succeeded", r.succeeded);
    push("failed", "failed", r.failed);
    push("abnormal", "abnormal", r.abnormal);
    return out;
  }

  function formatRecentRestart(v: unknown): string {
    const r = (v ?? {}) as WorkloadPodRollup;
    return r.last_container_restart ?? "-";
  }

  function isRecentRestartHot(v: unknown): boolean {
    const text = formatRecentRestart(v);
    if (text === "-") return false;
    const m = text.match(/\(([^)]+)\)/);
    const age = (m?.[1] ?? text).trim();
    if (age.endsWith("秒前")) return true;
    if (age.endsWith("分钟前")) {
      const n = Number.parseInt(age.replace("分钟前", ""), 10);
      return Number.isFinite(n) && n <= 15;
    }
    return false;
  }

  function isNodeAllocColumn(key: string): boolean {
    return key === "cpuRequests" || key === "memoryRequests" || key === "gpuRequests";
  }

  function parseAllocPercent(value: unknown): number | null {
    if (typeof value !== "string") return null;
    const m = value.match(/\((\d+)%\)/);
    if (!m) return null;
    const percent = Number.parseInt(m[1], 10);
    return Number.isFinite(percent) ? percent : null;
  }

  function nodeAllocTone(value: unknown): "" | "warn" | "danger" {
    const percent = parseAllocPercent(value);
    if (percent == null) return "";
    if (percent >= 90) return "danger";
    if (percent >= 80) return "warn";
    return "";
  }

  return {
    normalizeStatus,
    statusTone,
    isStatusColumn,
    isPodRollupColumn,
    buildPodRollupBadges,
    formatRecentRestart,
    isRecentRestartHot,
    isNodeAllocColumn,
    nodeAllocTone,
  };
}
