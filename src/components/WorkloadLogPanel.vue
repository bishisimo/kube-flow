<script setup lang="ts">
import { ref, watch, computed } from "vue";
import { NEmpty, NSelect, NSpin, NTooltip } from "naive-ui";
import PodLogPanel from "./PodLogPanel.vue";
import { extractErrorMessage } from "../utils/errorMessage";
import {
  kubeGetPodContainers,
  kubeListPodsForWorkload,
  type PodItem,
} from "../api/kube";

const props = defineProps<{
  envId: string | null;
  namespace: string | null;
  workloadKind: string;
  workloadName: string;
  sessionId?: string;
}>();

const pods = ref<PodItem[]>([]);
const selectedPod = ref<string>("");
const podsLoading = ref(false);
const podsError = ref<string | null>(null);
const containers = ref<string[]>([]);
const selectedContainer = ref<string>("");
const containersLoading = ref(false);

const workloadKindLabel = computed(() => {
  const m: Record<string, string> = {
    Deployment: "Deployment",
    StatefulSet: "StatefulSet",
    DaemonSet: "DaemonSet",
  };
  return m[props.workloadKind] ?? props.workloadKind;
});

// ─── Pod helpers ──────────────────────────────────────────

function podPhaseColor(phase: string | null | undefined): string {
  if (!phase) return "var(--kf-text-muted)";
  switch (phase.toLowerCase()) {
    case "running": return "var(--kf-success)";
    case "pending": return "var(--kf-warning)";
    case "failed": return "var(--kf-danger)";
    case "succeeded": return "var(--kf-info)";
    case "unknown": return "var(--kf-text-muted)";
    default: return "var(--kf-text-secondary)";
  }
}

function podPhaseDotClass(phase: string | null | undefined): string {
  if (!phase) return "dot-unknown";
  switch (phase.toLowerCase()) {
    case "running": return "dot-running";
    case "pending": return "dot-pending";
    case "failed": return "dot-failed";
    case "succeeded": return "dot-succeeded";
    default: return "dot-unknown";
  }
}

function selectBestPod(podList: PodItem[]): string {
  if (podList.length === 0) return "";
  // 优先选 Running 的 Pod
  const running = podList.filter((p) => p.phase?.toLowerCase() === "running");
  if (running.length > 0) return running[0].name;
  // 其次选非 Failed 的
  const notFailed = podList.filter((p) => p.phase?.toLowerCase() !== "failed");
  if (notFailed.length > 0) return notFailed[0].name;
  return podList[0].name;
}

// ─── Pod loading ──────────────────────────────────────────

async function loadPods() {
  if (!props.envId || !props.namespace || !props.workloadName) return;
  podsLoading.value = true;
  podsError.value = null;
  pods.value = [];
  selectedPod.value = "";
  try {
    pods.value = await kubeListPodsForWorkload(
      props.envId,
      props.workloadKind,
      props.workloadName,
      props.namespace
    );
    if (pods.value.length > 0) {
      selectedPod.value = selectBestPod(pods.value);
    }
    containers.value = [];
    selectedContainer.value = "";
  } catch (e) {
    podsError.value = extractErrorMessage(e);
  } finally {
    podsLoading.value = false;
  }
}

watch(
  () => [props.envId, props.namespace, props.workloadKind, props.workloadName] as const,
  () => {
    if (props.envId && props.namespace && props.workloadName) {
      loadPods();
    } else {
      pods.value = [];
      selectedPod.value = "";
      podsError.value = null;
    }
  },
  { immediate: true }
);

// Auto-switch when selected Pod disappears
watch(
  () => pods.value,
  (list) => {
    if (list.length > 0 && !list.some((p) => p.name === selectedPod.value)) {
      selectedPod.value = selectBestPod(list);
    }
  },
  { deep: true }
);

async function loadContainers() {
  if (!props.envId || !props.namespace || !selectedPod.value) return;
  containersLoading.value = true;
  containers.value = [];
  selectedContainer.value = "";
  try {
    containers.value = await kubeGetPodContainers(
      props.envId,
      props.namespace ?? "default",
      selectedPod.value
    );
    if (containers.value.length > 0) {
      selectedContainer.value = containers.value[0];
    }
  } catch {
    containers.value = [];
  } finally {
    containersLoading.value = false;
  }
}

watch(selectedPod, () => {
  if (selectedPod.value) loadContainers();
  else {
    containers.value = [];
    selectedContainer.value = "";
  }
}, { immediate: true });

// ─── Select options (with status indicator) ───────────────

const podNOptions = computed(() => {
  if (pods.value.length === 0) {
    return [{ label: "（无）", value: "", disabled: true }];
  }
  return [
    { label: "-- 选择 Pod --", value: "" },
    ...pods.value.map((p) => ({
      value: p.name,
      label: p.phase ? `${p.name} (${p.phase})` : p.name,
    })),
  ];
});

const containerNOptions = computed(() => {
  const list = containers.value.map((c) => ({ label: c, value: c }));
  if (!list.length) {
    if (containersLoading.value) return [{ label: "加载中…", value: "", disabled: true }];
    return [{ label: "（无）", value: "", disabled: true }];
  }
  return [{ label: "-- 选择容器 --", value: "" }, ...list];
});

function onPodTabClick(podName: string) {
  selectedPod.value = podName;
}
</script>

<template>
  <div class="workload-log-panel">
    <!-- Pod selector bar -->
    <div class="target-bar">
      <div class="toolbar-row">
        <label class="field-label">{{ workloadKindLabel }} Pod</label>
        <NSelect
          v-model:value="selectedPod"
          class="select kf-select-toolbar kf-select-toolbar--wide kf-select-toolbar--mono"
          size="small"
          :options="podNOptions"
          :disabled="podsLoading || pods.length === 0"
        />
      </div>
      <div class="toolbar-row">
        <label class="field-label">容器</label>
        <NSelect
          v-model:value="selectedContainer"
          class="select kf-select-toolbar kf-select-toolbar--wide kf-select-toolbar--mono"
          size="small"
          :options="containerNOptions"
          :disabled="containersLoading || containers.length === 0"
        />
      </div>
    </div>

    <!-- Pod quick-switch tab bar -->
    <div v-if="pods.length > 1" class="pod-tab-bar">
      <NTooltip v-for="pod in pods" :key="pod.name" trigger="hover" placement="bottom">
        <template #trigger>
          <button
            type="button"
            class="pod-tab"
            :class="{ active: selectedPod === pod.name, [podPhaseDotClass(pod.phase)]: true }"
            @click="onPodTabClick(pod.name)"
          >
            <span class="pod-tab-dot" :style="{ background: podPhaseColor(pod.phase) }" />
            <span class="pod-tab-name">{{ pod.name }}</span>
          </button>
        </template>
        <div class="pod-tab-tip">
          <div>{{ pod.name }}</div>
          <div v-if="pod.phase">状态: {{ pod.phase }}</div>
          <div v-if="pod.pod_ip">IP: {{ pod.pod_ip }}</div>
          <div v-if="pod.node_name">节点: {{ pod.node_name }}</div>
        </div>
      </NTooltip>
    </div>

    <div v-if="podsError" class="error-banner">{{ podsError }}</div>
    <div v-else-if="podsLoading && pods.length === 0" class="workload-state workload-state--center">
      <NSpin size="medium" description="加载 Pod 列表…" />
    </div>
    <div v-else-if="pods.length === 0" class="workload-state workload-state--center">
      <NEmpty description="暂无 Pod" size="small" />
    </div>
    <div v-else-if="selectedPod" class="log-panel-wrap">
      <PodLogPanel
        :env-id="props.envId"
        :namespace="props.namespace ?? 'default'"
        :pod-name="selectedPod"
        :session-id="props.sessionId"
        :external-containers="containers"
        :external-container="selectedContainer"
        :external-containers-loading="containersLoading"
        @update:external-container="selectedContainer = $event"
      />
    </div>
  </div>
</template>

<style scoped>
.workload-log-panel {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.target-bar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem 1rem;
  padding: 0.5rem 0 0.5rem 1rem;
  border-bottom: 1px solid var(--kf-border);
  flex-shrink: 0;
}
.toolbar-row {
  display: flex;
  align-items: center;
  gap: 0.35rem;
}
.field-label {
  font-size: 0.8125rem;
  color: var(--kf-text-secondary);
}

/* Pod tab bar */
.pod-tab-bar {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.4rem 0.75rem;
  border-bottom: 1px solid var(--kf-border);
  overflow-x: auto;
  flex-shrink: 0;
  scrollbar-width: thin;
}
.pod-tab {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.25rem 0.6rem;
  border: 1px solid var(--kf-border);
  border-radius: 999px;
  background: transparent;
  color: var(--kf-text-secondary);
  font-size: 0.72rem;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.15s;
  font-family: ui-monospace, monospace;
}
.pod-tab:hover {
  background: var(--kf-bg-soft);
  border-color: var(--kf-text-muted);
}
.pod-tab.active {
  background: var(--kf-primary-soft);
  border-color: color-mix(in srgb, var(--kf-primary) 40%, var(--kf-border));
  color: var(--kf-primary);
}
.pod-tab-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}
.pod-tab-name {
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.pod-tab-tip {
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
  font-size: 0.78rem;
  line-height: 1.4;
}

.error-banner {
  padding: 0.75rem;
  color: var(--kf-danger);
  font-size: 0.8125rem;
  background: var(--kf-danger-soft);
  border-radius: 6px;
  margin: 0.5rem 0;
}
.workload-state {
  padding: 1.5rem 1rem;
  font-size: 0.875rem;
  color: var(--kf-text-secondary);
}
.workload-state--center {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 120px;
}
.log-panel-wrap {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

:global(:root[data-kf-chrome="dark"]) .workload-log-panel .target-bar {
  background: color-mix(in srgb, var(--kf-surface-strong) 90%, var(--kf-bg-soft));
}
:global(:root[data-kf-chrome="dark"]) .workload-log-panel .error-banner {
  background: color-mix(in srgb, var(--kf-danger) 18%, var(--kf-surface-strong));
  color: color-mix(in srgb, var(--kf-danger) 86%, var(--kf-text-primary));
}
</style>
