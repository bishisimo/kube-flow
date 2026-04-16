<script setup lang="ts">
import type { ResourceKind } from "../../constants/resourceKinds";
defineProps<{
  drillFrom: { kind: string; name: string; namespace: string | null } | null;
  apiKindToId: Record<string, string>;
  workbenchKindLabel: string;
  effectiveNamespace: string;
  selectedKind: ResourceKind;
}>();

const emit = defineEmits<{
  "drill-namespace": [];
  "breadcrumb-kind": [];
  "breadcrumb-resource": [];
}>();
</script>

<template>
  <nav class="breadcrumb-bar" aria-label="导航">
    <span class="breadcrumb-kicker">当前位置</span>
    <div class="breadcrumb-trail">
      <template v-if="drillFrom">
        <button
          type="button"
          class="breadcrumb-seg"
          :title="drillFrom.namespace || '全部'"
          @click="emit('drill-namespace')"
        >
          {{ drillFrom.namespace || "全部" }}
        </button>
        <span class="breadcrumb-sep">›</span>
        <button type="button" class="breadcrumb-seg" :title="drillFrom.kind" @click="emit('breadcrumb-kind')">
          {{ drillFrom.kind }}
        </button>
        <span class="breadcrumb-sep">›</span>
        <button
          v-if="(apiKindToId[drillFrom.kind] ?? 'services') !== selectedKind"
          type="button"
          class="breadcrumb-seg"
          :title="drillFrom.name"
          @click="emit('breadcrumb-resource')"
        >
          {{ drillFrom.name }}
        </button>
        <template v-if="(apiKindToId[drillFrom.kind] ?? 'services') !== selectedKind">
          <span class="breadcrumb-sep">›</span>
          <span class="breadcrumb-seg breadcrumb-current" :title="workbenchKindLabel">{{ workbenchKindLabel }}</span>
        </template>
        <span v-else class="breadcrumb-seg breadcrumb-current" :title="drillFrom.name">{{ drillFrom.name }}</span>
      </template>
      <template v-else>
        <span class="breadcrumb-seg breadcrumb-base" :title="effectiveNamespace">{{ effectiveNamespace }}</span>
        <span class="breadcrumb-sep">›</span>
        <span class="breadcrumb-seg breadcrumb-current" :title="workbenchKindLabel">{{ workbenchKindLabel }}</span>
      </template>
    </div>
  </nav>
</template>

<style scoped>
.breadcrumb-bar {
  display: flex;
  align-items: center;
  gap: 0.55rem;
  padding: 0.55rem 1rem;
  font-size: 0.8rem;
  color: #64748b;
  background:
    linear-gradient(180deg, #f8fafc, #f1f5f9);
  border-bottom: 1px solid #e2e8f0;
  flex-shrink: 0;
  min-width: 0;
}
.breadcrumb-kicker {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  padding: 0.14rem 0.45rem;
  border-radius: 999px;
  background: #e2e8f0;
  color: #475569;
  font-size: 0.68rem;
  font-weight: 700;
  letter-spacing: 0.03em;
}
.breadcrumb-trail {
  display: flex;
  align-items: center;
  gap: 0.28rem;
  min-width: 0;
  overflow: auto hidden;
  padding-bottom: 0.05rem;
  scrollbar-width: none;
}
.breadcrumb-trail::-webkit-scrollbar {
  display: none;
}
.breadcrumb-seg {
  display: inline-flex;
  align-items: center;
  max-width: 260px;
  background: rgba(255, 255, 255, 0.9);
  border: 1px solid rgba(203, 213, 225, 0.9);
  padding: 0.28rem 0.55rem;
  border-radius: 999px;
  cursor: pointer;
  color: inherit;
  font: inherit;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  box-shadow: 0 1px 1px rgba(15, 23, 42, 0.02);
}
.breadcrumb-seg:hover:not(.breadcrumb-current):not(.breadcrumb-base) {
  background: #fff;
  border-color: #93c5fd;
  color: #1d4ed8;
}
.breadcrumb-seg.breadcrumb-base {
  background: transparent;
  border-color: transparent;
  color: #64748b;
  cursor: default;
  box-shadow: none;
}
.breadcrumb-seg.breadcrumb-current {
  background: #dbeafe;
  border-color: #bfdbfe;
  color: #0f172a;
  font-weight: 700;
  cursor: default;
}
.breadcrumb-sep {
  color: #94a3b8;
  user-select: none;
  flex-shrink: 0;
}
</style>
