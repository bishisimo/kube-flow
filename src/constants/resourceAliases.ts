import type { ResourceKind } from "./resourceKinds";

/**
 * 内置资源类型的 kubectl 风格短名（离线即可用，不依赖集群 discovery）。
 * 与 apiserver 返回的 shortNames 大体一致，便于无缓存时仍能搜索。
 */
export const RESOURCE_KIND_BUILTIN_ALIASES: Record<ResourceKind, readonly string[]> = {
  namespaces: ["ns", "namespace"],
  nodes: ["no", "node"],
  pods: ["po"],
  deployments: ["deploy", "deployment"],
  replicasets: ["rs"],
  statefulsets: ["sts"],
  daemonsets: ["ds"],
  jobs: ["job"],
  cronjobs: ["cj", "cronjob"],
  services: ["svc", "service"],
  endpoints: ["ep"],
  endpointslices: ["endpointslice", "eps"],
  ingresses: ["ing"],
  ingressclasses: ["ingressclass"],
  networkpolicies: ["netpol"],
  configmaps: ["cm"],
  secrets: ["secret"],
  persistentvolumeclaims: ["pvc"],
  persistentvolumes: ["pv"],
  storageclasses: ["sc"],
  serviceaccounts: ["sa"],
  roles: ["role"],
  rolebindings: ["rolebinding"],
  clusterroles: ["clusterrole", "cr"],
  clusterrolebindings: ["clusterrolebinding", "crb"],
  resourcequotas: ["quota"],
  limitranges: ["limits"],
  priorityclasses: ["pc"],
  horizontalpodautoscalers: ["hpa"],
  poddisruptionbudgets: ["pdb"],
};

/** 资源类型下拉：按展示名、id 或 kubectl 短名筛选 */
export function resourceKindMatchesSearch(id: ResourceKind, label: string, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  if (label.toLowerCase().includes(q)) return true;
  if (id.toLowerCase().includes(q)) return true;
  const aliases = RESOURCE_KIND_BUILTIN_ALIASES[id];
  return aliases.some((a) => a === q || a.startsWith(q));
}
