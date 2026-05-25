<script setup lang="ts">
import { ref } from "vue";
import { NButton, NScrollbar } from "naive-ui";
import type { ResourceSnapshotItem } from "../../stores/resourceSnapshots";
import { formatSnapshotResourceRef } from "../../stores/resourceSnapshots";
import { formatDateTime } from "../../utils/dateFormat";

defineProps<{
  snapshots: ResourceSnapshotItem[];
  selectedId: string | null;
  envNameMap?: Record<string, string>;
  emptyText?: string;
}>();

const emit = defineEmits<{
  (e: "select", snapshot: ResourceSnapshotItem): void;
  (e: "delete", snapshot: ResourceSnapshotItem): void;
  (e: "toggle-pin", snapshot: ResourceSnapshotItem): void;
}>();

const pendingDeleteId = ref<string | null>(null);

function sourceLabel(source: ResourceSnapshotItem["source"]): string {
  if (source === "manual") return "手动";
  if (source === "before-image-patch") return "镜像变更";
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
  <div class="sc-list">
    <div class="sc-list-head">
      <span class="sc-list-count">{{ snapshots.length }} 条快照</span>
    </div>
    <NScrollbar v-if="snapshots.length" class="sc-list-scroll" trigger="hover">
      <div class="sc-list-items">
        <button
          v-for="item in snapshots"
          :key="item.id"
          type="button"
          class="sc-list-item"
          :class="{
            active: selectedId === item.id,
            pinned: item.pinned,
            danger: pendingDeleteId === item.id,
          }"
          @click="emit('select', item)"
          @mouseleave="clearPendingDelete(item.id)"
        >
          <div class="sc-list-item-top">
            <span class="sc-list-item-title">{{ item.title }}</span>
            <div class="sc-list-item-actions">
              <NButton
                quaternary
                size="tiny"
                class="sc-list-pin"
                :class="{ active: item.pinned }"
                :title="item.pinned ? '取消置顶' : '置顶'"
                @click.stop="emit('toggle-pin', item)"
              >{{ item.pinned ? "★" : "☆" }}</NButton>
              <NButton
                quaternary
                size="tiny"
                class="sc-list-delete"
                :class="{ confirm: pendingDeleteId === item.id }"
                :title="pendingDeleteId === item.id ? '再次点击确认删除' : '删除'"
                @click.stop="onDeleteClick(item)"
                @blur="clearPendingDelete(item.id)"
              >{{ pendingDeleteId === item.id ? "确认" : "×" }}</NButton>
            </div>
          </div>
          <div class="sc-list-item-badges">
            <span v-if="envNameMap?.[item.env_id]" class="sc-badge env">{{ envNameMap[item.env_id] }}</span>
            <span class="sc-badge kind">{{ categoryLabel(item.category) }}</span>
            <span class="sc-badge">{{ sourceLabel(item.source) }}</span>
            <span v-if="item.pinned" class="sc-badge pinned">置顶</span>
          </div>
          <p class="sc-list-item-resource">{{ formatSnapshotResourceRef(item) }}</p>
          <p class="sc-list-item-summary">{{ item.summary }}</p>
          <span class="sc-list-item-time">{{ formatDateTime(item.created_at) }}</span>
        </button>
      </div>
    </NScrollbar>
    <div v-else class="sc-list-empty">
      {{ emptyText || "没有匹配的快照。" }}
    </div>
  </div>
</template>

<style scoped>
.sc-list {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--kf-surface-strong);
  border-right: 1px solid var(--kf-border);
}
.sc-list-head {
  flex-shrink: 0;
  padding: 0.65rem 0.85rem;
  border-bottom: 1px solid var(--kf-border);
}
.sc-list-count {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--kf-text-secondary);
}
.sc-list-scroll {
  flex: 1;
  min-height: 0;
}
.sc-list-items {
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
  padding: 0.55rem;
}
.sc-list-item {
  width: 100%;
  margin: 0;
  padding: 0.7rem 0.75rem 0.7rem 0.85rem;
  border: 1px solid var(--kf-border);
  border-radius: 10px;
  background: color-mix(in srgb, var(--kf-text-primary) 4%, var(--kf-mix-surface));
  text-align: left;
  cursor: pointer;
  transition: border-color 0.15s ease, background 0.15s ease, box-shadow 0.15s ease;
  position: relative;
}
.sc-list-item::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0.45rem;
  bottom: 0.45rem;
  width: 3px;
  border-radius: 0 3px 3px 0;
  background: transparent;
  transition: background 0.15s ease;
}
.sc-list-item:hover {
  border-color: color-mix(in srgb, var(--kf-primary) 42%, var(--kf-border));
  background: var(--kf-surface-strong);
}
.sc-list-item.active {
  border-color: color-mix(in srgb, var(--kf-primary) 55%, var(--kf-border));
  background: var(--kf-surface-strong);
  box-shadow: var(--kf-shadow-sm);
}
.sc-list-item.active::before {
  background: var(--kf-primary);
}
.sc-list-item.pinned {
  background: linear-gradient(
    180deg,
    color-mix(in srgb, var(--kf-warning) 12%, var(--kf-surface-strong)),
    var(--kf-surface-strong)
  );
}
.sc-list-item.danger {
  border-color: color-mix(in srgb, var(--kf-danger) 55%, var(--kf-border));
}
.sc-list-item-top {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.35rem;
}
.sc-list-item-title {
  font-size: 0.8125rem;
  font-weight: 700;
  color: var(--kf-text-primary);
  line-height: 1.35;
}
.sc-list-item-actions {
  display: inline-flex;
  align-items: center;
  gap: 0.15rem;
  flex-shrink: 0;
  opacity: 0;
  transition: opacity 0.15s ease;
}
.sc-list-item:hover .sc-list-item-actions,
.sc-list-item.active .sc-list-item-actions,
.sc-list-item.danger .sc-list-item-actions {
  opacity: 1;
}
.sc-list-pin,
.sc-list-delete {
  width: 1.35rem;
  height: 1.35rem;
  padding: 0 !important;
  font-size: 0.85rem;
}
.sc-list-pin.active {
  color: color-mix(in srgb, var(--kf-warning) 78%, var(--kf-text-primary));
}
.sc-list-delete.confirm {
  width: auto;
  min-width: 2.4rem;
  padding: 0 0.35rem !important;
  color: #fff !important;
  background: var(--kf-danger) !important;
  font-size: 0.625rem;
  font-weight: 700;
}
.sc-list-item-badges {
  display: flex;
  flex-wrap: wrap;
  gap: 0.3rem;
  margin-top: 0.4rem;
}
.sc-badge {
  padding: 0.1rem 0.4rem;
  border-radius: 999px;
  font-size: 0.625rem;
  font-weight: 600;
  background: color-mix(in srgb, var(--kf-text-primary) 8%, var(--kf-mix-surface));
  color: var(--kf-text-secondary);
}
.sc-badge.env {
  background: color-mix(in srgb, var(--kf-text-primary) 10%, var(--kf-mix-surface));
  color: var(--kf-text-primary);
}
.sc-badge.kind {
  background: var(--kf-primary-soft);
  color: color-mix(in srgb, var(--kf-primary) 82%, var(--kf-text-primary));
}
.sc-badge.pinned {
  background: var(--kf-warning-soft);
  color: color-mix(in srgb, var(--kf-warning) 78%, var(--kf-text-primary));
}
.sc-list-item-resource {
  margin: 0.35rem 0 0;
  font-size: 0.6875rem;
  color: var(--kf-text-secondary);
  line-height: 1.4;
}
.sc-list-item-summary {
  margin: 0.3rem 0 0;
  font-size: 0.6875rem;
  line-height: 1.45;
  color: var(--kf-text-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.sc-list-item-time {
  display: block;
  margin-top: 0.35rem;
  font-size: 0.625rem;
  color: var(--kf-text-muted);
}
.sc-list-empty {
  margin: 0.75rem;
  padding: 1rem;
  border: 1px dashed var(--kf-border);
  border-radius: 10px;
  font-size: 0.75rem;
  line-height: 1.55;
  color: var(--kf-text-secondary);
  background: color-mix(in srgb, var(--kf-text-primary) 4%, var(--kf-mix-surface));
}
</style>
