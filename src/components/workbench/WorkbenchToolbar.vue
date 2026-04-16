<script setup lang="ts">
import { ref } from "vue";
import type { ResourceKind } from "../../constants/resourceKinds";
import type { NamespaceItem, ResolvedAliasTarget } from "../../api/kube";
import { resourceSupportsWatch } from "../../resources/resourceRegistry";

type KindGroup = { id: string; label: string; kinds: { id: ResourceKind; label: string }[] };
type FilterChip = { id: string; label: string; value: string };

const props = defineProps<{
  currentEnv: { display_name: string; source?: string } | null;
  currentId: string | null;
  envConnectionState: string;
  currentEnvStateLabel: string;
  currentEnvSourceLabel: string;
  currentEnvContextLabel: string;
  shouldShowCurrentEnvContext: boolean;
  nsSelectionDisabled: boolean;
  effectiveNamespace: string;
  selectedNamespace: string | null;
  namespaceFavorites: NamespaceItem[];
  namespaceRecent: NamespaceItem[];
  namespaceOthers: NamespaceItem[];
  favoriteNamespaces: Set<string>;
  workbenchKindLabel: string;
  customResourceHintLine: string;
  customResourceStatusClass: string;
  recentKindItems: { id: ResourceKind; label: string }[];
  filteredKindGroups: KindGroup[];
  selectedKind: ResourceKind;
  selectedCustomTarget: ResolvedAliasTarget | null;
  customKindCandidates: ResolvedAliasTarget[];
  listLoading: boolean;
  showBatchToolbar: boolean;
  batchDeleteMode: boolean;
  selectedRowCount: number;
  podNodeOptions: string[];
  supportsIpFilter: boolean;
  selectedKindForIp: ResourceKind;
  ipFilterPlaceholder: string;
  ipFilterTitle: string;
  activeFilterChips: FilterChip[];
}>();

const emit = defineEmits<{
  "toggle-namespace": [];
  "select-namespace": [ns: string | null];
  "toggle-favorite-namespace": [name: string];
  "select-kind": [kind: ResourceKind];
  "select-custom-kind": [target: ResolvedAliasTarget];
  refresh: [];
  "enter-batch-delete": [];
  "exit-batch-delete": [];
  "open-batch-delete-confirm": [];
  "apply-label-filter": [];
  "clear-filter-chip": [id: string];
  "clear-all-filters": [];
}>();

const nsFilter = defineModel<string>("nsFilter", { required: true });
const kindFilter = defineModel<string>("kindFilter", { required: true });
const nameFilter = defineModel<string>("nameFilter", { required: true });
const labelSelector = defineModel<string>("labelSelector", { required: true });
const nodeFilter = defineModel<string>("nodeFilter", { required: true });
const podIpFilter = defineModel<string>("podIpFilter", { required: true });
const watchEnabled = defineModel<boolean>("watchEnabled", { required: true });
const nsDropdownOpen = defineModel<boolean>("nsDropdownOpen", { required: true });
const kindDropdownOpen = defineModel<boolean>("kindDropdownOpen", { required: true });

const nsDropdownRef = ref<HTMLElement | null>(null);
const kindDropdownRef = ref<HTMLElement | null>(null);
const nsMenuRef = ref<HTMLElement | null>(null);
const kindMenuRef = ref<HTMLElement | null>(null);

function onKindTriggerClick() {
  kindDropdownOpen.value = !kindDropdownOpen.value;
  if (kindDropdownOpen.value) {
    if (!props.selectedCustomTarget) kindFilter.value = "";
    nsDropdownOpen.value = false;
  }
}

defineExpose({
  nsDropdownRef,
  kindDropdownRef,
  nsMenuRef,
  kindMenuRef,
});
</script>

<template>
  <header class="toolbar">
    <div class="toolbar-card">
      <div class="toolbar-main">
        <div v-if="currentEnv" class="active-env-banner">
          <div class="active-env-copy">
            <div class="active-env-name-row">
              <span class="active-env-kicker">当前环境</span>
              <span class="env-name">{{ currentEnv.display_name }}</span>
              <span class="active-env-state" :class="`active-env-state-${envConnectionState}`">
                {{ currentEnvStateLabel }}
              </span>
            </div>
            <div class="active-env-meta-row">
              <span class="active-env-chip">{{ currentEnvSourceLabel }}</span>
              <span v-if="shouldShowCurrentEnvContext" class="active-env-chip subtle">
                Context: {{ currentEnvContextLabel }}
              </span>
            </div>
          </div>
        </div>
        <div v-if="currentId" ref="nsDropdownRef" class="combobox-wrap">
          <button
            type="button"
            class="combobox-trigger"
            :class="{ open: nsDropdownOpen, disabled: nsSelectionDisabled, 'combobox-trigger-strong': true }"
            :disabled="nsSelectionDisabled"
            :title="nsSelectionDisabled ? '当前资源为集群级，命名空间不生效' : '命名空间：输入筛选后选择'"
            @click="emit('toggle-namespace')"
          >
            <span class="combobox-trigger-main">
              <span class="combobox-label">命名空间</span>
              <span class="combobox-value">{{ nsSelectionDisabled ? "集群级资源" : effectiveNamespace }}</span>
            </span>
            <span class="combobox-arrow">▼</span>
          </button>
          <div v-show="nsDropdownOpen" ref="nsMenuRef" class="combobox-menu">
            <div class="combobox-panel-head">
              <div class="combobox-panel-title">选择命名空间</div>
              <div class="combobox-panel-subtitle">当前资源范围会基于这里切换</div>
            </div>
            <div class="combobox-search">
              <input
                v-model="nsFilter"
                type="text"
                class="combobox-input"
                placeholder="搜索命名空间…"
                autocomplete="off"
              />
            </div>
            <button
              type="button"
              class="combobox-item"
              :class="{ active: selectedNamespace === null }"
              @click="emit('select-namespace', null)"
            >
              <span class="combobox-item-main">
                <span class="combobox-item-title">全部命名空间</span>
                <span class="combobox-item-subtitle">浏览当前环境下的全部命名空间</span>
              </span>
              <span class="combobox-item-check">{{ selectedNamespace === null ? "✓" : "" }}</span>
            </button>
            <template v-if="namespaceFavorites.length > 0">
              <div class="combobox-group-label">收藏</div>
              <button
                v-for="n in namespaceFavorites"
                :key="`fav-${n.name}`"
                type="button"
                class="combobox-item combobox-item-with-action"
                :class="{ active: selectedNamespace === n.name }"
                @click="emit('select-namespace', n.name)"
              >
                <span class="combobox-item-main">
                  <span class="combobox-item-title">{{ n.name }}</span>
                  <span class="combobox-item-subtitle">收藏的命名空间</span>
                </span>
                <span class="combobox-item-trailing">
                  <span class="combobox-item-check">{{ selectedNamespace === n.name ? "✓" : "" }}</span>
                  <span class="ns-star active" title="取消收藏" @click.stop="emit('toggle-favorite-namespace', n.name)">
                    ★
                  </span>
                </span>
              </button>
            </template>
            <template v-if="namespaceRecent.length > 0">
              <div class="combobox-group-label">最近</div>
              <button
                v-for="n in namespaceRecent"
                :key="`recent-${n.name}`"
                type="button"
                class="combobox-item combobox-item-with-action"
                :class="{ active: selectedNamespace === n.name }"
                @click="emit('select-namespace', n.name)"
              >
                <span class="combobox-item-main">
                  <span class="combobox-item-title">{{ n.name }}</span>
                  <span class="combobox-item-subtitle">最近访问</span>
                </span>
                <span class="combobox-item-trailing">
                  <span class="combobox-item-check">{{ selectedNamespace === n.name ? "✓" : "" }}</span>
                  <span
                    class="ns-star"
                    :class="{ active: favoriteNamespaces.has(n.name) }"
                    :title="favoriteNamespaces.has(n.name) ? '取消收藏' : '收藏'"
                    @click.stop="emit('toggle-favorite-namespace', n.name)"
                  >
                    ★
                  </span>
                </span>
              </button>
            </template>
            <div class="combobox-group-label">全部</div>
            <button
              v-for="n in namespaceOthers"
              :key="n.name"
              type="button"
              class="combobox-item combobox-item-with-action"
              :class="{ active: selectedNamespace === n.name }"
              @click="emit('select-namespace', n.name)"
            >
              <span class="combobox-item-main">
                <span class="combobox-item-title">{{ n.name }}</span>
              </span>
              <span class="combobox-item-trailing">
                <span class="combobox-item-check">{{ selectedNamespace === n.name ? "✓" : "" }}</span>
                <span
                  class="ns-star"
                  :class="{ active: favoriteNamespaces.has(n.name) }"
                  :title="favoriteNamespaces.has(n.name) ? '取消收藏' : '收藏'"
                  @click.stop="emit('toggle-favorite-namespace', n.name)"
                >
                  ★
                </span>
              </span>
            </button>
          </div>
        </div>
        <div ref="kindDropdownRef" class="combobox-wrap">
          <button
            type="button"
            class="combobox-trigger"
            :class="{ open: kindDropdownOpen, 'combobox-trigger-strong': true }"
            title="资源类型：输入筛选后选择"
            @click="onKindTriggerClick"
          >
            <span class="combobox-trigger-main">
              <span class="combobox-label">资源类型</span>
              <span class="combobox-value">{{ workbenchKindLabel }}</span>
            </span>
            <span class="combobox-arrow">▼</span>
          </button>
          <div v-show="kindDropdownOpen" ref="kindMenuRef" class="combobox-menu combobox-menu-grouped">
            <div class="combobox-panel-head">
              <div class="combobox-panel-title">选择资源类型</div>
              <div class="combobox-panel-subtitle">内置资源按分组浏览；CRD 在下方专区搜索后选择</div>
            </div>
            <div class="combobox-search">
              <input
                v-model="kindFilter"
                type="text"
                class="combobox-input"
                placeholder="筛选内置类型，或在 CRD 专区匹配 Kind / plural / 短名…"
                autocomplete="off"
              />
            </div>
            <div v-if="kindFilter.trim() && customResourceHintLine" class="kind-custom-hint-wrap">
              <span :class="customResourceStatusClass">{{ customResourceHintLine }}</span>
            </div>
            <div v-if="recentKindItems.length > 0 && !kindFilter.trim()" class="recent-kind-panel">
              <div class="recent-kind-title">最近使用</div>
              <div class="recent-kind-list">
                <button
                  v-for="k in recentKindItems"
                  :key="`recent-kind-${k.id}`"
                  type="button"
                  class="recent-kind-pill"
                  :class="{ active: !selectedCustomTarget && selectedKind === k.id }"
                  @click="emit('select-kind', k.id)"
                >
                  {{ k.label }}
                </button>
              </div>
            </div>
            <template v-for="group in filteredKindGroups" :key="group.id">
              <div class="combobox-group-label">{{ group.label }}</div>
              <button
                v-for="k in group.kinds"
                :key="k.id"
                type="button"
                class="combobox-item"
                :class="{ active: !selectedCustomTarget && selectedKind === k.id }"
                @click="emit('select-kind', k.id)"
              >
                <span class="combobox-item-main">
                  <span class="combobox-item-title">{{ k.label }}</span>
                </span>
                <span class="combobox-item-check">{{ !selectedCustomTarget && selectedKind === k.id ? "✓" : "" }}</span>
              </button>
            </template>
            <div class="combobox-group-label">CRD（自定义资源）</div>
            <div v-if="!kindFilter.trim()" class="kind-crd-empty-hint">
              在上方输入框搜索后，此处列出与集群发现匹配的 CRD 类型；选中后表格展示该资源的实例列表。
            </div>
            <template v-if="kindFilter.trim() && customKindCandidates.length > 0">
              <button
                v-for="target in customKindCandidates"
                :key="`${target.api_version}/${target.kind}/${target.plural}`"
                type="button"
                class="combobox-item"
                :class="{
                  active:
                    selectedCustomTarget?.api_version === target.api_version &&
                    selectedCustomTarget?.kind === target.kind &&
                    selectedCustomTarget?.plural === target.plural,
                }"
                @click="emit('select-custom-kind', target)"
              >
                <span class="combobox-item-main">
                  <span class="combobox-item-title">{{ target.kind }}</span>
                  <span class="combobox-item-subtitle">
                    {{ target.api_version }} · {{ target.plural }} · {{ target.namespaced ? "Namespaced" : "Cluster" }}
                  </span>
                </span>
                <span class="combobox-item-check">
                  {{
                    selectedCustomTarget?.api_version === target.api_version &&
                    selectedCustomTarget?.kind === target.kind &&
                    selectedCustomTarget?.plural === target.plural
                      ? "✓"
                      : ""
                  }}
                </span>
              </button>
            </template>
          </div>
        </div>
        <div class="toolbar-actions">
          <button
            v-if="currentId && resourceSupportsWatch(selectedKind) && !selectedCustomTarget"
            type="button"
            class="btn-watch"
            :class="{ active: watchEnabled }"
            :title="watchEnabled ? '关闭 Watch 实时更新' : '开启 Watch 实时更新'"
            @click="watchEnabled = !watchEnabled"
          >
            {{ watchEnabled ? "Watch 开" : "Watch" }}
          </button>
          <button type="button" class="btn-refresh" :disabled="listLoading" @click="emit('refresh')">
            {{ listLoading ? "刷新中…" : "刷新" }}
          </button>
          <template v-if="showBatchToolbar">
            <button v-if="!batchDeleteMode" type="button" class="btn-secondary-outline" @click="emit('enter-batch-delete')">
              批量删除
            </button>
            <template v-else>
              <button type="button" class="btn-secondary-outline" @click="emit('exit-batch-delete')">取消</button>
              <button
                type="button"
                class="btn-danger-outline"
                :disabled="selectedRowCount === 0"
                @click="emit('open-batch-delete-confirm')"
              >
                删除选中 ({{ selectedRowCount }})
              </button>
            </template>
          </template>
        </div>
      </div>
      <div v-if="currentId" class="toolbar-filters">
        <div class="toolbar-filters-primary">
          <input
            v-model="nameFilter"
            type="text"
            class="filter-input"
            placeholder="按名称筛选…"
            autocomplete="off"
            title="按名称包含匹配（前端过滤）"
          />
          <input
            v-model="labelSelector"
            type="text"
            class="filter-input filter-input-label"
            placeholder="Label 筛选，如 app=nginx"
            autocomplete="off"
            title="K8s label selector，如 app=nginx 或 env in (prod,staging)"
            @keyup.enter="emit('apply-label-filter')"
          />
        </div>
        <div v-if="selectedKindForIp === 'pods' || selectedKindForIp === 'services'" class="toolbar-filters-secondary">
          <select v-if="selectedKindForIp === 'pods'" v-model="nodeFilter" class="filter-input" title="按 Node 选项筛选">
            <option value="all">Node: All</option>
            <option v-for="node in podNodeOptions" :key="node" :value="node">Node: {{ node }}</option>
          </select>
          <input
            v-if="supportsIpFilter"
            v-model="podIpFilter"
            type="text"
            class="filter-input"
            :placeholder="ipFilterPlaceholder"
            autocomplete="off"
            :title="ipFilterTitle"
          />
        </div>
      </div>
      <div v-if="activeFilterChips.length" class="filter-chip-bar">
        <span class="filter-chip-label">已启用筛选</span>
        <button
          v-for="chip in activeFilterChips"
          :key="chip.id"
          type="button"
          class="filter-chip"
          :title="`点击移除 ${chip.label} 筛选`"
          @click="emit('clear-filter-chip', chip.id)"
        >
          {{ chip.label }}: {{ chip.value }}
          <span class="filter-chip-close" aria-hidden="true">×</span>
        </button>
        <button type="button" class="filter-chip-clear-all" @click="emit('clear-all-filters')">清除全部</button>
      </div>
    </div>
  </header>
</template>

<style scoped>
.toolbar {
  padding: 0.75rem 1rem;
  border-bottom: 1px solid #e2e8f0;
  background: #f8fafc;
  display: block;
  flex-shrink: 0;
}
.toolbar-card {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.65rem 0.75rem;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  background: #fff;
}
.toolbar-main {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem;
}
.toolbar-filters {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem;
  padding-top: 0.1rem;
}
.toolbar-filters-primary,
.toolbar-filters-secondary {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem;
}
.toolbar-filters-secondary {
  margin-left: auto;
}
.toolbar-actions {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
}
.env-name {
  font-weight: 800;
  font-size: 0.88rem;
  color: #0f172a;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.active-env-banner {
  display: inline-flex;
  align-items: center;
  flex: 0 1 auto;
  width: fit-content;
  max-width: min(100%, 300px);
  min-width: 0;
  padding: 0.4rem 0.55rem;
  border-radius: 12px;
  border: 1px solid rgba(37, 99, 235, 0.14);
  background:
    radial-gradient(circle at top right, rgba(14, 165, 233, 0.14), transparent 26%),
    linear-gradient(135deg, #eff6ff, #f8fafc 72%);
}
.active-env-copy {
  min-width: 0;
  width: auto;
}
.active-env-kicker {
  display: inline-flex;
  align-items: center;
  padding: 0.12rem 0.34rem;
  border-radius: 999px;
  background: rgba(37, 99, 235, 0.1);
  font-size: 0.6rem;
  font-weight: 700;
  color: #2563eb;
  flex-shrink: 0;
}
.active-env-name-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.38rem;
}
.active-env-meta-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.24rem;
  margin-top: 0.28rem;
}
.active-env-chip {
  display: inline-flex;
  align-items: center;
  padding: 0.12rem 0.36rem;
  border-radius: 999px;
  background: rgba(37, 99, 235, 0.1);
  color: #1d4ed8;
  font-size: 0.64rem;
  font-weight: 700;
  max-width: 100%;
}
.active-env-chip.subtle {
  background: rgba(255, 255, 255, 0.78);
  color: #475569;
  border: 1px solid rgba(148, 163, 184, 0.22);
}
.active-env-state {
  display: inline-flex;
  align-items: center;
  padding: 0.12rem 0.34rem;
  border-radius: 999px;
  font-size: 0.62rem;
  font-weight: 700;
  flex-shrink: 0;
}
.active-env-state-connected,
.active-env-state-error {
  background: #ecfdf5;
  color: #15803d;
}
.active-env-state-connecting {
  background: #e0f2fe;
  color: #0369a1;
}
.active-env-state-disconnected {
  background: #fef2f2;
  color: #dc2626;
}
.combobox-wrap {
  position: relative;
}
.combobox-trigger {
  display: inline-flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.55rem;
  padding: 0.48rem 0.7rem;
  border: 1px solid #e2e8f0;
  border-radius: 12px;
  background: #fff;
  font-size: 0.8125rem;
  color: #475569;
  cursor: pointer;
  min-width: 0;
  min-height: 48px;
  box-shadow: 0 1px 2px rgba(15, 23, 42, 0.03);
}
.combobox-trigger:hover {
  background: #f8fafc;
  border-color: #cbd5e1;
}
.combobox-trigger-strong {
  min-width: 190px;
}
.combobox-trigger.disabled {
  opacity: 0.6;
  cursor: not-allowed;
  background: #f8fafc;
}
.combobox-trigger.open {
  border-color: #2563eb;
  background: #eff6ff;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
}
.combobox-trigger-main {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  min-width: 0;
}
.combobox-label {
  color: #64748b;
  font-size: 0.68rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
}
.combobox-value {
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: #0f172a;
  font-weight: 700;
  line-height: 1.25;
}
.combobox-arrow {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.4rem;
  height: 1.4rem;
  border-radius: 999px;
  background: #f1f5f9;
  font-size: 0.62rem;
  color: #64748b;
  flex-shrink: 0;
}
.combobox-menu {
  position: absolute;
  top: calc(100% + 8px);
  left: 0;
  min-width: max(280px, 100%);
  width: fit-content;
  max-width: min(560px, calc(100vw - 32px));
  max-height: 420px;
  overflow-y: auto;
  overflow-x: hidden;
  background: #fff;
  border: 1px solid #e2e8f0;
  border-radius: 16px;
  box-shadow: 0 18px 40px rgba(15, 23, 42, 0.16);
  padding: 0.3rem 0;
  z-index: 100;
  display: flex;
  flex-direction: column;
}
.combobox-panel-head {
  padding: 0.55rem 0.8rem 0.25rem;
}
.combobox-panel-title {
  font-size: 0.86rem;
  font-weight: 700;
  color: #0f172a;
}
.combobox-panel-subtitle {
  margin-top: 0.16rem;
  font-size: 0.72rem;
  color: #64748b;
}
.combobox-search {
  padding: 0.2rem 0.6rem 0.35rem;
}
.combobox-input {
  width: 100%;
  box-sizing: border-box;
  padding: 0.55rem 0.7rem;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  font-size: 0.8125rem;
  background: #f8fafc;
}
.combobox-input:focus {
  outline: none;
  border-color: #2563eb;
  background: #fff;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
}
.filter-input {
  padding: 0.35rem 0.6rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  font-size: 0.8125rem;
  min-width: 120px;
  max-width: 160px;
}
.filter-input:focus {
  outline: none;
  border-color: #2563eb;
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.12);
}
.filter-input-label {
  min-width: 160px;
  max-width: 220px;
}
.combobox-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: calc(100% - 0.7rem);
  box-sizing: border-box;
  gap: 0.5rem;
  padding: 0.52rem 0.8rem;
  border: none;
  background: none;
  font-size: 0.8125rem;
  text-align: left;
  cursor: pointer;
  color: #334155;
  border-radius: 10px;
  margin: 0 0.35rem;
}
.combobox-item-with-action {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.combobox-item-main {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
}
.combobox-item-title {
  white-space: nowrap;
}
.combobox-item-subtitle {
  margin-top: 0.1rem;
  font-size: 0.71rem;
  color: #94a3b8;
  white-space: nowrap;
}
.combobox-item-trailing {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  flex-shrink: 0;
}
.combobox-item-check {
  width: 1rem;
  text-align: center;
  color: #2563eb;
  font-weight: 800;
  flex-shrink: 0;
}
.ns-star {
  font-size: 0.875rem;
  line-height: 1;
  color: #cbd5e1;
  padding: 0.1rem 0.2rem;
  border-radius: 4px;
}
.ns-star.active {
  color: #f59e0b;
}
.ns-star:hover {
  background: #f1f5f9;
}
.combobox-item:hover {
  background: #f8fafc;
}
.combobox-item.active {
  background: rgba(37, 99, 235, 0.09);
  color: #1d4ed8;
  font-weight: 600;
}
.combobox-group-label {
  padding: 0.45rem 0.85rem 0.22rem;
  font-size: 0.6875rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: #94a3b8;
  border-top: 1px solid #f1f5f9;
}
.recent-kind-panel {
  margin: 0.15rem 0.6rem 0.45rem;
  padding: 0.45rem 0.5rem 0.5rem;
  border: 1px solid #dbeafe;
  border-radius: 12px;
  background: #f0f7ff;
}
.recent-kind-title {
  margin: 0.05rem 0.15rem 0.32rem;
  font-size: 0.71rem;
  font-weight: 600;
  color: #3b82f6;
  letter-spacing: 0.03em;
}
.recent-kind-list {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.3rem;
}
.recent-kind-pill {
  border: 1px solid #bfdbfe;
  background: #eff6ff;
  color: #1d4ed8;
  border-radius: 999px;
  padding: 0.2rem 0.6rem;
  font-size: 0.75rem;
  line-height: 1.2;
  cursor: pointer;
}
.recent-kind-pill:hover {
  background: #dbeafe;
}
.recent-kind-pill.active {
  border-color: #3b82f6;
  background: #bfdbfe;
  color: #1e40af;
  font-weight: 600;
}
.combobox-group-label:first-of-type {
  border-top: none;
  padding-top: 0.25rem;
}
.kind-custom-hint-wrap {
  padding: 0 0.75rem 0.35rem;
}
.kind-crd-empty-hint {
  padding: 0.35rem 0.75rem 0.6rem;
  font-size: 0.72rem;
  line-height: 1.45;
  color: #64748b;
}
.toolbar-cr-hint {
  font-size: 0.7rem;
  line-height: 1.35;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
}
.toolbar-cr-hint.muted {
  color: #94a3b8;
}
.toolbar-cr-hint.loading {
  color: #2563eb;
}
.toolbar-cr-hint.ok {
  color: #047857;
}
.toolbar-cr-hint.warn {
  color: #b45309;
}
.toolbar-cr-hint.err {
  color: #b91c1c;
}
.btn-refresh {
  padding: 0.35rem 0.75rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  background: #fff;
  font-size: 0.8125rem;
  cursor: pointer;
}
.btn-refresh:hover:not(:disabled) {
  background: #f8fafc;
}
.btn-refresh:disabled {
  opacity: 0.7;
  cursor: not-allowed;
}
.btn-secondary-outline {
  padding: 0.35rem 0.75rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  background: #fff;
  color: #475569;
  font-size: 0.8125rem;
  cursor: pointer;
}
.btn-secondary-outline:hover {
  background: #f8fafc;
}
.btn-danger-outline {
  padding: 0.35rem 0.75rem;
  border: 1px solid #dc2626;
  border-radius: 6px;
  background: #fff;
  color: #dc2626;
  font-size: 0.8125rem;
  cursor: pointer;
}
.btn-danger-outline:hover {
  background: #fef2f2;
}
.btn-danger-outline:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.btn-watch {
  padding: 0.35rem 0.75rem;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  background: #fff;
  font-size: 0.8125rem;
  cursor: pointer;
  color: #64748b;
}
.btn-watch:hover {
  background: #f8fafc;
}
.btn-watch.active {
  border-color: #22c55e;
  background: rgba(34, 197, 94, 0.08);
  color: #16a34a;
}
.filter-chip-bar {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  flex-wrap: wrap;
}
.filter-chip-label {
  font-size: 0.75rem;
  color: #64748b;
}
.filter-chip {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  border: 1px solid #bfdbfe;
  border-radius: 999px;
  padding: 0.2rem 0.55rem;
  font-size: 0.75rem;
  color: #1d4ed8;
  background: #eff6ff;
  cursor: pointer;
}
.filter-chip:hover {
  background: #dbeafe;
}
.filter-chip-close {
  opacity: 0.75;
}
.filter-chip-clear-all {
  border: none;
  background: transparent;
  color: #64748b;
  font-size: 0.75rem;
  cursor: pointer;
  text-decoration: underline;
  padding: 0.1rem 0.2rem;
}
@media (max-width: 960px) {
  .active-env-banner {
    display: flex;
    width: 100%;
    max-width: none;
  }
  .toolbar-filters-secondary {
    margin-left: 0;
  }
}
</style>
