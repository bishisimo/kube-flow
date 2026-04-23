<script setup lang="ts">
import { ref, watch, computed } from "vue";
import { NEmpty, NSelect, NSpin } from "naive-ui";
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
    if (pods.value.length > 0 && !selectedPod.value) {
      selectedPod.value = pods.value[0].name;
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

watch(
  () => pods.value,
  (list) => {
    if (list.length > 0 && !list.some((p) => p.name === selectedPod.value)) {
      selectedPod.value = list[0].name;
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
    if (containersLoading.value) {
      return [{ label: "加载中…", value: "", disabled: true }];
    }
    return [{ label: "（无）", value: "", disabled: true }];
  }
  return [{ label: "-- 选择容器 --", value: "" }, ...list];
});
</script>

<template>
  <div class="workload-log-panel">
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
</style>
