<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NButton, NEmpty, NInput, NSelect, NSpace } from "naive-ui";
import { kfSpace } from "../kf";
import SnapshotCenterList from "../components/snapshot/SnapshotCenterList.vue";
import ResourceSnapshotViewer from "../components/ResourceSnapshotViewer.vue";
import {
  deleteResourceSnapshot,
  getResourceSnapshotById,
  listAllResourceSnapshots,
  toggleResourceSnapshotPinned,
  type ResourceSnapshotItem,
  type ResourceSnapshotListFilters,
} from "../stores/resourceSnapshots";
import { useEnvStore } from "../stores/env";
import { useSnapshotCenterStore } from "../stores/snapshotCenter";

defineOptions({ name: "SnapshotCenterView" });

const { environments } = useEnvStore();
const { openResourceInWorkbench } = useSnapshotCenterStore();

const filterEnvId = ref<string>("");
const filterCategory = ref<ResourceSnapshotListFilters["category"]>("all");
const filterSource = ref<ResourceSnapshotListFilters["source"]>("all");
const searchQuery = ref("");
const selectedId = ref<string | null>(null);
const mobileShowDetail = ref(false);

const envNameMap = computed(() =>
  Object.fromEntries(environments.value.map((env) => [env.id, env.display_name]))
);

const envOptions = computed(() => [
  { label: "全部环境", value: "" },
  ...environments.value.map((env) => ({ label: env.display_name, value: env.id })),
]);

const categoryOptions = [
  { label: "全部分类", value: "all" },
  { label: "资源", value: "resource" },
  { label: "配置", value: "config" },
  { label: "镜像", value: "image" },
];

const sourceOptions = [
  { label: "全部来源", value: "all" },
  { label: "手动", value: "manual" },
  { label: "应用前", value: "before-apply" },
  { label: "镜像变更", value: "before-image-patch" },
];

const filteredSnapshots = computed(() =>
  listAllResourceSnapshots({
    envId: filterEnvId.value || null,
    category: filterCategory.value,
    source: filterSource.value,
    query: searchQuery.value,
  })
);

const selectedSnapshot = computed(() =>
  selectedId.value ? getResourceSnapshotById(selectedId.value) : null
);

const pinnedCount = computed(() => filteredSnapshots.value.filter((item) => item.pinned).length);

function syncSelection() {
  const list = filteredSnapshots.value;
  if (!list.length) {
    selectedId.value = null;
    mobileShowDetail.value = false;
    return;
  }
  if (!selectedId.value || !list.some((item) => item.id === selectedId.value)) {
    selectedId.value = list[0].id;
  }
}

watch(filteredSnapshots, syncSelection, { immediate: true });

function selectSnapshot(snapshot: ResourceSnapshotItem) {
  selectedId.value = snapshot.id;
  mobileShowDetail.value = true;
}

function removeSnapshot(snapshot: ResourceSnapshotItem) {
  deleteResourceSnapshot(snapshot.id);
  if (selectedId.value === snapshot.id) {
    syncSelection();
  }
}

function togglePinSnapshot(snapshot: ResourceSnapshotItem) {
  toggleResourceSnapshotPinned(snapshot.id);
}

function openResource(snapshot: ResourceSnapshotItem) {
  openResourceInWorkbench({
    envId: snapshot.env_id,
    kind: snapshot.resource_kind,
    name: snapshot.resource_name,
    namespace: snapshot.resource_namespace,
    initialTab: "snapshots",
  });
}

function resetFilters() {
  filterEnvId.value = "";
  filterCategory.value = "all";
  filterSource.value = "all";
  searchQuery.value = "";
}

function backToList() {
  mobileShowDetail.value = false;
}
</script>

<template>
  <div class="snapshot-center">
    <header class="sc-toolbar">
      <div class="sc-toolbar-title">
        <h1>快照中心</h1>
        <p>左侧选择快照，右侧即时预览 YAML、镜像对比与环境差异。</p>
      </div>
      <NSpace v-bind="kfSpace.settingInline" class="sc-toolbar-filters">
        <NSelect
          v-model:value="filterEnvId"
          :options="envOptions"
          clearable
          placeholder="环境"
          class="sc-filter-select"
        />
        <NSelect v-model:value="filterCategory" :options="categoryOptions" class="sc-filter-select sc-filter-narrow" />
        <NSelect v-model:value="filterSource" :options="sourceOptions" class="sc-filter-select sc-filter-narrow" />
        <NInput
          v-model:value="searchQuery"
          clearable
          placeholder="搜索标题、摘要、资源…"
          class="sc-filter-search"
        />
        <NButton quaternary @click="resetFilters">重置</NButton>
      </NSpace>
      <div class="sc-toolbar-meta">
        <span>{{ filteredSnapshots.length }} 条</span>
        <span v-if="pinnedCount">· {{ pinnedCount }} 置顶</span>
      </div>
    </header>

    <div class="sc-workspace" :class="{ 'sc-mobile-detail': mobileShowDetail }">
      <aside class="sc-rail" :class="{ 'sc-rail-hidden-mobile': mobileShowDetail }">
        <SnapshotCenterList
          :snapshots="filteredSnapshots"
          :selected-id="selectedId"
          :env-name-map="envNameMap"
          empty-text="还没有任何快照。在工作台编辑、应用配置或修改镜像时会自动生成；也可在资源详情的「快照」栏目手动保存。"
          @select="selectSnapshot"
          @delete="removeSnapshot"
          @toggle-pin="togglePinSnapshot"
        />
      </aside>

      <section class="sc-detail" :class="{ 'sc-detail-visible-mobile': mobileShowDetail }">
        <div v-if="mobileShowDetail" class="sc-detail-mobile-bar">
          <NButton quaternary size="small" @click="backToList">← 返回列表</NButton>
        </div>

        <ResourceSnapshotViewer
          v-if="selectedSnapshot"
          embedded
          :visible="true"
          :snapshot="selectedSnapshot"
          :env-id="selectedSnapshot.env_id"
          show-workbench-link
          @open-resource="openResource(selectedSnapshot)"
        />

        <div v-else class="sc-detail-empty">
          <NEmpty description="选择左侧快照以预览内容">
            <template v-if="filteredSnapshots.length" #extra>
              <p class="sc-detail-empty-hint">
                共 {{ filteredSnapshots.length }} 条快照，点击列表项即可在右侧查看。
              </p>
            </template>
          </NEmpty>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.snapshot-center {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  background: var(--kf-bg-soft);
}
.sc-toolbar {
  flex-shrink: 0;
  padding: 0.85rem 1rem;
  border-bottom: 1px solid var(--kf-border);
  background: var(--kf-surface-strong);
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  grid-template-rows: auto auto;
  gap: 0.5rem 1rem;
  align-items: center;
}
.sc-toolbar-title h1 {
  margin: 0;
  font-size: 1.125rem;
  font-weight: 600;
  color: var(--kf-text-primary);
}
.sc-toolbar-title p {
  margin: 0.2rem 0 0;
  font-size: 0.75rem;
  color: var(--kf-text-secondary);
}
.sc-toolbar-filters {
  grid-column: 1 / -1;
  flex-wrap: wrap;
}
.sc-filter-select {
  width: 150px;
}
.sc-filter-narrow {
  width: 120px;
}
.sc-filter-search {
  width: min(260px, 100%);
}
.sc-toolbar-meta {
  justify-self: end;
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--kf-text-secondary);
  white-space: nowrap;
}
.sc-workspace {
  flex: 1;
  min-height: 0;
  display: flex;
  overflow: hidden;
}
.sc-rail {
  width: min(360px, 38vw);
  min-width: 280px;
  flex-shrink: 0;
  min-height: 0;
}
.sc-detail {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--kf-surface-strong);
  border-left: 1px solid var(--kf-border);
}
.sc-detail-mobile-bar {
  display: none;
  flex-shrink: 0;
  padding: 0.35rem 0.5rem;
  border-bottom: 1px solid var(--kf-border);
}
.sc-detail-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 2rem;
}
.sc-detail-empty-hint {
  margin: 0.5rem 0 0;
  font-size: 0.8125rem;
  color: var(--kf-text-secondary);
  text-align: center;
}

@media (max-width: 900px) {
  .sc-toolbar {
    grid-template-columns: 1fr;
  }
  .sc-toolbar-meta {
    justify-self: start;
  }
  .sc-rail {
    width: 100%;
    min-width: 0;
    max-width: none;
  }
  .sc-workspace.sc-mobile-detail .sc-rail-hidden-mobile {
    display: none;
  }
  .sc-workspace:not(.sc-mobile-detail) .sc-detail {
    display: none;
  }
  .sc-workspace.sc-mobile-detail .sc-detail {
    width: 100%;
  }
  .sc-detail-mobile-bar {
    display: block;
  }
}
</style>
