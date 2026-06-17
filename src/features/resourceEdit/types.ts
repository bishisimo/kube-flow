import type { SelectedResource } from "../workbench/contracts";

export type K8sObject = Record<string, unknown>;

export type EditSurfaceMode = "structured" | "yaml" | "kv";

export type SnapshotCategory = "resource" | "config";

export interface Editability {
  allowed: boolean;
  structuredAllowed: boolean;
  reason?: string;
  parentKind?: string;
  parentName?: string;
}

export interface ResourceEditContext {
  resource: SelectedResource;
  obj: K8sObject | null;
}

export interface ResourceEditProfile {
  primarySurface: EditSurfaceMode;
  snapshotCategory: SnapshotCategory;
  resolveEditability: (ctx: ResourceEditContext) => Editability;
}

export type EditIntent =
  | { mode: "kv" }
  | { mode: "yaml" }
  | { mode: "structured" };
