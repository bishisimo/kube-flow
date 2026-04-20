import { kubeGetResourceGraph } from "../../../api/kube";

export type SyncRelatedKind =
  | "ConfigMap"
  | "Secret"
  | "Service"
  | "ServiceAccount"
  | "PersistentVolumeClaim"
  | "RoleBinding"
  | "ClusterRoleBinding"
  | "Role"
  | "ClusterRole";

export type SyncRelatedRef = { kind: SyncRelatedKind; name: string; namespace: string | null };

const SYNC_KINDS = new Set<string>([
  "ConfigMap",
  "Secret",
  "Service",
  "ServiceAccount",
  "PersistentVolumeClaim",
  "RoleBinding",
  "ClusterRoleBinding",
  "Role",
  "ClusterRole",
]);

export async function collectAssociatedRefs(
  envId: string,
  kind: string,
  name: string,
  namespace: string | null
): Promise<SyncRelatedRef[]> {
  const graph = await kubeGetResourceGraph(envId, kind, name, namespace);
  const rootKind = graph.root.kind;
  const rootName = graph.root.name;
  const rootNs = graph.root.namespace ?? null;

  const refs: SyncRelatedRef[] = [];
  for (const node of graph.nodes) {
    const ref = node.resource_ref;
    // skip root itself and aggregate (non-concrete) nodes
    if (!node.is_concrete) continue;
    if (ref.kind === rootKind && ref.name === rootName && (ref.namespace ?? null) === rootNs) continue;
    if (!SYNC_KINDS.has(ref.kind)) continue;
    refs.push({ kind: ref.kind as SyncRelatedKind, name: ref.name, namespace: ref.namespace ?? null });
  }

  // dedup by kind|namespace|name
  const dedup = new Map<string, SyncRelatedRef>();
  for (const r of refs) {
    dedup.set(`${r.kind}|${r.namespace ?? ""}|${r.name}`, r);
  }
  return Array.from(dedup.values());
}
