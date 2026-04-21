import { type Ref } from "vue";
import type { ResourceKind } from "../../../constants/resourceKinds";
import type { ResolvedAliasTarget } from "../../../api/kube";

export type DrillSource = { kind: string; name: string; namespace: string | null };

export interface NavigateOptions {
  kind?: ResourceKind;
  customTarget?: ResolvedAliasTarget | null;
  namespace?: string | null;
  nameFilter?: string;
  labelSelector?: string;
  drillFrom?: DrillSource | null;
  reload?: boolean;
  closeDrawer?: boolean;
}

export function useWorkbenchNavigation(opts: {
  selectedKind: Ref<ResourceKind>;
  selectedCustomTarget: Ref<ResolvedAliasTarget | null>;
  selectedNamespace: Ref<string | null>;
  labelSelector: Ref<string>;
  nameFilter: Ref<string>;
  nodeFilter: Ref<string>;
  podIpFilter: Ref<string>;
  kindDropdownOpen: Ref<boolean>;
  detailDrawerVisible: Ref<boolean>;
  selectedRowKeys: Ref<Set<string>>;
  batchDeleteMode: Ref<boolean>;
  drillFrom: Ref<DrillSource | null>;
  loadList: () => void;
}) {
  function navigateTo(nav: NavigateOptions) {
    if (nav.kind !== undefined) opts.selectedKind.value = nav.kind;
    if (nav.customTarget !== undefined) opts.selectedCustomTarget.value = nav.customTarget;
    if (nav.namespace !== undefined) opts.selectedNamespace.value = nav.namespace;
    if ("drillFrom" in nav) opts.drillFrom.value = nav.drillFrom ?? null;
    if (nav.nameFilter !== undefined) opts.nameFilter.value = nav.nameFilter;
    opts.labelSelector.value = nav.labelSelector ?? "";
    opts.nodeFilter.value = "all";
    opts.podIpFilter.value = "";
    opts.kindDropdownOpen.value = false;
    opts.selectedRowKeys.value = new Set();
    opts.batchDeleteMode.value = false;
    if (nav.closeDrawer !== false) opts.detailDrawerVisible.value = false;
    if (nav.reload !== false) opts.loadList();
  }

  return { navigateTo };
}
