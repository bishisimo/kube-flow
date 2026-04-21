import type { Ref } from "vue";
import type { ResourceKind } from "../../../constants/resourceKinds";
import type { NavigateOptions } from "./useWorkbenchNavigation";

export function useWorkbenchDrillNavigation(opts: {
  navigateTo: (nav: NavigateOptions) => void;
  selectedKind: Ref<ResourceKind>;
  selectedNamespace: Ref<string | null>;
  resourceKinds: { id: ResourceKind; label: string }[];
}) {
  const { navigateTo, selectedKind, selectedNamespace, resourceKinds } = opts;

  function isCellDrillable(colKey: string, row: Record<string, unknown>): boolean {
    if (colKey === "replicas" && row.labelSelector) return true;
    if (colKey === "roleRef" && row.roleRefName && row.roleRef !== "-") return true;
    if (colKey === "subjects" && Array.isArray(row.subjectsList) && (row.subjectsList as unknown[]).length > 0) return true;
    if (selectedKind.value === "persistentvolumeclaims") {
      if (colKey === "volume") {
        const v = row.volume;
        return typeof v === "string" && v !== "" && v !== "-";
      }
      if (colKey === "storageClass") {
        const s = row.storageClass;
        return typeof s === "string" && s !== "" && s !== "-";
      }
    }
    return false;
  }

  function onRoleRefClick(row: Record<string, unknown>) {
    const kind = row.roleRefKind as string;
    const name = row.roleRefName as string;
    if (!kind || !name) return;
    const isRole = kind === "Role";
    navigateTo({
      kind: isRole ? "roles" : "clusterroles",
      namespace: isRole ? ((row.ns as string) ?? selectedNamespace.value) : null,
      nameFilter: name,
      drillFrom: {
        kind: isRole ? "RoleBinding" : "ClusterRoleBinding",
        name: String(row.name),
        namespace: (row.ns as string) ?? null,
      },
    });
  }

  function getSubjectsList(row: Record<string, unknown>): { kind: string; name: string; namespace?: string | null }[] {
    const list = row.subjectsList;
    return Array.isArray(list) ? (list as { kind: string; name: string; namespace?: string | null }[]) : [];
  }

  function getSubjectLabel(s: { kind: string; name: string; namespace?: string | null }, row: Record<string, unknown>): string {
    const ns = s.namespace ?? (row.ns as string) ?? "default";
    return `${ns}/${s.name}`;
  }

  function onSubjectClick(row: Record<string, unknown>, subject: { kind: string; name: string; namespace?: string | null }) {
    if (subject.kind !== "ServiceAccount") return;
    const sourceKind = selectedKind.value === "clusterrolebindings" ? "ClusterRoleBinding" : "RoleBinding";
    navigateTo({
      kind: "serviceaccounts",
      namespace: subject.namespace ?? (row.ns as string) ?? selectedNamespace.value,
      nameFilter: subject.name,
      drillFrom: { kind: sourceKind, name: String(row.name), namespace: (row.ns as string) ?? null },
    });
  }

  function getSubjectLabelForTable(s: unknown, row: Record<string, unknown>): string {
    return getSubjectLabel(s as { kind: string; name: string; namespace?: string | null }, row);
  }

  function onSubjectClickForTable(row: Record<string, unknown>, s: unknown) {
    onSubjectClick(row, s as { kind: string; name: string; namespace?: string | null });
  }

  function onPvcCellClick(row: Record<string, unknown>, colKey: string) {
    if (colKey === "volume") {
      const name = row.volume as string;
      if (!name || name === "-") return;
      navigateTo({
        kind: "persistentvolumes",
        namespace: null,
        nameFilter: name,
        drillFrom: { kind: "PersistentVolumeClaim", name: String(row.name), namespace: (row.ns as string) ?? null },
      });
    } else if (colKey === "storageClass") {
      const name = row.storageClass as string;
      if (!name || name === "-") return;
      navigateTo({
        kind: "storageclasses",
        namespace: null,
        nameFilter: name,
        drillFrom: { kind: "PersistentVolumeClaim", name: String(row.name), namespace: (row.ns as string) ?? null },
      });
    }
  }

  function onReplicasClick(row: Record<string, unknown>) {
    const ls = row.labelSelector as string | null | undefined;
    if (!ls || !row.name) return;
    const kindLabel = resourceKinds.find((k) => k.id === selectedKind.value)?.label ?? "";
    if (!kindLabel) return;
    navigateTo({
      kind: "pods",
      namespace: (row.ns as string) ?? selectedNamespace.value,
      labelSelector: ls,
      nameFilter: "",
      drillFrom: { kind: kindLabel, name: String(row.name), namespace: (row.ns as string) ?? null },
    });
  }

  return {
    isCellDrillable,
    onRoleRefClick,
    getSubjectsList,
    getSubjectLabel,
    getSubjectLabelForTable,
    onSubjectClick,
    onSubjectClickForTable,
    onPvcCellClick,
    onReplicasClick,
  };
}
