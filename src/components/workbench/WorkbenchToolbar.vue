<script setup lang="ts">
import { ref } from "vue";
import { NButton, NInput, NPopover, NSelect, NTag } from "naive-ui";
import type { ResourceKind } from "../../constants/resourceKinds";
import type { NamespaceItem, ResolvedAliasTarget } from "../../api/kube";
import { resourceSupportsWatch } from "../../resources/resourceRegistry";

/** 环境连接状态 → NTag 语义类型：connected/error 归绿，connecting 归蓝，disconnected 归红 */
function envStateTagType(state: string): "success" | "info" | "error" | "default" {
  if (state === "connected" || state === "error") return "success";
  if (state === "connecting") return "info";
  if (state === "disconnected") return "error";
  return "default";
}

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
const nodeFilterOptions = [{ label: "Node: All", value: "all" }];

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
        <div v-if="currentEnv" class="toolbar-cluster toolbar-cluster-scope">
          <div class="active-env-banner">
            <span class="env-name" :title="currentEnv.display_name">{{ currentEnv.display_name }}</span>
            <NTag
              size="small"
              round
              :bordered="false"
              :type="envStateTagType(envConnectionState)"
              class="active-env-state-tag"
            >
              <span class="state-dot" :class="`state-dot-${envConnectionState}`" aria-hidden="true" />
              {{ currentEnvStateLabel }}
            </NTag>
            <NTag size="small" round :bordered="false" type="info" class="active-env-meta">
              {{ currentEnvSourceLabel }}
            </NTag>
            <NTag
              v-if="shouldShowCurrentEnvContext"
              size="small"
              round
              :bordered="true"
              class="active-env-meta subtle"
              :title="currentEnvContextLabel"
            >
              Context: {{ currentEnvContextLabel }}
            </NTag>
          </div>
        </div>
        <NPopover
          v-if="currentId"
          ref="nsDropdownRef"
          class="combobox-wrap"
          trigger="click"
          placement="bottom-start"
          :show-arrow="false"
          :show="nsDropdownOpen"
          @update:show="(v) => (nsDropdownOpen = v)"
        >
          <template #trigger>
            <button
              type="button"
              class="combobox-trigger"
              :class="{ open: nsDropdownOpen, 'combobox-trigger-strong': true }"
              title="命名空间：输入筛选后选择"
              @click="emit('toggle-namespace')"
            >
              <span class="combobox-trigger-main">
                <span class="combobox-label">命名空间</span>
                <span class="combobox-value">{{ effectiveNamespace }}</span>
              </span>
              <span class="combobox-arrow">▼</span>
            </button>
          </template>
          <div ref="nsMenuRef" class="combobox-menu">
            <div class="combobox-panel-head">
              <div class="combobox-panel-title">选择命名空间</div>
              <div class="combobox-panel-subtitle">当前资源范围会基于这里切换</div>
            </div>
            <div class="combobox-search">
              <NInput
                v-model:value="nsFilter"
                size="small"
                clearable
                placeholder="搜索命名空间…"
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
                class="combobox-item combobox-item-with-action combobox-item-favorite"
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
                class="combobox-item combobox-item-with-action combobox-item-recent"
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
        </NPopover>
        <NPopover
          ref="kindDropdownRef"
          class="combobox-wrap"
          trigger="click"
          placement="bottom-start"
          :show-arrow="false"
          :show="kindDropdownOpen"
          @update:show="(v) => (kindDropdownOpen = v)"
        >
          <template #trigger>
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
          </template>
          <div ref="kindMenuRef" class="combobox-menu combobox-menu-grouped">
            <div class="combobox-panel-head">
              <div class="combobox-panel-title">选择资源类型</div>
              <div class="combobox-panel-subtitle">内置资源按分组浏览；CRD 在下方专区搜索后选择</div>
            </div>
            <div class="combobox-search">
              <NInput
                v-model:value="kindFilter"
                size="small"
                clearable
                placeholder="筛选内置类型，或在 CRD 专区匹配 Kind / plural / 短名…"
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
        </NPopover>
        <span class="toolbar-vsep" aria-hidden="true" />
        <div class="toolbar-actions">
          <NButton
            v-if="currentId && resourceSupportsWatch(selectedKind) && !selectedCustomTarget"
            class="btn-watch"
            :class="{ active: watchEnabled }"
            secondary
            round
            size="small"
            @click="watchEnabled = !watchEnabled"
          >
            <span class="watch-dot" aria-hidden="true" />
            {{ watchEnabled ? "实时更新已开启" : "开启实时更新" }}
          </NButton>
          <NButton class="btn-refresh" type="primary" strong secondary size="small" :loading="listLoading" @click="emit('refresh')">
            {{ listLoading ? "刷新中…" : "刷新" }}
          </NButton>
          <template v-if="showBatchToolbar">
            <NButton v-if="!batchDeleteMode" class="btn-secondary-outline" secondary size="small" @click="emit('enter-batch-delete')">
              批量删除
            </NButton>
            <template v-else>
              <NButton class="btn-secondary-outline" secondary size="small" @click="emit('exit-batch-delete')">取消</NButton>
              <NButton
                class="btn-danger-outline"
                type="error"
                secondary
                size="small"
                :disabled="selectedRowCount === 0"
                @click="emit('open-batch-delete-confirm')"
              >
                删除选中 ({{ selectedRowCount }})
              </NButton>
            </template>
          </template>
        </div>
      </div>
      <div v-if="currentId" class="toolbar-filters-shell">
        <div class="toolbar-filters">
          <div class="toolbar-filters-primary">
            <NInput
              v-model:value="nameFilter"
              class="filter-input"
              placeholder="按名称筛选…"
              title="按名称包含匹配（前端过滤）"
              clearable
              size="small"
            />
            <NInput
              v-model:value="labelSelector"
              class="filter-input filter-input-label"
              placeholder="Label 筛选，如 app=nginx"
              title="K8s label selector，如 app=nginx 或 env in (prod,staging)"
              clearable
              size="small"
              @keyup.enter="emit('apply-label-filter')"
            />
          </div>
          <div v-if="selectedKindForIp === 'pods' || selectedKindForIp === 'services'" class="toolbar-filters-secondary">
            <NSelect
              v-if="selectedKindForIp === 'pods'"
              v-model:value="nodeFilter"
              class="filter-input node-select"
              :options="[...nodeFilterOptions, ...podNodeOptions.map((node) => ({ label: `Node: ${node}`, value: node }))]"
              size="small"
              title="按 Node 选项筛选"
            />
            <NInput
              v-if="supportsIpFilter"
              v-model:value="podIpFilter"
              class="filter-input"
              :placeholder="ipFilterPlaceholder"
              :title="ipFilterTitle"
              clearable
              size="small"
            />
          </div>
        </div>
      </div>
      <div v-if="activeFilterChips.length" class="filter-chip-bar">
        <span class="filter-chip-label">已启用筛选</span>
        <NTag
          v-for="chip in activeFilterChips"
          :key="chip.id"
          class="filter-chip"
          type="info"
          size="small"
          round
          closable
          :title="`点击移除 ${chip.label} 筛选`"
          @close="emit('clear-filter-chip', chip.id)"
        >
          <span class="filter-chip-name">{{ chip.label }}:</span>
          <span class="filter-chip-value">{{ chip.value }}</span>
        </NTag>
        <NButton text type="primary" size="tiny" class="filter-chip-clear-all" @click="emit('clear-all-filters')">清除全部</NButton>
      </div>
    </div>
  </header>
</template>

<style scoped>
.toolbar {
  padding: 0.75rem 1rem;
  border-bottom: 1px solid var(--wb-line, rgba(148, 163, 184, 0.22));
  background: var(--wb-canvas, #eef2f9);
  display: block;
  flex-shrink: 0;
  position: relative;
  z-index: 30;
  --wb-ctrl-height: 40px;
  --wb-ctrl-radius: 10px;
  --wb-ctrl-font: 0.8rem;
  --wb-focus-ring: 0 0 0 3px rgba(37, 99, 235, 0.18);
}
.toolbar-card {
  display: flex;
  flex-direction: column;
  gap: 0.65rem;
  padding: 0.8rem 0.9rem;
  border: 1px solid var(--wb-line, rgba(148, 163, 184, 0.22));
  border-radius: 12px;
  background: color-mix(in srgb, var(--wb-panel-elevated, #ffffff) 92%, transparent);
  backdrop-filter: blur(8px);
  box-shadow: var(--kf-shadow-sm, 0 12px 32px rgba(15, 23, 42, 0.08));
}
.toolbar-cluster-scope {
  flex: 0 1 auto;
}
.toolbar-main {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.55rem;
}
.toolbar-vsep {
  flex: 0 0 1px;
  width: 1px;
  min-height: 2.35rem;
  align-self: stretch;
  margin: 0 0.15rem;
  background: var(--wb-line, rgba(148, 163, 184, 0.22));
  border-radius: 1px;
}
.toolbar-filters-shell {
  padding: 0.55rem 0.65rem;
  border-radius: 12px;
  background: var(--wb-panel-soft, #f8fbff);
  border: 1px solid var(--wb-line, rgba(148, 163, 184, 0.22));
}
.toolbar-filters {
  display: grid;
  grid-template-columns: minmax(280px, 1.25fr) minmax(240px, 1fr);
  align-items: start;
  gap: 0.5rem;
  padding-top: 0;
}
.toolbar-filters-primary,
.toolbar-filters-secondary {
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  gap: 0.5rem;
  min-width: 0;
}
.toolbar-filters-secondary {
  justify-content: flex-end;
}
.toolbar-actions {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  flex-shrink: 0;
}
.active-env-banner {
  display: inline-flex;
  align-items: center;
  flex-wrap: wrap;
  flex: 0 1 auto;
  max-width: min(100%, 420px);
  min-width: 0;
  gap: 0.36rem;
  padding: 0.38rem 0.56rem;
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--kf-primary) 22%, #fff);
  background:
    radial-gradient(circle at top right, rgba(37, 99, 235, 0.12), transparent 38%),
    linear-gradient(135deg, #eff6ff, #f8fafc 72%);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.85),
    0 4px 14px rgba(37, 99, 235, 0.06);
}
.env-name {
  font-weight: 800;
  font-size: 0.84rem;
  color: var(--wb-text-primary, #0f172a);
  max-width: 220px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  letter-spacing: -0.01em;
}
.active-env-state-tag :deep(.n-tag__icon),
.active-env-state-tag :deep(.n-tag__content) {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
}
.state-dot {
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: currentColor;
}
.state-dot-connecting {
  animation: toolbar-state-pulse 1s ease-in-out infinite;
}
@keyframes toolbar-state-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.35; }
}
.active-env-meta {
  max-width: 220px;
}
.active-env-meta.subtle :deep(.n-tag__content) {
  color: var(--kf-text-secondary);
}
.combobox-wrap {
  position: relative;
}
.combobox-trigger {
  display: inline-flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.55rem;
  padding: 0.38rem 0.58rem;
  border: 1px solid var(--kf-border);
  border-radius: var(--wb-ctrl-radius);
  background: var(--wb-panel, #fff);
  font-size: var(--wb-ctrl-font);
  color: var(--wb-text-secondary, #66768f);
  cursor: pointer;
  min-width: 0;
  min-height: var(--wb-ctrl-height);
  box-shadow: 0 1px 2px rgba(15, 23, 42, 0.03);
  transition: border-color 0.16s ease, background-color 0.16s ease, box-shadow 0.16s ease;
}
.combobox-trigger:hover {
  background: var(--kf-bg-soft);
  border-color: var(--kf-border-strong);
}
.combobox-trigger:focus-visible {
  outline: none;
  border-color: #2563eb;
  box-shadow: var(--wb-focus-ring);
}
.combobox-trigger-strong {
  min-width: 156px;
}
.combobox-trigger.disabled {
  opacity: 0.6;
  cursor: not-allowed;
  background: #f8fafc;
}
.combobox-trigger.open {
  border-color: #2563eb;
  background: #eff6ff;
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.14);
}
.combobox-trigger-main {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  min-width: 0;
}
.combobox-label {
  color: var(--wb-text-secondary, #66768f);
  font-size: 0.68rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
}
.combobox-value {
  max-width: 170px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--wb-text-primary, #0f172a);
  font-weight: 700;
  line-height: 1.25;
}
.combobox-arrow {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.2rem;
  height: 1.2rem;
  border-radius: 999px;
  background: #f1f5f9;
  font-size: 0.62rem;
  color: #64748b;
  flex-shrink: 0;
}
.combobox-menu {
  min-width: 220px;
  width: min(360px, calc(100vw - 32px));
  max-width: min(360px, calc(100vw - 32px));
  max-height: 420px;
  overflow-y: auto;
  overflow-x: hidden;
  background: var(--wb-panel-elevated, #fff);
  border: 1px solid var(--wb-line, rgba(148, 163, 184, 0.22));
  border-radius: 12px;
  box-shadow: 0 18px 40px rgba(15, 23, 42, 0.16);
  padding: 0.3rem 0;
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
.combobox-search :deep(.n-input) {
  --n-border-radius: var(--wb-ctrl-radius);
  --n-color: #f8fafc;
  --n-color-focus: #ffffff;
}
.filter-input {
  min-width: 0;
  width: 100%;
}
.filter-input-label {
  min-width: 0;
  width: 100%;
}
.node-select {
  min-width: 150px;
  max-width: 240px;
  width: 100%;
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
  border-radius: var(--wb-ctrl-radius);
  margin: 0 0.35rem;
  transition: background-color 0.16s ease, color 0.16s ease, box-shadow 0.16s ease;
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
.combobox-item-favorite {
  background: rgba(255, 247, 237, 0.72);
}
.combobox-item-favorite:hover {
  background: rgba(255, 237, 213, 0.92);
}
.combobox-item-recent {
  background: rgba(236, 254, 255, 0.78);
}
.combobox-item-recent:hover {
  background: rgba(165, 243, 252, 0.86);
}
.combobox-item:focus-visible {
  outline: none;
  box-shadow: inset 0 0 0 2px rgba(37, 99, 235, 0.32);
}
.combobox-item.active {
  background: rgba(37, 99, 235, 0.14);
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
  border-radius: var(--wb-ctrl-radius);
  padding: 0.2rem 0.6rem;
  font-size: 0.75rem;
  line-height: 1.2;
  cursor: pointer;
}
.recent-kind-pill:hover {
  background: #dbeafe;
}
.recent-kind-pill:focus-visible {
  outline: none;
  box-shadow: var(--wb-focus-ring);
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
.btn-secondary-outline {
}
.btn-danger-outline {
}
.btn-watch {
  border: 1px solid rgba(148, 163, 184, 0.35);
  color: #475569;
  background: rgba(255, 255, 255, 0.86);
  --n-height: 34px;
  --n-border-radius: 10px;
  transition: border-color 0.16s ease, background-color 0.16s ease, color 0.16s ease, box-shadow 0.16s ease;
}
.btn-watch :deep(.n-button__content) {
  display: inline-flex;
  align-items: center;
  gap: 0.36rem;
}
.watch-dot {
  width: 0.46rem;
  height: 0.46rem;
  border-radius: 999px;
  background: #94a3b8;
  box-shadow: 0 0 0 2px rgba(148, 163, 184, 0.22);
  transition: all 0.18s ease;
}
.btn-watch.active {
  color: #166534;
  border-color: rgba(34, 197, 94, 0.45);
  background: rgba(240, 253, 244, 0.92);
}
.btn-watch.active .watch-dot {
  background: #16a34a;
  box-shadow: 0 0 0 2px rgba(34, 197, 94, 0.24);
}
.btn-watch:focus-visible {
  outline: none;
  box-shadow: var(--wb-focus-ring);
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
  cursor: pointer;
  max-width: 260px;
}
.filter-chip :deep(.n-tag__content) {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  min-width: 0;
  overflow: hidden;
}
.filter-chip-name {
  font-weight: 600;
  opacity: 0.8;
  flex-shrink: 0;
}
.filter-chip-value {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}
.filter-chip-clear-all {
  border: none;
  background: transparent;
  color: #64748b;
  font-size: 0.75rem;
  cursor: pointer;
  text-decoration: underline;
  padding: 0.1rem 0.2rem;
  border-radius: 6px;
}
.filter-chip-clear-all:focus-visible {
  outline: none;
  box-shadow: var(--wb-focus-ring);
}
@media (max-width: 960px) {
  .active-env-banner {
    display: flex;
    width: 100%;
    max-width: none;
  }
  .toolbar-filters {
    grid-template-columns: 1fr;
  }
  .toolbar-filters-secondary {
    justify-content: flex-start;
    flex-wrap: wrap;
  }
  .toolbar-vsep {
    display: none;
  }
}
</style>
