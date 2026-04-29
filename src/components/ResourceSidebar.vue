<script setup lang="ts">
import { ref } from "vue";
import { NButton } from "naive-ui";
import { RESOURCE_GROUPS, type ResourceKind } from "../constants/resourceKinds";

defineProps<{
  selectedKind: ResourceKind;
}>();

const emit = defineEmits<{
  (e: "update:selectedKind", v: ResourceKind): void;
}>();

const expandedGroups = ref<Set<string>>(new Set(RESOURCE_GROUPS.map((g) => g.id)));

function toggleGroup(id: string) {
  const next = new Set(expandedGroups.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  expandedGroups.value = next;
}

function selectKind(kind: ResourceKind) {
  emit("update:selectedKind", kind);
}
</script>

<template>
  <aside class="resource-sidebar">
    <section class="section">
      <h3 class="section-title">资源类型</h3>
      <div class="group-list">
        <div v-for="group in RESOURCE_GROUPS" :key="group.id" class="group">
          <NButton
            quaternary
            block
            class="group-header"
            :class="{ expanded: expandedGroups.has(group.id) }"
            @click="toggleGroup(group.id)"
          >
            <span class="group-chevron">{{ expandedGroups.has(group.id) ? "▼" : "▶" }}</span>
            {{ group.label }}
          </NButton>
          <ul v-show="expandedGroups.has(group.id)" class="kind-list">
            <li
              v-for="k in group.kinds"
              :key="k.id"
              class="kind-item"
              :class="{ active: selectedKind === k.id }"
              @click="selectKind(k.id)"
            >
              {{ k.label }}
            </li>
          </ul>
        </div>
      </div>
    </section>
  </aside>
</template>

<style scoped>
.resource-sidebar {
  width: 200px;
  min-width: 200px;
  border-right: 1px solid var(--border-color, #e2e8f0);
  background: var(--sidebar-bg, #f8fafc);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.section {
  padding: 0.75rem 0;
  border-bottom: 1px solid var(--border-color, #e2e8f0);
}
.section-title {
  margin: 0 0 0.5rem 0;
  padding: 0 0.75rem;
  font-size: 0.6875rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: #64748b;
}
.kind-list {
  list-style: none;
  margin: 0;
  padding: 0;
}
.kind-item {
  padding: 0.4rem 0.75rem;
  font-size: 0.8125rem;
  cursor: pointer;
  transition: background 0.1s;
}
.kind-item:hover {
  background: rgba(0, 0, 0, 0.04);
}
.kind-item.active {
  background: rgba(37, 99, 235, 0.1);
  color: #2563eb;
  font-weight: 500;
}
.group-list {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}
.group {
  border-radius: 4px;
}
.group-header {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  width: 100%;
  padding: 0.35rem 0.75rem;
  font-size: 0.75rem;
  font-weight: 600;
  color: #64748b;
  background: none;
  border: none;
  cursor: pointer;
  text-align: left;
  transition: background 0.1s;
}
.group-header:hover {
  background: rgba(0, 0, 0, 0.04);
}
.group-chevron {
  font-size: 0.6rem;
  opacity: 0.8;
}
.group .kind-list {
  padding-left: 0.5rem;
  border-left: 1px solid var(--border-color, #e2e8f0);
  margin-left: 0.75rem;
}
</style>
