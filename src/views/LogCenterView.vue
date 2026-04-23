<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { NButton, NEmpty, NPopover, NScrollbar, NSpace, NTabs, NTabPane, NTag } from "naive-ui";
import { kfSpace } from "../kf";

defineOptions({ name: "LogCenterView" });
import LogView from "./LogView.vue";
import PodLogPanel from "../components/PodLogPanel.vue";
import WorkloadLogPanel from "../components/WorkloadLogPanel.vue";
import { useLogCenterStore } from "../stores/logCenter";
import { useAppSettingsStore } from "../stores/appSettings";
import { setLogStreamActiveLimit, touchLogStreamSession } from "../stores/logStreamManager";

const {
  sessions,
  currentSessionId,
  currentSession,
  pendingLogOpen,
  clearPendingOpen,
  openOrFocusSession,
  closeSession,
  setCurrentSession,
} = useLogCenterStore();
const { ensureAppSettingsLoaded, logActiveStreamLimit } = useAppSettingsStore();

const activePane = ref<"resource" | "debug">("resource");
const sessionRailCollapsed = ref(false);
const compareSessionMap = ref<Record<string, string>>({});
const comparePickerOpen = ref(false);
const comparePickerRef = ref<HTMLElement | null>(null);

const groupedSessions = computed(() => {
  const groups = new Map<string, { envId: string; envName: string; items: typeof sessions.value }>();
  for (const session of sessions.value) {
    const group =
      groups.get(session.envId) ??
      ({ envId: session.envId, envName: session.envName, items: [] } as {
        envId: string;
        envName: string;
        items: typeof sessions.value;
      });
    group.items.push(session);
    groups.set(session.envId, group);
  }
  return Array.from(groups.values()).sort((a, b) => a.envName.localeCompare(b.envName));
});

const currentSessionTitle = computed(() => {
  const session = currentSession.value;
  if (!session) return "";
  return session.kind === "pod"
    ? session.podName || "Pod"
    : `${session.workloadKind || "Workload"} / ${session.workloadName || "-"}`;
});

const currentSessionMeta = computed(() => {
  const session = currentSession.value;
  if (!session) return "";
  return `${session.envName} / ${session.namespace}`;
});

const compareCandidates = computed(() =>
  sessions.value.filter((session) => session.id !== currentSessionId.value)
);

const compareSessionId = computed(() => {
  const currentId = currentSessionId.value;
  if (!currentId) return "";
  return compareSessionMap.value[currentId] ?? "";
});

const compareSession = computed(() => {
  const targetId = compareSessionId.value;
  return sessions.value.find((session) => session.id === targetId) ?? null;
});

const compareEnabled = computed(() => Boolean(currentSession.value && compareSession.value));

function sessionBadge(session: (typeof sessions.value)[number]) {
  return session.kind === "pod" ? "Pod" : "Workload";
}

function sessionLabel(session: (typeof sessions.value)[number]) {
  return session.kind === "pod"
    ? session.podName || "-"
    : `${session.workloadKind || "Workload"} / ${session.workloadName || "-"}`;
}

function handlePendingOpen() {
  const pending = pendingLogOpen.value;
  if (!pending) return;
  clearPendingOpen();
  openOrFocusSession({
    kind: pending.kind,
    envId: pending.envId,
    envName: pending.envName,
    namespace: pending.namespace,
    podName: pending.podName,
    workloadKind: pending.workloadKind,
    workloadName: pending.workloadName,
  });
  activePane.value = "resource";
}

function toggleComparePicker() {
  if (!currentSession.value) return;
  comparePickerOpen.value = !comparePickerOpen.value;
}

function selectCompareSession(sessionId: string) {
  const currentId = currentSessionId.value;
  if (!currentId) return;
  compareSessionMap.value = {
    ...compareSessionMap.value,
    [currentId]: sessionId,
  };
  comparePickerOpen.value = false;
}

function clearCompareSession() {
  const currentId = currentSessionId.value;
  if (!currentId) return;
  const next = { ...compareSessionMap.value };
  delete next[currentId];
  compareSessionMap.value = next;
  comparePickerOpen.value = false;
}

function closeLogSession(sessionId: string) {
  const next = { ...compareSessionMap.value };
  delete next[sessionId];
  for (const [primaryId, compareId] of Object.entries(next)) {
    if (compareId === sessionId) {
      delete next[primaryId];
    }
  }
  compareSessionMap.value = next;
  closeSession(sessionId);
}

function handleDocumentClick(event: MouseEvent) {
  if (!comparePickerOpen.value) return;
  const target = event.target as Node | null;
  if (comparePickerRef.value && target && !comparePickerRef.value.contains(target)) {
    comparePickerOpen.value = false;
  }
}

watch(pendingLogOpen, (pending) => {
  if (pending) handlePendingOpen();
});

watch(
  () => [currentSessionId.value, sessions.value.map((session) => session.id).join(",")] as const,
  () => {
    comparePickerOpen.value = false;
    const validIds = new Set(sessions.value.map((session) => session.id));
    const next = { ...compareSessionMap.value };
    for (const [primaryId, compareId] of Object.entries(next)) {
      if (!validIds.has(primaryId) || !validIds.has(compareId) || primaryId === compareId) {
        delete next[primaryId];
      }
    }
    if (Object.keys(next).length !== Object.keys(compareSessionMap.value).length) {
      compareSessionMap.value = next;
    }
  }
);

watch(
  () => [currentSession.value?.id ?? "", compareSession.value?.id ?? "", logActiveStreamLimit.value] as const,
  ([currentId, compareId, limit]) => {
    setLogStreamActiveLimit(limit);
    if (currentId) touchLogStreamSession(currentId);
    if (compareId) touchLogStreamSession(compareId);
  },
  { immediate: true }
);

onMounted(() => {
  void ensureAppSettingsLoaded();
  if (pendingLogOpen.value) handlePendingOpen();
  window.addEventListener("click", handleDocumentClick);
});

onBeforeUnmount(() => {
  window.removeEventListener("click", handleDocumentClick);
});
</script>

<template>
  <div class="log-center">
    <aside class="session-rail" :class="{ collapsed: sessionRailCollapsed }">
      <NButton quaternary class="rail-toggle" @click="sessionRailCollapsed = !sessionRailCollapsed">
        <span>{{ sessionRailCollapsed ? "»" : "«" }}</span>
        <span v-if="!sessionRailCollapsed">日志会话</span>
      </NButton>
      <div v-if="!sessionRailCollapsed" class="session-rail-body">
        <NTabs v-model:value="activePane" type="segment" size="small" animated class="log-mode-tabs">
          <NTabPane name="resource" tab="资源日志">
            <NScrollbar class="session-scroll" trigger="hover">
              <div v-if="groupedSessions.length" class="session-groups">
                <section v-for="group in groupedSessions" :key="group.envId" class="session-group">
                  <div class="session-group-title">{{ group.envName }}</div>
                  <div
                    v-for="session in group.items"
                    :key="session.id"
                    class="session-item"
                    :class="{ active: currentSessionId === session.id }"
                  >
                    <NButton quaternary class="session-item-main-button" @click="setCurrentSession(session.id)">
                      <NTag
                        size="small"
                        round
                        :bordered="false"
                        :class="session.kind === 'pod' ? 'badge-pod' : 'badge-workload'"
                      >{{ sessionBadge(session) }}</NTag>
                      <span class="session-item-main">
                        <span class="session-item-name" :title="sessionLabel(session)">{{ sessionLabel(session) }}</span>
                        <span class="session-item-meta">{{ session.namespace }}</span>
                      </span>
                    </NButton>
                    <NButton text class="session-item-close" @click="closeLogSession(session.id)">×</NButton>
                  </div>
                </section>
              </div>
              <NEmpty
                v-else
                class="session-empty"
                description="还没有资源日志会话。你可以从工作台资源右键进入日志中心。"
              />
            </NScrollbar>
          </NTabPane>
          <NTabPane name="debug" tab="调试日志">
            <p class="session-empty session-empty-text">调试日志用于查看 kube-flow 自身的后台行为与排障记录。</p>
          </NTabPane>
        </NTabs>
      </div>
    </aside>

    <main class="log-main">
      <section v-if="activePane === 'debug'" class="log-stage debug-stage">
        <LogView :visible="true" />
      </section>

      <template v-else>
        <header v-if="currentSession" class="context-bar">
          <NSpace v-bind="kfSpace.contextBar" class="context-bar-row">
            <NSpace v-bind="kfSpace.contextBarMain" class="context-bar-main">
              <NTag
                size="small"
                round
                :bordered="false"
                :class="currentSession.kind === 'pod' ? 'ctx-pill-pod' : 'ctx-pill-workload'"
              >{{ currentSession.kind === "pod" ? "Pod 日志" : "Workload 日志" }}</NTag>
              <NTag type="info" size="small" round :bordered="false" class="context-name-pill ctx-name">
                {{ currentSessionTitle }}
              </NTag>
              <span class="context-meta">{{ currentSessionMeta }}</span>
            </NSpace>
            <div class="compare-tools" ref="comparePickerRef">
            <NPopover
              v-model:show="comparePickerOpen"
              trigger="manual"
              placement="bottom-end"
              :width="340"
              display-directive="show"
              @clickoutside="comparePickerOpen = false"
            >
              <template #trigger>
                <NButton
                  size="small"
                  :type="comparePickerOpen || compareEnabled ? 'primary' : 'default'"
                  secondary
                  @click.stop="toggleComparePicker"
                >{{ compareEnabled ? "更换对比" : "对比" }}</NButton>
              </template>
              <div class="compare-popover-inner" @click.stop>
                <div class="compare-popover-title">选择对比目标</div>
                <NButton
                  v-if="compareEnabled"
                  quaternary
                  type="error"
                  block
                  class="compare-clear"
                  @click="clearCompareSession"
                >结束对比</NButton>
                <div v-if="compareCandidates.length" class="compare-option-list">
                  <NButton
                    v-for="session in compareCandidates"
                    :key="session.id"
                    quaternary
                    block
                    class="compare-option"
                    :class="{ selected: compareSessionId === session.id }"
                    @click="selectCompareSession(session.id)"
                  >
                    <span class="compare-option-title">{{ sessionLabel(session) }}</span>
                    <span class="compare-option-meta">{{ session.envName }} / {{ session.namespace }}</span>
                  </NButton>
                </div>
                <div v-else class="compare-empty">还没有可用于对比的其他日志会话。</div>
              </div>
            </NPopover>
            </div>
          </NSpace>
        </header>

        <section v-if="currentSession" class="log-stage" :class="{ compare: compareEnabled }">
          <article class="log-pane">
            <NSpace v-if="compareEnabled" v-bind="kfSpace.logPaneHead" class="log-pane-head">
              <span class="log-pane-title">{{ currentSessionTitle }}</span>
              <span class="log-pane-meta">{{ currentSessionMeta }}</span>
            </NSpace>
            <KeepAlive :max="12">
              <WorkloadLogPanel
                v-if="currentSession.kind === 'workload'"
                :key="currentSession.id"
                :session-id="currentSession.id"
                :env-id="currentSession.envId"
                :namespace="currentSession.namespace"
                :workload-kind="currentSession.workloadKind || ''"
                :workload-name="currentSession.workloadName || ''"
                class="log-session-panel"
              />
              <PodLogPanel
                v-else
                :key="currentSession.id"
                :session-id="currentSession.id"
                :env-id="currentSession.envId"
                :namespace="currentSession.namespace"
                :pod-name="currentSession.podName || ''"
                class="log-session-panel"
              />
            </KeepAlive>
          </article>

          <article v-if="compareSession" class="log-pane secondary">
            <div class="log-pane-head">
              <span class="log-pane-title">{{ sessionLabel(compareSession) }}</span>
              <span class="log-pane-meta">{{ compareSession.envName }} / {{ compareSession.namespace }}</span>
            </div>
            <KeepAlive :max="12">
              <WorkloadLogPanel
                v-if="compareSession.kind === 'workload'"
                :key="compareSession.id"
                :session-id="compareSession.id"
                :env-id="compareSession.envId"
                :namespace="compareSession.namespace"
                :workload-kind="compareSession.workloadKind || ''"
                :workload-name="compareSession.workloadName || ''"
                class="log-session-panel"
              />
              <PodLogPanel
                v-else
                :key="compareSession.id"
                :session-id="compareSession.id"
                :env-id="compareSession.envId"
                :namespace="compareSession.namespace"
                :pod-name="compareSession.podName || ''"
                class="log-session-panel"
              />
            </KeepAlive>
          </article>
        </section>

        <section v-else class="log-empty">
          <div class="empty-kicker">日志中心</div>
          <h3>集中查看 Pod 与 Workload 日志</h3>
          <p>从工作台资源右键进入后，会在这里保留日志会话，便于持续排查与切换比对。</p>
        </section>
      </template>
    </main>
  </div>
</template>

<style scoped>
.log-center {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  background:
    radial-gradient(circle at top left, color-mix(in srgb, var(--kf-primary) 12%, transparent), transparent 28%),
    linear-gradient(180deg, var(--wb-panel-soft) 0%, var(--kf-bg-elevated) 100%);
}

.session-rail {
  width: 320px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-right: 1px solid var(--kf-border);
  background: var(--kf-surface);
  backdrop-filter: blur(18px);
}

.session-rail.collapsed {
  width: 52px;
}

.rail-toggle {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  padding: 0.85rem 1rem;
  border: none;
  border-bottom: 1px solid var(--kf-border);
  background: transparent;
  color: var(--kf-text-primary);
  font-size: 0.875rem;
  font-weight: 700;
  cursor: pointer;
}

.session-rail-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 0.75rem 0.9rem 1rem;
  overflow: hidden;
}
.log-mode-tabs {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.log-mode-tabs :deep(.n-tabs-pane-wrapper) {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.log-mode-tabs :deep(.n-tab-pane) {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding-top: 0.75rem;
}
.session-scroll {
  flex: 1;
  min-height: 0;
  max-height: calc(100vh - 12rem);
}
.session-empty-text {
  margin: 0.5rem 0 0;
  line-height: 1.6;
}
.badge-pod {
  min-width: 58px;
  text-align: center;
  background: color-mix(in srgb, var(--kf-success) 16%, transparent) !important;
  color: var(--kf-success) !important;
}
.badge-workload {
  min-width: 58px;
  text-align: center;
  background: color-mix(in srgb, var(--kf-warning) 16%, transparent) !important;
  color: var(--kf-warning) !important;
}
.session-item-main-button {
  flex: 1;
  min-width: 0;
  height: auto !important;
  padding: 0 !important;
  justify-content: flex-start !important;
}
.session-item-main-button :deep(.n-button__content) {
  display: flex;
  align-items: center;
  gap: 0.7rem;
  width: 100%;
  min-width: 0;
}
.compare-popover-inner {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  max-height: min(400px, 70vh);
  overflow: auto;
}
.compare-clear {
  justify-content: center;
}
.ctx-pill-pod {
  background: color-mix(in srgb, var(--kf-success) 16%, transparent) !important;
  color: var(--kf-success) !important;
  font-weight: 700;
}
.ctx-pill-workload {
  background: color-mix(in srgb, var(--kf-warning) 16%, transparent) !important;
  color: var(--kf-warning) !important;
  font-weight: 700;
}
.context-name-pill.ctx-name {
  max-width: min(38vw, 420px);
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.context-bar .context-meta {
  flex-shrink: 0;
}

.session-groups {
  margin-top: 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

.session-group-title {
  margin-bottom: 0.4rem;
  font-size: 0.75rem;
  font-weight: 700;
  color: var(--kf-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.08em;
}

.session-item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.7rem;
  padding: 0.8rem 0.85rem;
  border: 1px solid var(--kf-border);
  border-radius: 16px;
  background: var(--kf-surface-strong);
}

.session-item + .session-item {
  margin-top: 0.45rem;
}

.session-item:hover {
  transform: translateY(-1px);
  box-shadow: var(--kf-shadow-sm);
}

.session-item.active {
  border-color: color-mix(in srgb, var(--kf-primary) 40%, var(--kf-border));
  box-shadow: 0 14px 30px color-mix(in srgb, var(--kf-primary) 12%, transparent);
  background: linear-gradient(135deg, var(--kf-surface-strong) 0%, var(--kf-primary-soft) 100%);
}

.session-item-main-button {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 0.7rem;
  padding: 0;
  border: none;
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.session-item-badge {
  flex-shrink: 0;
  min-width: 58px;
  padding: 0.2rem 0.45rem;
  border-radius: 999px;
  font-size: 0.6875rem;
  font-weight: 700;
  text-align: center;
}

.session-item-badge.workload {
  background: color-mix(in srgb, var(--kf-warning) 16%, transparent);
  color: var(--kf-warning);
}

.session-item-badge.pod {
  background: color-mix(in srgb, var(--kf-success) 16%, transparent);
  color: var(--kf-success);
}

.session-item-main {
  flex: 1;
  min-width: 0;
}

.session-item-name {
  display: block;
  font-size: 0.84rem;
  font-weight: 600;
  color: var(--kf-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.session-item-meta {
  display: block;
  margin-top: 0.18rem;
  font-size: 0.75rem;
  color: var(--kf-text-secondary);
}

.session-item-close {
  border: none;
  background: transparent;
  color: var(--kf-text-muted);
  font-size: 1rem;
  cursor: pointer;
}

.session-item-close:hover {
  color: var(--kf-danger);
}

.session-empty {
  margin-top: 1rem;
  padding: 1rem;
  border-radius: 16px;
  background: var(--kf-bg-soft);
  color: var(--kf-text-secondary);
  font-size: 0.8125rem;
  line-height: 1.6;
}

.log-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.context-bar {
  padding: 0.9rem 1rem;
  border-bottom: 1px solid var(--kf-border);
  background: color-mix(in srgb, var(--kf-surface-strong) 78%, transparent);
  backdrop-filter: blur(14px);
}
.context-bar-row {
  width: 100%;
  min-width: 0;
}
.context-bar-main {
  min-width: 0;
}

.context-pill {
  padding: 0.28rem 0.6rem;
  border-radius: 999px;
  font-size: 0.72rem;
  font-weight: 700;
}

.context-pill.workload {
  background: color-mix(in srgb, var(--kf-warning) 16%, transparent);
  color: var(--kf-warning);
}

.context-pill.pod {
  background: color-mix(in srgb, var(--kf-success) 16%, transparent);
  color: var(--kf-success);
}

.context-name-pill {
  max-width: min(38vw, 420px);
  padding: 0.42rem 0.8rem;
  border-radius: 14px;
  background: color-mix(in srgb, var(--kf-primary) 8%, var(--kf-mix-surface));
  color: var(--wb-chip-text);
  font-size: 0.82rem;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.context-meta {
  color: var(--kf-text-secondary);
  font-size: 0.82rem;
}

.compare-tools {
  position: relative;
  flex-shrink: 0;
}

.compare-trigger {
  padding: 0.5rem 0.9rem;
  border: 1px solid color-mix(in srgb, var(--kf-primary) 20%, var(--kf-border));
  border-radius: 12px;
  background: var(--kf-primary-soft);
  color: var(--wb-chip-text);
  font-size: 0.8rem;
  font-weight: 700;
  cursor: pointer;
}

.compare-trigger.active,
.compare-trigger:hover {
  background: color-mix(in srgb, var(--kf-primary) 16%, var(--kf-mix-surface));
  border-color: color-mix(in srgb, var(--kf-primary) 32%, var(--kf-border));
}

.compare-popover {
  position: absolute;
  top: calc(100% + 0.55rem);
  right: 0;
  z-index: 20;
  width: min(340px, 72vw);
  max-height: min(420px, 70vh);
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
  padding: 0.8rem;
  overflow: auto;
  border: 1px solid var(--kf-border);
  border-radius: 18px;
  background: var(--kf-surface-strong);
  box-shadow: var(--kf-shadow-md);
}

.compare-popover-title {
  font-size: 0.78rem;
  font-weight: 700;
  color: var(--kf-text-primary);
}

.compare-option-list {
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
}

.compare-option {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.18rem;
  width: 100%;
  padding: 0.72rem 0.8rem;
  border: 1px solid var(--kf-border);
  border-radius: 14px;
  background: var(--kf-surface-strong);
  text-align: left;
  cursor: pointer;
}

.compare-option:hover,
.compare-option.selected {
  border-color: color-mix(in srgb, var(--kf-primary) 35%, var(--kf-border));
  background: var(--kf-primary-soft);
}

.compare-option.clear {
  align-items: center;
  color: var(--kf-danger);
  font-weight: 700;
}

.compare-option-title {
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--kf-text-primary);
}

.compare-option-meta {
  font-size: 0.75rem;
  color: var(--kf-text-secondary);
}

.compare-empty {
  padding: 0.8rem;
  border-radius: 14px;
  background: var(--kf-bg-soft);
  color: var(--kf-text-secondary);
  font-size: 0.8rem;
}

.context-action {
  padding: 0.5rem 0.85rem;
  border: 1px solid var(--kf-border);
  border-radius: 12px;
  background: color-mix(in srgb, var(--kf-surface-strong) 92%, transparent);
  color: var(--kf-text-primary);
  font-size: 0.8rem;
  font-weight: 600;
  cursor: pointer;
}

.context-action:hover {
  background: var(--kf-surface-strong);
}

.log-stage {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.log-stage.compare {
  flex-direction: row;
  gap: 0.9rem;
  padding: 0.9rem;
}

.log-pane {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.log-stage.compare .log-pane {
  border: 1px solid var(--kf-border);
  border-radius: 20px;
  background: color-mix(in srgb, var(--kf-surface-strong) 88%, transparent);
  box-shadow: var(--kf-shadow-sm);
}

.log-pane-head {
  width: 100%;
  box-sizing: border-box;
  padding: 0.8rem 1rem;
  border-bottom: 1px solid var(--kf-border);
  background: color-mix(in srgb, var(--kf-bg-soft) 90%, var(--kf-surface-strong));
}

.log-pane-title {
  font-size: 0.84rem;
  font-weight: 700;
  color: var(--kf-text-primary);
}

.log-pane-meta {
  font-size: 0.78rem;
  color: var(--kf-text-secondary);
}

.log-stage :deep(.workload-log-panel),
.log-stage :deep(.pod-log-panel),
.debug-stage :deep(.log-view) {
  flex: 1;
  min-height: 0;
}

.log-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.6rem;
  padding: 2rem;
  text-align: center;
}

.empty-kicker {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--kf-primary);
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.log-empty h3 {
  margin: 0;
  font-size: 1.5rem;
  color: var(--kf-text-primary);
}

.log-empty p {
  margin: 0;
  max-width: 560px;
  line-height: 1.6;
  color: var(--kf-text-secondary);
}

@media (max-width: 960px) {
  .log-center {
    flex-direction: column;
  }

  .session-rail,
  .session-rail.collapsed {
    width: 100%;
    border-right: none;
    border-bottom: 1px solid var(--kf-border);
  }

  .context-bar-row {
    align-items: flex-start;
  }

  .compare-popover {
    left: 0;
    right: auto;
    width: min(340px, calc(100vw - 2rem));
  }

  .context-action {
    margin-left: 0;
  }

  .log-stage.compare {
    flex-direction: column;
  }
}
</style>
