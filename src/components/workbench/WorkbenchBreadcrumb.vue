<script setup lang="ts">
import { computed } from "vue";
import { NBreadcrumb, NBreadcrumbItem, NTag } from "naive-ui";
import type { ResourceKind } from "../../constants/resourceKinds";

const props = defineProps<{
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

/** 钻取路径下，selectedKind 与 drillFrom.kind 一致表示用户回到了"该资源所属类型"列表，
 *  此时不再显示资源名段，最后一段直接展示 drill 资源名以体现选中态。 */
const drillKindMatches = computed(() => {
  if (!props.drillFrom) return false;
  return (props.apiKindToId[props.drillFrom.kind] ?? "services") === props.selectedKind;
});
</script>

<template>
  <nav class="breadcrumb-bar" aria-label="导航">
    <NTag size="small" round :bordered="false" class="breadcrumb-kicker">当前位置</NTag>
    <NBreadcrumb separator="›" class="breadcrumb-trail">
      <template v-if="drillFrom">
        <NBreadcrumbItem clickable @click="emit('drill-namespace')">
          {{ drillFrom.namespace || "全部" }}
        </NBreadcrumbItem>
        <NBreadcrumbItem clickable @click="emit('breadcrumb-kind')">
          {{ drillFrom.kind }}
        </NBreadcrumbItem>
        <NBreadcrumbItem v-if="!drillKindMatches" clickable @click="emit('breadcrumb-resource')">
          {{ drillFrom.name }}
        </NBreadcrumbItem>
        <NBreadcrumbItem :clickable="false" class="breadcrumb-current">
          <NTag type="primary" size="small" :bordered="false" round>
            {{ drillKindMatches ? drillFrom.name : workbenchKindLabel }}
          </NTag>
        </NBreadcrumbItem>
      </template>
      <template v-else>
        <NBreadcrumbItem :clickable="false" class="breadcrumb-base">
          {{ effectiveNamespace }}
        </NBreadcrumbItem>
        <NBreadcrumbItem :clickable="false" class="breadcrumb-current">
          <NTag type="primary" size="small" :bordered="false" round>{{ workbenchKindLabel }}</NTag>
        </NBreadcrumbItem>
      </template>
    </NBreadcrumb>
  </nav>
</template>

<style scoped>
.breadcrumb-bar {
  display: flex;
  align-items: center;
  gap: 0.55rem;
  padding: 0.6rem 1rem;
  font-size: 0.8rem;
  color: var(--wb-text-secondary, #66768f);
  background: linear-gradient(180deg, var(--wb-panel-soft, #f8fbff), var(--wb-panel-elevated, #ffffff));
  border-bottom: 1px solid var(--wb-line, rgba(148, 163, 184, 0.22));
  flex-shrink: 0;
  min-width: 0;
}
.breadcrumb-kicker {
  flex-shrink: 0;
  background: color-mix(in srgb, var(--wb-chip, #e8f0ff) 70%, #eef2ff) !important;
  color: var(--wb-chip-text, #1d4ed8) !important;
  font-weight: 700;
  letter-spacing: 0.03em;
}
.breadcrumb-trail {
  min-width: 0;
  overflow: auto hidden;
  padding-bottom: 0.05rem;
  scrollbar-width: none;
}
.breadcrumb-trail::-webkit-scrollbar {
  display: none;
}
.breadcrumb-trail :deep(.n-breadcrumb-item__separator) {
  color: #94a3b8;
  margin: 0 0.32rem;
}
.breadcrumb-trail :deep(.n-breadcrumb > ul) {
  flex-wrap: nowrap;
}
.breadcrumb-trail :deep(.n-breadcrumb-item) {
  flex-shrink: 0;
  max-width: 260px;
}
.breadcrumb-trail :deep(.n-breadcrumb-item__link) {
  display: inline-flex;
  align-items: center;
  padding: 0.18rem 0.48rem;
  border-radius: 999px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  border: 1px solid var(--wb-line, rgba(148, 163, 184, 0.22));
  background: color-mix(in srgb, var(--wb-panel, #fff) 92%, transparent);
  color: inherit;
  transition: border-color 0.15s ease, color 0.15s ease, background-color 0.15s ease;
}
.breadcrumb-trail :deep(.n-breadcrumb-item__link:hover) {
  background: #fff;
  border-color: #93c5fd;
  color: #1d4ed8;
}
.breadcrumb-base :deep(.n-breadcrumb-item__link) {
  background: transparent;
  border-color: transparent;
  color: var(--wb-text-secondary, #66768f);
  cursor: default;
}
.breadcrumb-base :deep(.n-breadcrumb-item__link:hover) {
  background: transparent;
  border-color: transparent;
  color: var(--wb-text-secondary, #66768f);
}
.breadcrumb-current :deep(.n-breadcrumb-item__link) {
  padding: 0;
  border: none;
  background: transparent;
  cursor: default;
}
.breadcrumb-current :deep(.n-breadcrumb-item__link:hover) {
  background: transparent;
  border: none;
  color: inherit;
}
</style>
