<script setup lang="ts">
import { ref, watch, computed } from "vue";
import PodLogPanel from "./PodLogPanel.vue";
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
    podsError.value = e instanceof Error ? e.message : String(e);
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
</script>

<template>
  <div class="workload-log-panel">
    <div class="target-bar">
      <div class="toolbar-row">
        <label class="field-label">{{ workloadKindLabel }} Pod</label>
        <select
          v-model="selectedPod"
          class="pod-select"
          :disabled="podsLoading || pods.length === 0"
        >
          <option value="">-- 选择 Pod --</option>
          <option
            v-for="p in pods"
            :key="p.name"
            :value="p.name"
          >
            {{ p.name }}{{ p.phase ? ` (${p.phase})` : "" }}
          </option>
        </select>
      </div>
      <div class="toolbar-row">
        <label class="field-label">容器</label>
        <select
          v-model="selectedContainer"
          class="pod-select"
          :disabled="containersLoading || containers.length === 0"
        >
          <option value="">-- 选择容器 --</option>
          <option v-for="c in containers" :key="c" :value="c">{{ c }}</option>
        </select>
      </div>
    </div>
    <div v-if="podsError" class="error-banner">{{ podsError }}</div>
    <div v-else-if="podsLoading && pods.length === 0" class="loading-hint">
      加载 Pod 列表…
    </div>
    <div v-else-if="pods.length === 0" class="empty-hint">
      暂无 Pod
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
  border-bottom: 1px solid #e2e8f0;
  flex-shrink: 0;
}
.toolbar-row {
  display: flex;
  align-items: center;
  gap: 0.35rem;
}
.field-label {
  font-size: 0.8125rem;
  color: #64748b;
}
.pod-select {
  padding: 0.25rem 0.5rem;
  border: 1px solid #e2e8f0;
  border-radius: 4px;
  font-size: 0.8125rem;
  min-width: 180px;
  font-family: ui-monospace, monospace;
}
.error-banner {
  padding: 0.75rem;
  color: #dc2626;
  font-size: 0.8125rem;
  background: #fef2f2;
  border-radius: 6px;
  margin: 0.5rem 0;
}
.loading-hint,
.empty-hint {
  padding: 2rem;
  text-align: center;
  font-size: 0.875rem;
  color: #94a3b8;
}
.log-panel-wrap {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
</style>
