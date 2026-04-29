<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { kubeGetResourceGraph, type ResourceGraph, type ResourceGraphNode, type ResourceGraphEdge } from "../api/kube";
import { extractErrorMessage } from "../utils/errorMessage";

export interface ResourceRef {
  kind: string;
  name: string;
  namespace: string | null;
}

const props = defineProps<{
  envId: string | null;
  resource: ResourceRef | null;
}>();

const emit = defineEmits<{
  (e: "navigate", payload: {
    targetKind: string;
    namespace: string | null;
    labelSelector?: string | null;
    resourceName?: string | null;
  }): void;
}>();

const loading = ref(false);
const error = ref<string | null>(null);
const graph = ref<ResourceGraph | null>(null);

/** 关联 kind → 关系标签（简短中文，用于行内说明） */
function formatRelationType(rt: string): string {
  const map: Record<string, string> = {
    volume: "数据卷",
    env_from: "环境引用",
    env_value: "环境变量",
    image_pull_secret: "镜像拉取",
    service_account_ref: "运行身份",
    owner_ref: "从属",
    manages: "管理",
    selector: "选择器",
    service_selector: "服务选择",
    service_pod_match: "对应 Service",
    hpa_target: "伸缩目标",
    bound_volume: "存储绑定",
    storage_class: "存储类",
    ingress_backend: "入口后端",
    role_ref: "角色引用",
    used_by: "被使用",
    routes: "路由",
    scaled_by: "由…伸缩",
  };
  return map[rt] ?? rt;
}

type KindCategory =
  | "workload"
  | "network"
  | "config"
  | "storage"
  | "rbac"
  | "autoscaling"
  | "other";

const CATEGORY_ORDER: KindCategory[] = [
  "workload",
  "network",
  "config",
  "storage",
  "rbac",
  "autoscaling",
  "other",
];

const CATEGORY_META: Record<KindCategory, { title: string; blurb: string; accent: string }> = {
  workload: {
    title: "工作负载与 Pod",
    blurb: "Deployment、Pod、Job 等计算类对象",
    accent: "cat-workload",
  },
  network: {
    title: "服务与网络",
    blurb: "Service、Ingress、端点等流量与发现",
    accent: "cat-network",
  },
  config: {
    title: "配置与密钥",
    blurb: "ConfigMap、Secret 等配置资产",
    accent: "cat-config",
  },
  storage: {
    title: "存储",
    blurb: "卷声明、PV、StorageClass 等",
    accent: "cat-storage",
  },
  rbac: {
    title: "身份与权限",
    blurb: "ServiceAccount、Role、Binding 等",
    accent: "cat-rbac",
  },
  autoscaling: {
    title: "自动伸缩",
    blurb: "HPA 等",
    accent: "cat-autoscaling",
  },
  other: {
    title: "其他",
    blurb: "未归入上述分类的资源",
    accent: "cat-other",
  },
};

const WORKLOAD = new Set([
  "Pod",
  "Deployment",
  "StatefulSet",
  "DaemonSet",
  "ReplicaSet",
  "ReplicationController",
  "Job",
  "CronJob",
]);
const NETWORK = new Set([
  "Service",
  "Endpoints",
  "EndpointSlice",
  "Ingress",
  "NetworkPolicy",
  "IngressClass",
]);
const CONFIG = new Set(["ConfigMap", "Secret"]);
const STORAGE = new Set([
  "PersistentVolumeClaim",
  "PersistentVolume",
  "StorageClass",
  "VolumeAttachment",
]);
const RBAC = new Set([
  "ServiceAccount",
  "Role",
  "ClusterRole",
  "RoleBinding",
  "ClusterRoleBinding",
]);
const AUTOSCALING = new Set(["HorizontalPodAutoscaler", "VerticalPodAutoscaler"]);

function classifyKind(kind: string): KindCategory {
  if (WORKLOAD.has(kind)) return "workload";
  if (NETWORK.has(kind)) return "network";
  if (CONFIG.has(kind)) return "config";
  if (STORAGE.has(kind)) return "storage";
  if (RBAC.has(kind)) return "rbac";
  if (AUTOSCALING.has(kind)) return "autoscaling";
  return "other";
}

interface DisplayItem {
  node: ResourceGraphNode;
  edges: ResourceGraphEdge[];
}

interface CategoryGroup {
  category: KindCategory;
  title: string;
  blurb: string;
  accent: string;
  items: DisplayItem[];
}

/** 当前资源作为边的起点所指向的节点（本资源在规约/卷/环境中等直接引用到）。 */
const upstream = computed<DisplayItem[]>(() => {
  if (!graph.value) return [];
  const rootRef = graph.value.root;
  const outEdges = graph.value.edges.filter(
    (e) => e.from.kind === rootRef.kind && e.from.name === rootRef.name && e.from.namespace === rootRef.namespace
  );
  const targetKeys = new Set(outEdges.map((e) => nodeKey(e.to)));
  return graph.value.nodes
    .filter((n) => n.resource_ref.kind !== rootRef.kind || n.resource_ref.name !== rootRef.name)
    .filter((n) => targetKeys.has(nodeKey(n.resource_ref)))
    .map((n) => ({
      node: n,
      edges: outEdges.filter((e) => nodeKey(e.to) === nodeKey(n.resource_ref)),
    }));
});

/** 当前资源作为边的终点、由其它资源指向的节点（引用本资源）。 */
const downstream = computed<DisplayItem[]>(() => {
  if (!graph.value) return [];
  const rootRef = graph.value.root;
  const inEdges = graph.value.edges.filter(
    (e) => e.to.kind === rootRef.kind && e.to.name === rootRef.name && e.to.namespace === rootRef.namespace
  );
  const sourceKeys = new Set(inEdges.map((e) => nodeKey(e.from)));
  return graph.value.nodes
    .filter((n) => n.resource_ref.kind !== rootRef.kind || n.resource_ref.name !== rootRef.name)
    .filter((n) => sourceKeys.has(nodeKey(n.resource_ref)))
    .map((n) => ({
      node: n,
      edges: inEdges.filter((e) => nodeKey(e.from) === nodeKey(n.resource_ref)),
    }));
});

function edgeDedupeKey(e: ResourceGraphEdge): string {
  return `${nodeKey(e.from)}|${nodeKey(e.to)}|${e.relation_type}`;
}

/**
 * 与当前资源有边相连的所有其它节点（不区分出边/入边），同一边去重、同一对端多关系合并。
 */
const relatedItems = computed((): DisplayItem[] => {
  const byNode = new Map<string, DisplayItem>();
  const seenEdges = new Map<string, Set<string>>();

  function add(fromList: DisplayItem[]) {
    for (const it of fromList) {
      const k = nodeKey(it.node.resource_ref);
      const edgeKeys = seenEdges.get(k) ?? new Set();
      if (!seenEdges.has(k)) seenEdges.set(k, edgeKeys);
      const bucket = byNode.get(k);
      if (!bucket) {
        byNode.set(k, { node: it.node, edges: [...it.edges] });
        for (const e of it.edges) edgeKeys.add(edgeDedupeKey(e));
        continue;
      }
      for (const e of it.edges) {
        const ek = edgeDedupeKey(e);
        if (edgeKeys.has(ek)) continue;
        edgeKeys.add(ek);
        bucket.edges.push(e);
      }
    }
  }

  add(upstream.value);
  add(downstream.value);
  return Array.from(byNode.values());
});

function groupByCategory(items: DisplayItem[]): CategoryGroup[] {
  const buckets = new Map<KindCategory, DisplayItem[]>();
  for (const c of CATEGORY_ORDER) buckets.set(c, []);
  for (const item of items) {
    const k = item.node.resource_ref.kind;
    const cat = classifyKind(k);
    buckets.get(cat)!.push(item);
  }
  const out: CategoryGroup[] = [];
  for (const c of CATEGORY_ORDER) {
    const list = buckets.get(c)!;
    if (!list.length) continue;
    const m = CATEGORY_META[c];
    out.push({
      category: c,
      title: m.title,
      blurb: m.blurb,
      accent: m.accent,
      items: list,
    });
  }
  return out;
}

const relatedGrouped = computed(() => groupByCategory(relatedItems.value));

function nodeKey(ref: { kind: string; name: string; namespace?: string | null; set_id?: string | null }): string {
  const sid = ref.set_id ?? "";
  return `${ref.kind}|${ref.namespace ?? ""}|${ref.name}|${sid}`;
}

function formatNodeLabel(node: ResourceGraphNode): string {
  if (node.display_label) return node.display_label;
  const ref = node.resource_ref;
  if (!node.is_concrete || !ref.name) {
    return ref.name ? `${ref.kind} / ${ref.name}` : `${ref.kind} (group)`;
  }
  return ref.namespace ? `${ref.namespace}/${ref.name}` : ref.name;
}

function formatItemRelations(item: DisplayItem): string {
  const labels = new Set(item.edges.map((e) => formatRelationType(e.relation_type)));
  return Array.from(labels).join("、");
}

function onJump(item: DisplayItem) {
  const ref = item.node.resource_ref;
  emit("navigate", {
    targetKind: ref.kind.toLowerCase() + "s",
    namespace: ref.namespace ?? null,
    labelSelector: item.node.label_selector ?? null,
    resourceName: item.node.is_concrete && ref.name ? ref.name : null,
  });
}

async function fetchGraph() {
  if (!props.envId || !props.resource) return;
  loading.value = true;
  error.value = null;
  graph.value = null;
  try {
    graph.value = await kubeGetResourceGraph(
      props.envId,
      props.resource.kind,
      props.resource.name,
      props.resource.namespace
    );
  } catch (e) {
    error.value = extractErrorMessage(e);
  } finally {
    loading.value = false;
  }
}

watch(
  () => [props.envId, props.resource?.kind, props.resource?.name, props.resource?.namespace] as const,
  () => {
    if (props.resource) fetchGraph();
    else graph.value = null;
  },
  { immediate: true }
);
</script>

<template>
  <div class="topology-panel">
    <div v-if="loading" class="topology-loading">
      <span class="loading-dot" />
      <span>加载中…</span>
    </div>
    <div v-else-if="error" class="topology-error">
      <span class="error-icon">!</span>
      {{ error }}
    </div>
    <template v-else-if="graph">
      <div class="topology-flow">
        <div v-if="relatedGrouped.length" class="category-stack">
            <div
            v-for="group in relatedGrouped"
            :key="`rel-${group.category}`"
            class="category-card"
            :class="group.accent"
          >
            <div class="category-card-head">
              <span class="category-title">{{ group.title }}</span>
              <span class="category-hint">{{ group.blurb }}</span>
              <span class="category-count">{{ group.items.length }}</span>
            </div>
            <ul class="res-list">
              <li
                v-for="(item, i) in group.items"
                :key="`rel-${group.category}-${i}`"
                class="res-item"
                @click="onJump(item)"
              >
                <span class="res-kind">{{ item.node.resource_ref.kind }}</span>
                <span v-if="item.edges.length" class="res-rel">{{ formatItemRelations(item) }}</span>
                <span class="res-name">{{ formatNodeLabel(item.node) }}</span>
              </li>
            </ul>
          </div>
        </div>
      </div>

      <div
        v-if="!relatedItems.length"
        class="topology-empty topology-empty-inline"
      >
        <span class="empty-icon">◇</span>
        <p>暂无其它关联对象</p>
      </div>
    </template>
    <div v-else class="topology-empty">
      <span class="empty-icon">◇</span>
      <p>请选择资源</p>
    </div>
  </div>
</template>

<style scoped>
.topology-panel {
  padding: 0.9rem 1.1rem 1.2rem;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
  background: linear-gradient(180deg, var(--kf-bg-soft, #f8fafc) 0%, var(--kf-surface-strong, #fff) 40%);
  color: var(--kf-text-primary, #0f172a);
}

.topology-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  padding: 2rem;
  font-size: 0.875rem;
  color: var(--kf-text-secondary, #64748b);
}
.loading-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--kf-primary, #2563eb);
  animation: top-pulse 1s ease-in-out infinite;
}
@keyframes top-pulse {
  0%,
  100% {
    opacity: 0.35;
  }
  50% {
    opacity: 1;
  }
}

.topology-error {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.9rem 1rem;
  font-size: 0.875rem;
  color: var(--kf-danger, #dc2626);
  background: var(--kf-danger-soft, #fef2f2);
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--kf-danger) 22%, transparent);
}
.error-icon {
  flex-shrink: 0;
  width: 1.25rem;
  height: 1.25rem;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.75rem;
  font-weight: 700;
  background: var(--kf-danger, #dc2626);
  color: #fff;
  border-radius: 50%;
}

.topology-flow {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}
.category-stack {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.category-card {
  border-radius: 12px;
  border: 1px solid var(--kf-border, #e2e8f0);
  background: var(--kf-surface-strong, #fff);
  overflow: hidden;
  box-shadow: 0 4px 18px color-mix(in srgb, #0f172a 4%, transparent);
}

/* 分组左边线颜色 */
.category-card-head {
  position: relative;
  display: grid;
  grid-template-columns: 1fr auto;
  grid-template-areas:
    "title count"
    "hint count";
  gap: 0.1rem 0.75rem;
  padding: 0.5rem 0.75rem 0.5rem 0.85rem;
  border-bottom: 1px solid color-mix(in srgb, var(--kf-border) 70%, transparent);
  background: color-mix(in srgb, var(--kf-bg-soft) 50%, var(--kf-surface-strong));
}
.category-card.cat-workload .category-card-head {
  box-shadow: inset 3px 0 0 #7c3aed;
}
.category-card.cat-network .category-card-head {
  box-shadow: inset 3px 0 0 #0ea5e9;
}
.category-card.cat-config .category-card-head {
  box-shadow: inset 3px 0 0 #d97706;
}
.category-card.cat-storage .category-card-head {
  box-shadow: inset 3px 0 0 #059669;
}
.category-card.cat-rbac .category-card-head {
  box-shadow: inset 3px 0 0 #c026d3;
}
.category-card.cat-autoscaling .category-card-head {
  box-shadow: inset 3px 0 0 #db2777;
}
.category-card.cat-other .category-card-head {
  box-shadow: inset 3px 0 0 #64748b;
}
.category-title {
  grid-area: title;
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--kf-text-primary, #0f172a);
}
.category-hint {
  grid-area: hint;
  font-size: 0.7rem;
  line-height: 1.4;
  color: var(--kf-text-muted, #8a98ac);
}
.category-count {
  grid-area: count;
  align-self: center;
  min-width: 1.4rem;
  text-align: center;
  font-size: 0.7rem;
  font-weight: 700;
  padding: 0.15rem 0.4rem;
  border-radius: 999px;
  background: var(--kf-primary-soft, #e7efff);
  color: var(--wb-chip-text, #1d4ed8);
  border: 1px solid color-mix(in srgb, var(--kf-border) 60%, transparent);
}

.res-list {
  margin: 0;
  padding: 0.2rem 0 0.35rem;
  list-style: none;
}
.res-item {
  display: flex;
  align-items: center;
  gap: 0.5rem 0.65rem;
  flex-wrap: wrap;
  padding: 0.45rem 0.75rem 0.45rem 0.85rem;
  font-size: 0.8rem;
  transition: background 0.12s ease;
  cursor: pointer;
  border-bottom: 1px solid color-mix(in srgb, var(--kf-border) 40%, transparent);
}
.res-item:last-child {
  border-bottom: none;
}
.res-item:hover {
  background: color-mix(in srgb, var(--kf-primary) 6%, var(--kf-surface-strong));
}
.res-item:hover .res-name {
  color: var(--kf-primary, #2563eb);
}
.res-kind {
  flex-shrink: 0;
  padding: 0.12rem 0.4rem;
  font-size: 0.65rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--wb-text-primary, #334155);
  background: color-mix(in srgb, var(--kf-text-secondary) 12%, var(--kf-surface-strong));
  border-radius: 6px;
  border: 1px solid color-mix(in srgb, var(--kf-border) 50%, transparent);
}
.res-rel {
  flex-shrink: 0;
  font-size: 0.65rem;
  font-weight: 600;
  color: var(--kf-text-secondary, #64748b);
  padding: 0.1rem 0.35rem;
  border-radius: 4px;
  background: var(--kf-bg-soft, #f1f5f9);
}
.res-name {
  flex: 1;
  min-width: 0;
  font-family: ui-monospace, "SF Mono", Monaco, Consolas, monospace;
  font-size: 0.8rem;
  color: var(--wb-text-primary, #1e293b);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.topology-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.5rem;
  padding: 2rem 1rem;
  text-align: center;
}
.empty-icon {
  font-size: 1.5rem;
  color: var(--kf-text-muted, #cbd5e1);
  line-height: 1;
}
.topology-empty p {
  margin: 0;
  font-size: 0.8125rem;
  color: var(--kf-text-secondary, #94a3b8);
}
.topology-empty-inline {
  padding: 1.1rem 1rem;
  flex-direction: row;
  gap: 0.5rem;
  border-radius: 12px;
  background: var(--kf-bg-elevated, #eef2f6);
  border: 1px dashed var(--kf-border, #e2e8f0);
}
.topology-empty-inline .empty-icon {
  font-size: 1.1rem;
}
</style>
