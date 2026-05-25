<script setup lang="ts">
import { ref } from "vue";
import { NButton, NSpace } from "naive-ui";
import { kfSpace } from "../kf";
import type { ResourceSnapshotItem } from "../stores/resourceSnapshots";
import { formatSnapshotResourceRef } from "../stores/resourceSnapshots";
import { formatDateTime } from "../utils/dateFormat";

withDefaults(
  defineProps<{
    title?: string;
    subtitle?: string;
    currentSummary?: string;
    createLabel?: string;
    emptyText?: string;
    snapshots: ResourceSnapshotItem[];
    creating?: boolean;
    /** 在快照中心等全局列表中展示资源定位信息 */
    showResourceRef?: boolean;
    /** 资源环境显示名，key 为 env_id */
    envNameMap?: Record<string, string>;
    showCreate?: boolean;
    showOpenResource?: boolean;
    layout?: "sidebar" | "full";
  }>(),
  {
    showResourceRef: false,
    showCreate: true,
    showOpenResource: false,
    layout: "sidebar",
  }
);

const emit = defineEmits<{
  (e: "create"): void;
  (e: "view", snapshot: ResourceSnapshotItem): void;
  (e: "delete", snapshot: ResourceSnapshotItem): void;
  (e: "toggle-pin", snapshot: ResourceSnapshotItem): void;
  (e: "open-resource", snapshot: ResourceSnapshotItem): void;
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
  <aside class="snapshot-panel" :class="{ 'snapshot-panel-full': layout === 'full' }">
    <div class="snapshot-head">
      <div>
        <div class="snapshot-title">{{ title || "资源快照" }}</div>
        <p v-if="subtitle" class="snapshot-subtitle">{{ subtitle }}</p>
      </div>
      <NButton
        v-if="showCreate"
        type="primary"
        class="snapshot-create"
        :disabled="creating"
        :loading="creating"
        @click="emit('create')"
      >
        {{ creating ? "生成中…" : createLabel || "生成快照" }}
      </NButton>
    </div>
    <div v-if="currentSummary" class="snapshot-current">
      <span class="snapshot-current-label">当前资源</span>
      <p>{{ currentSummary }}</p>
    </div>
    <div v-if="snapshots.length" class="snapshot-list" :class="{ 'snapshot-list-grid': layout === 'full' }">
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
        <p v-if="showResourceRef" class="snapshot-card-resource">
          <span v-if="envNameMap?.[item.env_id]" class="snapshot-card-env">{{ envNameMap[item.env_id] }}</span>
          <span>{{ formatSnapshotResourceRef(item) }}</span>
        </p>
        <p class="snapshot-card-summary">{{ item.summary }}</p>
        <div class="snapshot-card-meta">
          <span>{{ formatDateTime(item.created_at) }}</span>
          <NSpace v-bind="kfSpace.settingInline" :size="4">
            <NButton
              v-if="showOpenResource"
              quaternary
              size="tiny"
              class="snapshot-open-resource"
              @click.stop="emit('open-resource', item)"
            >打开资源</NButton>
            <span>查看</span>
          </NSpace>
        </div>
      </div>
    </div>
    <div v-else class="snapshot-empty">
      {{ emptyText || "还没有快照，先保存一个当前资源快照。" }}
    </div>
  </aside>
</template>

<style scoped>
.snapshot-panel-full {
  width: 100%;
  min-width: 0;
  max-width: none;
  border-left: none;
  background: transparent;
}
.snapshot-panel-full .snapshot-head {
  padding: 0 0 1rem;
  border-bottom: none;
}
.snapshot-panel-full .snapshot-list-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 0.75rem;
  padding: 0;
}
.snapshot-panel {
  width: 280px;
  min-width: 280px;
  max-width: 320px;
  border-left: 1px solid var(--kf-border);
  background:
    radial-gradient(circle at top, color-mix(in srgb, var(--kf-info) 12%, transparent), transparent 38%),
    linear-gradient(180deg, var(--kf-surface-strong) 0%, var(--kf-bg-soft) 100%);
  display: flex;
  flex-direction: column;
}
.snapshot-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 1rem;
  border-bottom: 1px solid var(--kf-border);
}
.snapshot-title {
  font-size: 0.875rem;
  font-weight: 700;
  color: var(--kf-text-primary);
}
.snapshot-subtitle {
  margin: 0.35rem 0 0;
  font-size: 0.75rem;
  line-height: 1.5;
  color: var(--kf-text-secondary);
}
.snapshot-create {
  flex-shrink: 0;
}
.snapshot-current {
  margin: 1rem;
  padding: 0.85rem 0.9rem;
  border: 1px solid color-mix(in srgb, var(--kf-primary) 24%, var(--kf-border));
  border-radius: 14px;
  background: var(--kf-surface-strong);
}
.snapshot-current-label {
  display: inline-block;
  margin-bottom: 0.35rem;
  padding: 0.15rem 0.45rem;
  border-radius: 999px;
  background: var(--kf-primary-soft);
  color: color-mix(in srgb, var(--kf-primary) 82%, var(--kf-text-primary));
  font-size: 0.6875rem;
  font-weight: 700;
}
.snapshot-current p {
  margin: 0;
  font-size: 0.75rem;
  line-height: 1.5;
  color: var(--kf-text-primary);
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
  border: 1px solid var(--kf-border);
  border-radius: 14px;
  background: var(--kf-surface-strong);
  text-align: left;
  cursor: pointer;
  transition: transform 0.16s ease, border-color 0.16s ease, box-shadow 0.16s ease;
}
.snapshot-card:hover {
  transform: translateY(-1px);
  border-color: color-mix(in srgb, var(--kf-primary) 42%, var(--kf-border));
  box-shadow: var(--kf-shadow-sm);
}
.snapshot-card-danger {
  border-color: color-mix(in srgb, var(--kf-danger) 55%, var(--kf-border));
  box-shadow: 0 12px 24px color-mix(in srgb, var(--kf-danger) 16%, transparent);
}
.snapshot-card-pinned {
  border-color: color-mix(in srgb, var(--kf-warning) 55%, var(--kf-border));
  box-shadow: var(--kf-shadow-sm);
  background: linear-gradient(
    180deg,
    color-mix(in srgb, var(--kf-warning) 12%, var(--kf-surface-strong)),
    var(--kf-surface-strong)
  );
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
  color: var(--kf-text-primary);
}
.snapshot-card-badge {
  flex-shrink: 0;
  padding: 0.15rem 0.45rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--kf-text-primary) 8%, var(--kf-mix-surface));
  color: var(--kf-text-secondary);
  font-size: 0.6875rem;
}
.snapshot-card-badge.kind {
  background: var(--kf-primary-soft);
  color: color-mix(in srgb, var(--kf-primary) 82%, var(--kf-text-primary));
}
.snapshot-card-badge.pinned {
  background: var(--kf-warning-soft);
  color: color-mix(in srgb, var(--kf-warning) 78%, var(--kf-text-primary));
}
.snapshot-pin {
  width: 1.4rem;
  height: 1.4rem;
  border: none;
  border-radius: 999px;
  background: color-mix(in srgb, var(--kf-text-primary) 6%, var(--kf-mix-surface));
  color: var(--kf-text-secondary);
  font-size: 0.95rem;
  line-height: 1;
  cursor: pointer;
}
.snapshot-pin:hover {
  background: var(--kf-warning-soft);
  color: color-mix(in srgb, var(--kf-warning) 78%, var(--kf-text-primary));
}
.snapshot-pin.active {
  background: var(--kf-warning-soft);
  color: color-mix(in srgb, var(--kf-warning) 78%, var(--kf-text-primary));
}
.snapshot-delete {
  width: 1.4rem;
  height: 1.4rem;
  border: none;
  border-radius: 999px;
  background: var(--kf-danger-soft);
  color: var(--kf-danger);
  font-size: 0.95rem;
  line-height: 1;
  cursor: pointer;
}
.snapshot-delete:hover {
  background: color-mix(in srgb, var(--kf-danger) 18%, var(--kf-mix-surface));
}
.snapshot-delete.confirm {
  width: auto;
  min-width: 2.7rem;
  padding: 0 0.45rem;
  background: var(--kf-danger);
  color: #fff;
  font-size: 0.6875rem;
  font-weight: 700;
}
.snapshot-delete.confirm:hover {
  background: color-mix(in srgb, var(--kf-danger) 88%, black);
}
.snapshot-card-resource {
  margin: 0.45rem 0 0;
  font-size: 0.6875rem;
  line-height: 1.45;
  color: var(--kf-text-secondary);
}
.snapshot-card-env {
  display: inline-block;
  margin-right: 0.35rem;
  padding: 0.1rem 0.4rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--kf-text-primary) 8%, var(--kf-mix-surface));
  color: var(--kf-text-primary);
  font-weight: 600;
}
.snapshot-card-summary {
  margin: 0.6rem 0 0.75rem;
  font-size: 0.75rem;
  line-height: 1.5;
  color: var(--kf-text-secondary);
}
.snapshot-card-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  font-size: 0.6875rem;
  color: var(--kf-text-muted);
}
.snapshot-empty {
  margin: 1rem;
  padding: 1rem;
  border: 1px dashed var(--kf-border);
  border-radius: 14px;
  font-size: 0.75rem;
  line-height: 1.6;
  color: var(--kf-text-secondary);
  background: color-mix(in srgb, var(--kf-text-primary) 4%, var(--kf-mix-surface));
}

@media (max-width: 960px) {
  .snapshot-panel {
    width: auto;
    min-width: 0;
    max-width: none;
    border-left: none;
    border-top: 1px solid var(--kf-border);
  }
  .snapshot-list {
    max-height: 220px;
  }
}
</style>
