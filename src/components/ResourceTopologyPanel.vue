<script setup lang="ts">
import { ref, watch } from "vue";
import { kubeGetResourceTopology, type TopologyItem, type ResourceTopology } from "../api/kube";

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
const topology = ref<ResourceTopology | null>(null);

function formatItem(item: TopologyItem): string {
  if (!item.is_concrete || !item.name) return item.label;
  return item.namespace ? `${item.namespace}/${item.name}` : item.name;
}

function onJump(item: TopologyItem) {
  emit("navigate", {
    targetKind: item.target_kind,
    namespace: item.namespace ?? null,
    labelSelector: item.label_selector ?? null,
    resourceName: item.is_concrete && item.name ? item.name : (item.resource_name ?? null),
  });
}

async function fetchTopology() {
  if (!props.envId || !props.resource) return;
  loading.value = true;
  error.value = null;
  topology.value = null;
  try {
    topology.value = await kubeGetResourceTopology(
      props.envId,
      props.resource.kind,
      props.resource.name,
      props.resource.namespace
    );
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
}

watch(
  () => [props.envId, props.resource?.kind, props.resource?.name, props.resource?.namespace] as const,
  () => {
    if (props.resource) fetchTopology();
    else topology.value = null;
  },
  { immediate: true }
);
</script>

<template>
  <div class="topology-panel">
    <div v-if="loading" class="topology-loading">
      <span class="loading-dot"></span>
      <span>加载中…</span>
    </div>
    <div v-else-if="error" class="topology-error">
      <span class="error-icon">!</span>
      {{ error }}
    </div>
    <template v-else-if="topology">
      <div class="topology-flow">
        <section v-if="topology.upstream.length" class="topology-section topology-upstream">
          <div class="section-header">
            <span class="section-badge">引用</span>
            <span class="section-count">{{ topology.upstream.length }}</span>
          </div>
          <ul class="topology-list">
            <li
              v-for="(item, i) in topology.upstream"
              :key="`u-${i}`"
              class="topology-item topology-item-clickable"
              @click="onJump(item)"
            >
              <span class="item-kind-badge">{{ item.kind }}</span>
              <span v-if="item.relation_type" class="item-relation-badge">{{ item.relation_type }}</span>
              <span class="item-name">{{ formatItem(item) }}</span>
            </li>
          </ul>
        </section>

        <div v-if="resource" class="topology-current">
          <div class="current-card">
            <span class="current-kind">{{ resource.kind }}</span>
            <span class="current-name">{{ resource.namespace ? `${resource.namespace}/` : "" }}{{ resource.name }}</span>
          </div>
        </div>

        <section v-if="topology.downstream.length" class="topology-section topology-downstream">
          <div class="section-header">
            <span class="section-badge">管理</span>
            <span class="section-count">{{ topology.downstream.length }}</span>
          </div>
          <ul class="topology-list">
            <li
              v-for="(item, i) in topology.downstream"
              :key="`d-${i}`"
              class="topology-item topology-item-clickable"
              @click="onJump(item)"
            >
              <span class="item-kind-badge">{{ item.kind }}</span>
              <span v-if="item.relation_type" class="item-relation-badge">{{ item.relation_type }}</span>
              <span class="item-name">{{ formatItem(item) }}</span>
            </li>
          </ul>
        </section>
      </div>

      <div
        v-if="!topology.upstream.length && !topology.downstream.length"
        class="topology-empty topology-empty-inline"
      >
        <span class="empty-icon">◇</span>
        <p>暂无关联资源</p>
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
  padding: 1rem 1.25rem;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
  background: #fafbfc;
}

.topology-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  padding: 2rem;
  font-size: 0.875rem;
  color: #64748b;
}
.loading-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #94a3b8;
  animation: pulse 1s ease-in-out infinite;
}
@keyframes pulse {
  0%, 100% { opacity: 0.4; }
  50% { opacity: 1; }
}

.topology-error {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 1rem 1.25rem;
  font-size: 0.875rem;
  color: #dc2626;
  background: #fef2f2;
  border-radius: 8px;
  border: 1px solid #fecaca;
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
  background: #dc2626;
  color: #fff;
  border-radius: 50%;
}

.topology-flow {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.topology-section {
  background: #fff;
  border-radius: 8px;
  border: 1px solid #e2e8f0;
  overflow: hidden;
}
.topology-upstream {
  border-left: 3px solid #0ea5e9;
}
.topology-downstream {
  border-left: 3px solid #22c55e;
}

.section-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 0.75rem;
  background: #f8fafc;
  border-bottom: 1px solid #e2e8f0;
  font-size: 0.8125rem;
}
.section-badge {
  font-weight: 600;
  color: #475569;
}
.section-count {
  color: #94a3b8;
  font-size: 0.75rem;
}

.topology-list {
  margin: 0;
  padding: 0.25rem 0;
  list-style: none;
}

.topology-item {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.5rem 0.75rem;
  font-size: 0.8125rem;
  transition: background 0.15s;
}
.topology-item:not(:last-child) {
  border-bottom: 1px solid #f1f5f9;
}
.topology-item-clickable {
  cursor: pointer;
}
.topology-item-clickable:hover {
  background: #eff6ff;
}
.topology-item-clickable:hover .item-name {
  color: #2563eb;
}

.item-kind-badge {
  flex-shrink: 0;
  padding: 0.15rem 0.5rem;
  font-size: 0.6875rem;
  font-weight: 600;
  color: #475569;
  background: #e2e8f0;
  border-radius: 4px;
  letter-spacing: 0.02em;
}
.item-relation-badge {
  flex-shrink: 0;
  padding: 0.1rem 0.4rem;
  font-size: 0.625rem;
  font-weight: 500;
  color: #64748b;
  background: #f1f5f9;
  border-radius: 3px;
  letter-spacing: 0.02em;
}

.item-name {
  flex: 1;
  min-width: 0;
  font-family: ui-monospace, "SF Mono", Monaco, Consolas, monospace;
  font-size: 0.8125rem;
  color: #334155;
  overflow: hidden;
  text-overflow: ellipsis;
}

.topology-current {
  display: flex;
  justify-content: center;
  padding: 0.5rem 0;
}
.current-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.25rem;
  padding: 0.75rem 1.25rem;
  background: linear-gradient(135deg, #f0f9ff 0%, #e0f2fe 100%);
  border: 1px solid #bae6fd;
  border-radius: 8px;
  box-shadow: 0 1px 2px rgba(14, 165, 233, 0.08);
}
.current-kind {
  font-size: 0.6875rem;
  font-weight: 600;
  color: #0369a1;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}
.current-name {
  font-family: ui-monospace, "SF Mono", Monaco, Consolas, monospace;
  font-size: 0.875rem;
  font-weight: 500;
  color: #0c4a6e;
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
  color: #cbd5e1;
  line-height: 1;
}
.topology-empty p {
  margin: 0;
  font-size: 0.8125rem;
  color: #94a3b8;
}
.topology-empty-inline {
  padding: 1rem;
  flex-direction: row;
  gap: 0.5rem;
}
.topology-empty-inline .empty-icon {
  font-size: 1rem;
}
</style>
