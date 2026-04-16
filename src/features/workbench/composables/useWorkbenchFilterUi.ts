import { computed, type Ref } from "vue";
import type { ResourceKind } from "../../../constants/resourceKinds";
import type { ResolvedAliasTarget } from "../../../api/kube";
import { getWorkbenchResourceDescriptor } from "../resourceDescriptors";

export type ActiveFilterChip = {
  id: "name" | "node" | "podIp" | "label";
  label: string;
  value: string;
};

export type UseWorkbenchFilterUiOptions = {
  selectedKind: Ref<ResourceKind>;
  selectedCustomTarget: Ref<ResolvedAliasTarget | null>;
  watchEnabled: Ref<boolean>;
  nameFilter: Ref<string>;
  nodeFilter: Ref<string>;
  podIpFilter: Ref<string>;
  labelSelector: Ref<string>;
  applyWatch: () => void;
  loadList: () => void | Promise<void>;
};

/**
 * 工作台筛选栏 UI 状态：IP 筛选提示、活跃筛选项与清理动作，按当前资源类型切换行为。
 */
export function useWorkbenchFilterUi(o: UseWorkbenchFilterUiOptions) {
  function supportsIpFilter(): boolean {
    return getWorkbenchResourceDescriptor(o.selectedKind.value).capabilities.supportsIpFilter;
  }

  function ipFilterPlaceholder(): string {
    return o.selectedKind.value === "services" ? "按 Service IP 筛选…" : "按 Pod IP 筛选…";
  }

  function ipFilterTitle(): string {
    return o.selectedKind.value === "services"
      ? "按 Service ClusterIP 包含匹配（前端过滤）"
      : "按 Pod IP 包含匹配（前端过滤）";
  }

  function ipFilterChipLabel(): string {
    return o.selectedKind.value === "services" ? "Service IP" : "Pod IP";
  }

  const activeFilterChips = computed<ActiveFilterChip[]>(() => {
    const chips: ActiveFilterChip[] = [];
    const name = o.nameFilter.value.trim();
    const label = o.labelSelector.value.trim();
    if (name) chips.push({ id: "name", label: "名称", value: name });
    if (o.selectedKind.value === "pods" && o.nodeFilter.value !== "all") {
      chips.push({ id: "node", label: "Node", value: o.nodeFilter.value });
    }
    if (supportsIpFilter()) {
      const podIp = o.podIpFilter.value.trim();
      if (podIp) chips.push({ id: "podIp", label: ipFilterChipLabel(), value: podIp });
    }
    if (label) chips.push({ id: "label", label: "Label", value: label });
    return chips;
  });

  function reloadByCurrentMode() {
    if (
      o.watchEnabled.value &&
      getWorkbenchResourceDescriptor(o.selectedKind.value).capabilities.supportsWatch &&
      !o.selectedCustomTarget.value
    ) {
      o.applyWatch();
    } else {
      void o.loadList();
    }
  }

  function clearFilterChip(id: ActiveFilterChip["id"]) {
    if (id === "name") o.nameFilter.value = "";
    else if (id === "node") o.nodeFilter.value = "all";
    else if (id === "podIp") o.podIpFilter.value = "";
    else if (id === "label") o.labelSelector.value = "";
    reloadByCurrentMode();
  }

  function clearAllFilters() {
    o.nameFilter.value = "";
    o.nodeFilter.value = "all";
    o.podIpFilter.value = "";
    o.labelSelector.value = "";
    reloadByCurrentMode();
  }

  function applyLabelFilterFromToolbar() {
    reloadByCurrentMode();
  }

  function onToolbarClearFilterChip(id: string) {
    clearFilterChip(id as ActiveFilterChip["id"]);
  }

  return {
    supportsIpFilter,
    ipFilterPlaceholder,
    ipFilterTitle,
    activeFilterChips,
    clearFilterChip,
    clearAllFilters,
    applyLabelFilterFromToolbar,
    onToolbarClearFilterChip,
  };
}
