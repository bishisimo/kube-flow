import {
  defaultEditability,
  resolvePodEditability,
  resolveReplicaSetEditability,
} from "./podEditability";
import type { ResourceEditContext, ResourceEditProfile } from "./types";
import { isWorkloadKind, STRUCTURED_READONLY_KINDS } from "./workloadPaths";

const DEFAULT_YAML_PROFILE: ResourceEditProfile = {
  primarySurface: "yaml",
  snapshotCategory: "resource",
  resolveEditability: () => defaultEditability(),
};

const CONFIG_PROFILE: ResourceEditProfile = {
  primarySurface: "kv",
  snapshotCategory: "config",
  resolveEditability: () => defaultEditability(),
};

const WORKLOAD_PROFILE: ResourceEditProfile = {
  primarySurface: "structured",
  snapshotCategory: "resource",
  resolveEditability: () => defaultEditability(),
};

const POD_PROFILE: ResourceEditProfile = {
  primarySurface: "yaml",
  snapshotCategory: "resource",
  resolveEditability: resolvePodEditability,
};

const REPLICASET_PROFILE: ResourceEditProfile = {
  primarySurface: "yaml",
  snapshotCategory: "resource",
  resolveEditability: () => resolveReplicaSetEditability(),
};

const REGISTRY: Record<string, ResourceEditProfile> = {
  ConfigMap: CONFIG_PROFILE,
  Secret: CONFIG_PROFILE,
  Deployment: WORKLOAD_PROFILE,
  StatefulSet: WORKLOAD_PROFILE,
  DaemonSet: WORKLOAD_PROFILE,
  Job: WORKLOAD_PROFILE,
  CronJob: WORKLOAD_PROFILE,
  Pod: POD_PROFILE,
  ReplicaSet: REPLICASET_PROFILE,
};

export function resolveEditProfile(kind: string): ResourceEditProfile {
  return REGISTRY[kind] ?? DEFAULT_YAML_PROFILE;
}

export function resolveEditability(ctx: ResourceEditContext) {
  const profile = resolveEditProfile(ctx.resource.kind);
  if (STRUCTURED_READONLY_KINDS.has(ctx.resource.kind)) {
    return resolveReplicaSetEditability();
  }
  return profile.resolveEditability(ctx);
}

export function supportsStructuredEdit(kind: string): boolean {
  return isWorkloadKind(kind);
}
