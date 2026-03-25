<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
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
      <button type="button" class="rail-toggle" @click="sessionRailCollapsed = !sessionRailCollapsed">
        <span>{{ sessionRailCollapsed ? "»" : "«" }}</span>
        <span v-if="!sessionRailCollapsed">日志会话</span>
      </button>
      <div v-if="!sessionRailCollapsed" class="session-rail-body">
        <div class="mode-switch">
          <button
            type="button"
            class="mode-btn"
            :class="{ active: activePane === 'resource' }"
            @click="activePane = 'resource'"
          >
            资源日志
          </button>
          <button
            type="button"
            class="mode-btn"
            :class="{ active: activePane === 'debug' }"
            @click="activePane = 'debug'"
          >
            调试日志
          </button>
        </div>

        <div v-if="activePane === 'resource' && groupedSessions.length" class="session-groups">
          <section v-for="group in groupedSessions" :key="group.envId" class="session-group">
            <div class="session-group-title">{{ group.envName }}</div>
            <div
              v-for="session in group.items"
              :key="session.id"
              class="session-item"
              :class="{ active: currentSessionId === session.id }"
            >
              <button type="button" class="session-item-main-button" @click="setCurrentSession(session.id)">
                <span class="session-item-badge" :class="session.kind">{{ sessionBadge(session) }}</span>
                <span class="session-item-main">
                  <span class="session-item-name" :title="sessionLabel(session)">{{ sessionLabel(session) }}</span>
                  <span class="session-item-meta">{{ session.namespace }}</span>
                </span>
              </button>
              <button type="button" class="session-item-close" @click="closeLogSession(session.id)">×</button>
            </div>
          </section>
        </div>
        <div v-else-if="activePane === 'resource'" class="session-empty">
          还没有资源日志会话。你可以从工作台资源右键进入日志中心。
        </div>
        <div v-else class="session-empty">
          调试日志用于查看 kube-flow 自身的后台行为与排障记录。
        </div>
      </div>
    </aside>

    <main class="log-main">
      <section v-if="activePane === 'debug'" class="log-stage debug-stage">
        <LogView :visible="true" />
      </section>

      <template v-else>
        <header v-if="currentSession" class="context-bar">
          <div class="context-pill" :class="currentSession.kind">
            {{ currentSession.kind === "pod" ? "Pod 日志" : "Workload 日志" }}
          </div>
          <div class="context-name-pill" :class="currentSession.kind">{{ currentSessionTitle }}</div>
          <div class="context-meta">{{ currentSessionMeta }}</div>
          <div class="compare-tools" ref="comparePickerRef">
            <button
              type="button"
              class="compare-trigger"
              :class="{ active: comparePickerOpen || compareEnabled }"
              @click.stop="toggleComparePicker"
            >
              {{ compareEnabled ? "更换对比" : "对比" }}
            </button>
            <div v-if="comparePickerOpen" class="compare-popover" @click.stop>
              <div class="compare-popover-title">选择对比目标</div>
              <button
                v-if="compareEnabled"
                type="button"
                class="compare-option clear"
                @click="clearCompareSession"
              >
                结束对比
              </button>
              <div v-if="compareCandidates.length" class="compare-option-list">
                <button
                  v-for="session in compareCandidates"
                  :key="session.id"
                  type="button"
                  class="compare-option"
                  :class="{ selected: compareSessionId === session.id }"
                  @click="selectCompareSession(session.id)"
                >
                  <span class="compare-option-title">{{ sessionLabel(session) }}</span>
                  <span class="compare-option-meta">{{ session.envName }} / {{ session.namespace }}</span>
                </button>
              </div>
              <div v-else class="compare-empty">还没有可用于对比的其他日志会话。</div>
            </div>
          </div>
        </header>

        <section v-if="currentSession" class="log-stage" :class="{ compare: compareEnabled }">
          <article class="log-pane">
            <div v-if="compareEnabled" class="log-pane-head">
              <span class="log-pane-title">{{ currentSessionTitle }}</span>
              <span class="log-pane-meta">{{ currentSessionMeta }}</span>
            </div>
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
    radial-gradient(circle at top left, rgba(37, 99, 235, 0.12), transparent 28%),
    linear-gradient(180deg, #f8fbff 0%, #eef4fb 100%);
}

.session-rail {
  width: 320px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-right: 1px solid rgba(148, 163, 184, 0.22);
  background: rgba(255, 255, 255, 0.92);
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
  border-bottom: 1px solid rgba(226, 232, 240, 0.9);
  background: transparent;
  color: #1e293b;
  font-size: 0.875rem;
  font-weight: 700;
  cursor: pointer;
}

.session-rail-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 1rem;
}

.mode-switch {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.45rem;
  padding: 0.3rem;
  border-radius: 16px;
  background: rgba(226, 232, 240, 0.72);
}

.mode-btn {
  padding: 0.58rem 0.75rem;
  border: none;
  border-radius: 12px;
  background: transparent;
  color: #64748b;
  font-size: 0.82rem;
  font-weight: 700;
  cursor: pointer;
}

.mode-btn.active {
  background: #fff;
  color: #1d4ed8;
  box-shadow: 0 6px 18px rgba(148, 163, 184, 0.16);
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
  color: #64748b;
  text-transform: uppercase;
  letter-spacing: 0.08em;
}

.session-item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.7rem;
  padding: 0.8rem 0.85rem;
  border: 1px solid rgba(226, 232, 240, 0.95);
  border-radius: 16px;
  background: #fff;
}

.session-item + .session-item {
  margin-top: 0.45rem;
}

.session-item:hover {
  transform: translateY(-1px);
  box-shadow: 0 10px 22px rgba(15, 23, 42, 0.08);
}

.session-item.active {
  border-color: rgba(37, 99, 235, 0.35);
  box-shadow: 0 14px 30px rgba(37, 99, 235, 0.12);
  background: linear-gradient(135deg, #ffffff 0%, #eff6ff 100%);
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
  background: rgba(245, 158, 11, 0.14);
  color: #b45309;
}

.session-item-badge.pod {
  background: rgba(34, 197, 94, 0.14);
  color: #15803d;
}

.session-item-main {
  flex: 1;
  min-width: 0;
}

.session-item-name {
  display: block;
  font-size: 0.84rem;
  font-weight: 600;
  color: #0f172a;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.session-item-meta {
  display: block;
  margin-top: 0.18rem;
  font-size: 0.75rem;
  color: #64748b;
}

.session-item-close {
  border: none;
  background: transparent;
  color: #94a3b8;
  font-size: 1rem;
  cursor: pointer;
}

.session-item-close:hover {
  color: #dc2626;
}

.session-empty {
  margin-top: 1rem;
  padding: 1rem;
  border-radius: 16px;
  background: rgba(248, 250, 252, 0.92);
  color: #64748b;
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
  display: flex;
  align-items: center;
  gap: 0.8rem;
  padding: 0.9rem 1rem;
  border-bottom: 1px solid rgba(226, 232, 240, 0.9);
  background: rgba(255, 255, 255, 0.78);
  backdrop-filter: blur(14px);
}

.context-pill {
  padding: 0.28rem 0.6rem;
  border-radius: 999px;
  font-size: 0.72rem;
  font-weight: 700;
}

.context-pill.workload {
  background: rgba(245, 158, 11, 0.14);
  color: #b45309;
}

.context-pill.pod {
  background: rgba(34, 197, 94, 0.14);
  color: #15803d;
}

.context-name-pill {
  max-width: min(38vw, 420px);
  padding: 0.42rem 0.8rem;
  border-radius: 14px;
  background: rgba(37, 99, 235, 0.08);
  color: #1d4ed8;
  font-size: 0.82rem;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.context-meta {
  color: #64748b;
  font-size: 0.82rem;
}

.compare-tools {
  position: relative;
  margin-left: auto;
}

.compare-trigger {
  padding: 0.5rem 0.9rem;
  border: 1px solid rgba(37, 99, 235, 0.16);
  border-radius: 12px;
  background: rgba(239, 246, 255, 0.9);
  color: #1d4ed8;
  font-size: 0.8rem;
  font-weight: 700;
  cursor: pointer;
}

.compare-trigger.active,
.compare-trigger:hover {
  background: #dbeafe;
  border-color: rgba(37, 99, 235, 0.28);
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
  border: 1px solid rgba(226, 232, 240, 0.95);
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.98);
  box-shadow: 0 20px 40px rgba(15, 23, 42, 0.14);
}

.compare-popover-title {
  font-size: 0.78rem;
  font-weight: 700;
  color: #334155;
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
  border: 1px solid rgba(226, 232, 240, 0.95);
  border-radius: 14px;
  background: #fff;
  text-align: left;
  cursor: pointer;
}

.compare-option:hover,
.compare-option.selected {
  border-color: rgba(37, 99, 235, 0.26);
  background: rgba(239, 246, 255, 0.9);
}

.compare-option.clear {
  align-items: center;
  color: #b91c1c;
  font-weight: 700;
}

.compare-option-title {
  font-size: 0.82rem;
  font-weight: 700;
  color: #0f172a;
}

.compare-option-meta {
  font-size: 0.75rem;
  color: #64748b;
}

.compare-empty {
  padding: 0.8rem;
  border-radius: 14px;
  background: rgba(248, 250, 252, 0.95);
  color: #64748b;
  font-size: 0.8rem;
}

.context-action {
  padding: 0.5rem 0.85rem;
  border: 1px solid rgba(148, 163, 184, 0.24);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.8);
  color: #334155;
  font-size: 0.8rem;
  font-weight: 600;
  cursor: pointer;
}

.context-action:hover {
  background: #fff;
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
  border: 1px solid rgba(226, 232, 240, 0.92);
  border-radius: 20px;
  background: rgba(255, 255, 255, 0.88);
  box-shadow: 0 14px 30px rgba(15, 23, 42, 0.06);
}

.log-pane-head {
  display: flex;
  align-items: center;
  gap: 0.7rem;
  padding: 0.8rem 1rem;
  border-bottom: 1px solid rgba(226, 232, 240, 0.9);
  background: rgba(248, 250, 252, 0.88);
}

.log-pane-title {
  font-size: 0.84rem;
  font-weight: 700;
  color: #0f172a;
}

.log-pane-meta {
  font-size: 0.78rem;
  color: #64748b;
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
  color: #2563eb;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.log-empty h3 {
  margin: 0;
  font-size: 1.5rem;
  color: #0f172a;
}

.log-empty p {
  margin: 0;
  max-width: 560px;
  line-height: 1.6;
  color: #64748b;
}

@media (max-width: 960px) {
  .log-center {
    flex-direction: column;
  }

  .session-rail,
  .session-rail.collapsed {
    width: 100%;
    border-right: none;
    border-bottom: 1px solid rgba(148, 163, 184, 0.22);
  }

  .context-bar {
    flex-wrap: wrap;
  }

  .compare-tools {
    margin-left: 0;
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
