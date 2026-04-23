<script setup lang="ts">
import { ref } from "vue";
import { NButton } from "naive-ui";
import type { ResourceSnapshotItem } from "../stores/resourceSnapshots";
import { formatDateTime } from "../utils/dateFormat";

defineProps<{
  title?: string;
  subtitle?: string;
  currentSummary?: string;
  createLabel?: string;
  emptyText?: string;
  snapshots: ResourceSnapshotItem[];
  creating?: boolean;
}>();

const emit = defineEmits<{
  (e: "create"): void;
  (e: "view", snapshot: ResourceSnapshotItem): void;
  (e: "delete", snapshot: ResourceSnapshotItem): void;
  (e: "toggle-pin", snapshot: ResourceSnapshotItem): void;
}>();

const pendingDeleteId = ref<string | null>(null);

function sourceLabel(source: ResourceSnapshotItem["source"]): string {
  if (source === "manual") return "手动";
  if (source === "before-image-patch") return "镜像变更";
  if (source === "after-image-patch") return "镜像变更后";
  return "应用前";
}

function categoryLabel(category: ResourceSnapshotItem["category"]): string {
  if (category === "config") return "配置";
  if (category === "image") return "镜像";
  return "资源";
}

function onDeleteClick(snapshot: ResourceSnapshotItem) {
  if (pendingDeleteId.value === snapshot.id) {
    pendingDeleteId.value = null;
    emit("delete", snapshot);
    return;
  }
  pendingDeleteId.value = snapshot.id;
}

function clearPendingDelete(snapshotId?: string) {
  if (!snapshotId || pendingDeleteId.value === snapshotId) {
    pendingDeleteId.value = null;
  }
}
</script>

<template>
  <aside class="snapshot-panel">
    <div class="snapshot-head">
      <div>
        <div class="snapshot-title">{{ title || "资源快照" }}</div>
        <p v-if="subtitle" class="snapshot-subtitle">{{ subtitle }}</p>
      </div>
      <NButton type="primary" class="snapshot-create" :disabled="creating" :loading="creating" @click="emit('create')">
        {{ creating ? "生成中…" : createLabel || "生成快照" }}
      </NButton>
    </div>
    <div v-if="currentSummary" class="snapshot-current">
      <span class="snapshot-current-label">当前资源</span>
      <p>{{ currentSummary }}</p>
    </div>
    <div v-if="snapshots.length" class="snapshot-list">
      <div
        v-for="item in snapshots"
        :key="item.id"
        class="snapshot-card"
        :class="{ 'snapshot-card-danger': pendingDeleteId === item.id, 'snapshot-card-pinned': item.pinned }"
        role="button"
        tabindex="0"
        @click="emit('view', item)"
        @mouseleave="clearPendingDelete(item.id)"
        @keydown.enter.prevent="emit('view', item)"
        @keydown.space.prevent="emit('view', item)"
      >
        <div class="snapshot-card-top">
          <span class="snapshot-card-title">{{ item.title }}</span>
          <div class="snapshot-card-actions">
            <span class="snapshot-card-badge kind">{{ categoryLabel(item.category) }}</span>
            <span v-if="item.pinned" class="snapshot-card-badge pinned">已置顶</span>
            <span class="snapshot-card-badge">{{ sourceLabel(item.source) }}</span>
            <NButton
              quaternary
              size="tiny"
              class="snapshot-pin"
              :class="{ active: item.pinned }"
              :aria-label="item.pinned ? '取消置顶快照' : '置顶快照'"
              :title="item.pinned ? '取消置顶快照' : '置顶快照'"
              @click.stop="emit('toggle-pin', item)"
            >
              {{ item.pinned ? "★" : "☆" }}
            </NButton>
            <NButton
              quaternary
              size="tiny"
              class="snapshot-delete"
              :class="{ confirm: pendingDeleteId === item.id }"
              aria-label="删除快照"
              :title="pendingDeleteId === item.id ? '再次点击确认删除' : '删除快照'"
              @click.stop="onDeleteClick(item)"
              @blur="clearPendingDelete(item.id)"
            >
              {{ pendingDeleteId === item.id ? "确认" : "×" }}
            </NButton>
          </div>
        </div>
        <p class="snapshot-card-summary">{{ item.summary }}</p>
        <div class="snapshot-card-meta">
          <span>{{ formatDateTime(item.created_at) }}</span>
          <span>查看</span>
        </div>
      </div>
    </div>
    <div v-else class="snapshot-empty">
      {{ emptyText || "还没有快照，先保存一个当前资源快照。" }}
    </div>
  </aside>
</template>

<style scoped>
.snapshot-panel {
  width: 280px;
  min-width: 280px;
  max-width: 320px;
  border-left: 1px solid #e2e8f0;
  background:
    radial-gradient(circle at top, rgba(14, 165, 233, 0.08), transparent 38%),
    linear-gradient(180deg, #fcfdff 0%, #f8fafc 100%);
  display: flex;
  flex-direction: column;
}
.snapshot-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 1rem;
  border-bottom: 1px solid #e2e8f0;
}
.snapshot-title {
  font-size: 0.875rem;
  font-weight: 700;
  color: #0f172a;
}
.snapshot-subtitle {
  margin: 0.35rem 0 0;
  font-size: 0.75rem;
  line-height: 1.5;
  color: #64748b;
}
.snapshot-create {
  flex-shrink: 0;
  padding: 0.45rem 0.75rem;
  border: 1px solid #bfdbfe;
  border-radius: 999px;
  background: #eff6ff;
  color: #1d4ed8;
  font-size: 0.75rem;
  font-weight: 600;
  cursor: pointer;
}
.snapshot-create:hover:not(:disabled) {
  background: #dbeafe;
}
.snapshot-create:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.snapshot-current {
  margin: 1rem;
  padding: 0.85rem 0.9rem;
  border: 1px solid #dbeafe;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.88);
}
.snapshot-current-label {
  display: inline-block;
  margin-bottom: 0.35rem;
  padding: 0.15rem 0.45rem;
  border-radius: 999px;
  background: #dbeafe;
  color: #1d4ed8;
  font-size: 0.6875rem;
  font-weight: 700;
}
.snapshot-current p {
  margin: 0;
  font-size: 0.75rem;
  line-height: 1.5;
  color: #334155;
}
.snapshot-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 0 1rem 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}
.snapshot-card {
  width: 100%;
  padding: 0.9rem;
  border: 1px solid #d7e1ee;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.92);
  text-align: left;
  cursor: pointer;
  transition: transform 0.16s ease, border-color 0.16s ease, box-shadow 0.16s ease;
}
.snapshot-card:hover {
  transform: translateY(-1px);
  border-color: #93c5fd;
  box-shadow: 0 12px 24px rgba(148, 163, 184, 0.14);
}
.snapshot-card-danger {
  border-color: #fca5a5;
  box-shadow: 0 12px 24px rgba(248, 113, 113, 0.12);
}
.snapshot-card-pinned {
  border-color: #fbbf24;
  box-shadow: 0 12px 24px rgba(251, 191, 36, 0.12);
  background:
    linear-gradient(180deg, rgba(255, 251, 235, 0.96), rgba(255, 255, 255, 0.96));
}
.snapshot-card-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}
.snapshot-card-actions {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
}
.snapshot-card-title {
  font-size: 0.8125rem;
  font-weight: 700;
  color: #0f172a;
}
.snapshot-card-badge {
  flex-shrink: 0;
  padding: 0.15rem 0.45rem;
  border-radius: 999px;
  background: #f1f5f9;
  color: #475569;
  font-size: 0.6875rem;
}
.snapshot-card-badge.kind {
  background: #dbeafe;
  color: #1d4ed8;
}
.snapshot-card-badge.pinned {
  background: #fef3c7;
  color: #b45309;
}
.snapshot-pin {
  width: 1.4rem;
  height: 1.4rem;
  border: none;
  border-radius: 999px;
  background: #f8fafc;
  color: #64748b;
  font-size: 0.95rem;
  line-height: 1;
  cursor: pointer;
}
.snapshot-pin:hover {
  background: #fef3c7;
  color: #b45309;
}
.snapshot-pin.active {
  background: #fef3c7;
  color: #b45309;
}
.snapshot-delete {
  width: 1.4rem;
  height: 1.4rem;
  border: none;
  border-radius: 999px;
  background: #fee2e2;
  color: #b91c1c;
  font-size: 0.95rem;
  line-height: 1;
  cursor: pointer;
}
.snapshot-delete:hover {
  background: #fecaca;
}
.snapshot-delete.confirm {
  width: auto;
  min-width: 2.7rem;
  padding: 0 0.45rem;
  background: #dc2626;
  color: #fff;
  font-size: 0.6875rem;
  font-weight: 700;
}
.snapshot-delete.confirm:hover {
  background: #b91c1c;
}
.snapshot-card-summary {
  margin: 0.6rem 0 0.75rem;
  font-size: 0.75rem;
  line-height: 1.5;
  color: #475569;
}
.snapshot-card-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  font-size: 0.6875rem;
  color: #64748b;
}
.snapshot-empty {
  margin: 1rem;
  padding: 1rem;
  border: 1px dashed #cbd5e1;
  border-radius: 14px;
  font-size: 0.75rem;
  line-height: 1.6;
  color: #64748b;
  background: rgba(255, 255, 255, 0.72);
}

@media (max-width: 960px) {
  .snapshot-panel {
    width: auto;
    min-width: 0;
    max-width: none;
    border-left: none;
    border-top: 1px solid #e2e8f0;
  }
  .snapshot-list {
    max-height: 220px;
  }
}
</style>
