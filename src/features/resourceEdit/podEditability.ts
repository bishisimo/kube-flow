import type { Editability, K8sObject, ResourceEditContext } from "./types";

interface OwnerRef {
  kind?: string;
  name?: string;
  controller?: boolean;
}

function getOwnerRefs(obj: K8sObject | null): OwnerRef[] {
  const metadata = obj?.metadata;
  if (!metadata || typeof metadata !== "object") return [];
  const owners = (metadata as Record<string, unknown>).ownerReferences;
  return Array.isArray(owners) ? (owners as OwnerRef[]) : [];
}

function resolveControllerOwner(owners: OwnerRef[]): OwnerRef | null {
  const controller = owners.find((o) => o.controller);
  return controller ?? owners[0] ?? null;
}

/** 解析 Pod 是否由上层 Workload 管理。 */
export function resolvePodEditability(ctx: ResourceEditContext): Editability {
  const owner = resolveControllerOwner(getOwnerRefs(ctx.obj));
  if (!owner?.kind || !owner.name) {
    return { allowed: true, structuredAllowed: true };
  }

  const parentKind = owner.kind;
  const parentName = owner.name;

  return {
    allowed: true,
    structuredAllowed: false,
    reason: `此 Pod 由 ${owner.kind}/${owner.name} 管理，结构化编辑将在滚动更新后被覆盖。请编辑父级 Workload，或使用 YAML 模式。`,
    parentKind,
    parentName,
  };
}

export function resolveReplicaSetEditability(): Editability {
  return {
    allowed: true,
    structuredAllowed: false,
    reason: "ReplicaSet 由 Deployment 管理，请通过关联拓扑找到父级 Deployment 后编辑。",
  };
}

export function defaultEditability(): Editability {
  return { allowed: true, structuredAllowed: true };
}

export function readonlyEditability(reason: string): Editability {
  return { allowed: false, structuredAllowed: false, reason };
}
