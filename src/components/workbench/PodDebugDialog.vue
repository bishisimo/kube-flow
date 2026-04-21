<script setup lang="ts">
import { ref, watch } from "vue";
import {
  WORKBENCH_POD_DEBUG_NAMESPACE_OPTIONS,
} from "../../features/workbench";
import type { PodDebugNamespace } from "../../api/terminal";

const props = defineProps<{
  visible: boolean;
  loading: boolean;
  error: string;
  containerOptions: string[];
}>();

const emit = defineEmits<{
  close: [];
  confirm: [
    selectedContainer: string,
    processMode: "main" | "pid",
    pid: number | null,
    namespaces: PodDebugNamespace[],
  ];
}>();

const selectedContainer = ref("");
const processMode = ref<"main" | "pid">("main");
const pidInput = ref("");
const namespaces = ref<PodDebugNamespace[]>(["net"]);
const localError = ref("");

watch(
  () => props.visible,
  (vis) => {
    if (!vis) return;
    selectedContainer.value = props.containerOptions[0] ?? "";
    processMode.value = "main";
    pidInput.value = "";
    namespaces.value = ["net"];
    localError.value = "";
  }
);

watch(
  () => props.containerOptions,
  (opts) => {
    if (opts.length && !selectedContainer.value) {
      selectedContainer.value = opts[0];
    }
  }
);

function toggleNamespace(value: PodDebugNamespace) {
  const next = new Set(namespaces.value);
  if (next.has(value)) {
    if (next.size === 1) return;
    next.delete(value);
  } else {
    next.add(value);
  }
  namespaces.value = WORKBENCH_POD_DEBUG_NAMESPACE_OPTIONS
    .map((item) => item.value)
    .filter((item) => next.has(item));
}

function onConfirm() {
  localError.value = "";
  if (processMode.value === "pid") {
    const pid = Number.parseInt(pidInput.value.trim(), 10);
    if (!Number.isInteger(pid) || pid <= 0) {
      localError.value = "请输入有效的 PID。";
      return;
    }
    emit("confirm", selectedContainer.value, processMode.value, pid, namespaces.value);
  } else {
    emit("confirm", selectedContainer.value, processMode.value, null, namespaces.value);
  }
}

const displayError = () => localError.value || props.error;
const isConfirmDisabled = () =>
  props.loading ||
  !selectedContainer.value ||
  !!displayError() ||
  (processMode.value === "pid" && !pidInput.value.trim());
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="error-modal-overlay" @click.self="emit('close')">
      <div class="pod-debug-modal" role="dialog" aria-label="进入容器调试环境">
        <h3 class="sync-orchestrator-title">进入容器调试环境</h3>
        <p class="sync-orchestrator-desc">
          先通过节点终端策略进入 Pod 所在主机，再按你勾选的 namespace 组合执行 `nsenter`。这样可以保留主机工具，同时进入目标容器的关键隔离空间。
        </p>
        <div class="pod-debug-grid">
          <label class="sync-field">
            <span>容器</span>
            <select v-model="selectedContainer" class="filter-input pod-debug-input" :disabled="loading || !containerOptions.length">
              <option value="" disabled>选择容器</option>
              <option v-for="name in containerOptions" :key="name" :value="name">
                {{ name }}
              </option>
            </select>
          </label>
          <div class="sync-field">
            <span>进程目标</span>
            <div class="pod-debug-radio-row">
              <label class="pod-debug-radio">
                <input v-model="processMode" type="radio" value="main" />
                <span>容器主进程</span>
              </label>
              <label class="pod-debug-radio">
                <input v-model="processMode" type="radio" value="pid" />
                <span>指定 PID</span>
              </label>
            </div>
          </div>
          <label v-if="processMode === 'pid'" class="sync-field">
            <span>PID</span>
            <input
              v-model="pidInput"
              type="text"
              class="filter-input pod-debug-input"
              inputmode="numeric"
              placeholder="输入目标进程 PID"
            />
          </label>
        </div>
        <div class="pod-debug-section">
          <div class="pod-debug-section-title">
            <span>Namespace 组合</span>
            <small>至少保留一个，推荐先勾选 `网络`</small>
          </div>
          <div class="pod-debug-option-grid">
            <button
              v-for="item in WORKBENCH_POD_DEBUG_NAMESPACE_OPTIONS"
              :key="item.value"
              type="button"
              class="pod-debug-option"
              :class="{ active: namespaces.includes(item.value) }"
              @click="toggleNamespace(item.value)"
            >
              <div class="pod-debug-option-head">
                <span class="pod-debug-option-title">
                  {{ item.label }}
                  <span v-if="item.recommended" class="pod-debug-badge">推荐</span>
                </span>
                <span class="pod-debug-option-check">{{ namespaces.includes(item.value) ? "✓" : "" }}</span>
              </div>
              <div class="pod-debug-option-desc">{{ item.description }}</div>
            </button>
          </div>
        </div>
        <p class="pod-debug-summary">
          当前组合：{{ namespaces.join(" + ") }}，{{ processMode === "main" ? "默认进入容器主进程" : `按指定 PID ${pidInput || "..." } 进入` }}。
        </p>
        <p v-if="displayError()" class="form-error">{{ displayError() }}</p>
        <div class="pod-debug-actions">
          <button type="button" class="btn-secondary-outline pod-debug-cancel-btn" @click="emit('close')">取消</button>
          <button
            type="button"
            class="btn-primary pod-debug-primary-btn"
            :disabled="isConfirmDisabled()"
            @click="onConfirm"
          >
            进入调试终端
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
