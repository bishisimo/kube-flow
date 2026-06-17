import type { K8sObject } from "./types";

export const WORKLOAD_KINDS = new Set([
  "Deployment",
  "StatefulSet",
  "DaemonSet",
  "Job",
  "CronJob",
]);

export const STRUCTURED_READONLY_KINDS = new Set(["ReplicaSet"]);

export function isWorkloadKind(kind: string): boolean {
  return WORKLOAD_KINDS.has(kind);
}

/** Pod 模板 spec 在资源对象中的 JSON 路径。 */
export function getPodTemplateSpecPath(kind: string): string[] {
  switch (kind) {
    case "Deployment":
    case "StatefulSet":
    case "DaemonSet":
    case "Job":
      return ["spec", "template", "spec"];
    case "CronJob":
      return ["spec", "jobTemplate", "spec", "template", "spec"];
    default:
      return [];
  }
}

export function getAtPath(obj: unknown, path: string[]): unknown {
  let cur = obj;
  for (const key of path) {
    if (!cur || typeof cur !== "object") return undefined;
    cur = (cur as Record<string, unknown>)[key];
  }
  return cur;
}

export function setAtPath(obj: Record<string, unknown>, path: string[], value: unknown): void {
  if (path.length === 0) return;
  let cur: Record<string, unknown> = obj;
  for (let i = 0; i < path.length - 1; i++) {
    const key = path[i];
    const next = cur[key];
    if (!next || typeof next !== "object" || Array.isArray(next)) {
      cur[key] = {};
    }
    cur = cur[key] as Record<string, unknown>;
  }
  cur[path[path.length - 1]] = value;
}

/** 由叶子路径构造 patch 用的嵌套对象。 */
export function buildNestedPatch(path: string[], value: unknown): Record<string, unknown> {
  let current: unknown = value;
  for (let i = path.length - 1; i >= 0; i--) {
    current = { [path[i]]: current };
  }
  return current as Record<string, unknown>;
}

export interface WorkloadContainerDraft {
  name: string;
  image: string;
}

export function extractContainers(obj: K8sObject, kind: string): WorkloadContainerDraft[] {
  const podSpec = getAtPath(obj, getPodTemplateSpecPath(kind)) as Record<string, unknown> | undefined;
  const containers = Array.isArray(podSpec?.containers) ? podSpec.containers : [];
  return containers
    .filter((c): c is Record<string, unknown> => !!c && typeof c === "object")
    .map((c) => ({
      name: typeof c.name === "string" ? c.name : "",
      image: typeof c.image === "string" ? c.image : "",
    }));
}

export function buildContainersPatch(
  kind: string,
  containers: WorkloadContainerDraft[],
): Record<string, unknown> {
  const templatePath =
    kind === "CronJob"
      ? ["spec", "jobTemplate", "spec", "template", "spec", "containers"]
      : ["spec", "template", "spec", "containers"];
  const patchContainers = containers.map((c) => ({ name: c.name, image: c.image }));
  return buildNestedPatch(templatePath, patchContainers);
}
