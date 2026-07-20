<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import { downloadDir, join } from "@tauri-apps/api/path";
import { NAlert, NButton, NCheckbox, NInput, NProgress, NSpace } from "naive-ui";
import BaseModal from "./base/BaseModal.vue";
import {
  createTransferSpeedTracker,
  formatTransferBytes,
  formatTransferSpeed,
  transferProgressPercent,
  type FileTransferProgress,
} from "../api/fileTransfer";

export type FileTransferDirection = "upload" | "download";

const props = defineProps<{
  visible: boolean;
  direction: FileTransferDirection;
  targetLabel: string;
  defaultRemotePath?: string;
  initialLocalPath?: string | null;
  transferring: boolean;
  progress: FileTransferProgress | null;
  error: string | null;
}>();

const emit = defineEmits<{
  close: [];
  start: [payload: { localPath: string; remotePath: string; overwrite: boolean }];
  cancelTransfer: [];
}>();

const localPath = ref("");
const remotePath = ref("");
const overwrite = ref(false);
const localError = ref("");
const downloadsDir = ref<string | null>(null);
const speedBytesPerSec = ref(0);
const speedTracker = createTransferSpeedTracker();

const title = computed(() => (props.direction === "upload" ? "上传文件" : "下载文件"));
const primaryLabel = computed(() => (props.direction === "upload" ? "开始上传" : "开始下载"));
const percent = computed(() =>
  transferProgressPercent(props.progress?.transferredBytes ?? 0, props.progress?.totalBytes)
);
/** SCP 等路径只有起止进度时，用不确定进度条，避免一直卡在 0%。 */
const isIndeterminate = computed(() => props.transferring && percent.value == null);
const progressText = computed(() => {
  if (!props.transferring) return "";
  const total = props.progress?.totalBytes;
  const transferred = props.progress?.transferredBytes ?? 0;
  const speedText =
    !isIndeterminate.value && speedBytesPerSec.value > 0
      ? ` · ${formatTransferSpeed(speedBytesPerSec.value)}`
      : "";
  if (isIndeterminate.value) {
    if (total != null && total > 0) {
      return `传输中 · ${formatTransferBytes(total)}`;
    }
    return "传输中…";
  }
  if (total != null) {
    return `${formatTransferBytes(transferred)} / ${formatTransferBytes(total)}${speedText}`;
  }
  return `已传输 ${formatTransferBytes(transferred)}${speedText}`;
});

watch(
  () => props.transferring,
  (busy) => {
    if (busy) {
      speedTracker.reset();
      speedBytesPerSec.value = 0;
    }
  }
);

watch(
  () => props.progress,
  (progress) => {
    if (!props.transferring || !progress) return;
    speedBytesPerSec.value = speedTracker.push(progress.transferredBytes);
  }
);

function fileNameFromPath(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || "";
}

async function ensureDownloadsDir(): Promise<string | null> {
  if (downloadsDir.value) return downloadsDir.value;
  try {
    downloadsDir.value = await downloadDir();
  } catch {
    downloadsDir.value = null;
  }
  return downloadsDir.value;
}

async function defaultDownloadLocalPath(remote: string): Promise<string> {
  const name = fileNameFromPath(remote.trim()) || "download";
  const dir = await ensureDownloadsDir();
  if (!dir) return name;
  return join(dir, name);
}

watch(
  () => props.visible,
  async (visible) => {
    if (!visible) return;
    overwrite.value = false;
    localError.value = "";
    remotePath.value = props.defaultRemotePath?.trim() || "";
    if (props.direction === "upload") {
      localPath.value = props.initialLocalPath?.trim() || "";
      return;
    }
    // 下载默认落到系统「下载」目录
    if (props.initialLocalPath?.trim()) {
      localPath.value = props.initialLocalPath.trim();
    } else {
      localPath.value = await defaultDownloadLocalPath(remotePath.value);
    }
  }
);

watch(remotePath, async (remote) => {
  if (!props.visible || props.direction !== "download" || props.transferring) return;
  // 远端文件名变化时，若本地仍在下载目录下则同步更新文件名
  const dir = await ensureDownloadsDir();
  if (!dir) return;
  const currentLocal = localPath.value.trim();
  const inDownloads =
    !currentLocal ||
    currentLocal === dir ||
    currentLocal.startsWith(`${dir}/`) ||
    currentLocal.startsWith(`${dir}\\`);
  if (!inDownloads) return;
  localPath.value = await defaultDownloadLocalPath(remote);
});

async function browseLocal() {
  localError.value = "";
  if (props.direction === "upload") {
    const selected = await open({
      multiple: false,
      directory: false,
      title: "选择要上传的文件",
    });
    if (typeof selected !== "string" || !selected) return;
    localPath.value = selected;
    const name = fileNameFromPath(selected);
    const currentRemote = remotePath.value.trim();
    if (!currentRemote) {
      remotePath.value = props.defaultRemotePath?.trim() || (name ? `/tmp/${name}` : "");
    } else if (currentRemote.endsWith("/") && name) {
      remotePath.value = `${currentRemote}${name}`;
    }
    return;
  }

  const selected = await save({
    title: "保存到本地",
    defaultPath:
      localPath.value ||
      (await defaultDownloadLocalPath(remotePath.value)) ||
      undefined,
  });
  if (typeof selected !== "string" || !selected) return;
  localPath.value = selected;
}

function onStart() {
  localError.value = "";
  const local = localPath.value.trim();
  const remote = remotePath.value.trim();
  if (!local) {
    localError.value = props.direction === "upload" ? "请选择本地文件" : "请选择本地保存路径";
    return;
  }
  if (!remote) {
    localError.value = props.direction === "upload" ? "请填写远端目标路径" : "请填写远端来源路径";
    return;
  }
  emit("start", { localPath: local, remotePath: remote, overwrite: overwrite.value });
}

function onClose() {
  if (props.transferring) return;
  emit("close");
}
</script>

<template>
  <BaseModal :visible="visible" :title="title" width="560px" @close="onClose">
    <div class="file-transfer-dialog">
      <p class="target-line">目标：{{ targetLabel }}</p>

      <label class="field">
        <span>{{ direction === "upload" ? "本地文件" : "保存到本地" }}</span>
        <NSpace :wrap="false" style="width: 100%">
          <NInput
            v-model:value="localPath"
            class="path-input"
            :disabled="transferring"
            :placeholder="direction === 'upload' ? '选择本地文件' : '选择保存路径'"
          />
          <NButton secondary :disabled="transferring" @click="browseLocal">浏览</NButton>
        </NSpace>
      </label>

      <label class="field">
        <span>{{ direction === "upload" ? "远端路径" : "远端来源路径" }}</span>
        <NInput
          v-model:value="remotePath"
          :disabled="transferring"
          :placeholder="direction === 'upload' ? '/tmp/filename' : '/path/to/remote/file'"
        />
      </label>

      <NCheckbox v-model:checked="overwrite" :disabled="transferring">
        覆盖已存在的文件
      </NCheckbox>

      <div v-if="transferring" class="progress-block">
        <NProgress
          type="line"
          :percentage="percent ?? 0"
          :processing="isIndeterminate"
          :show-indicator="!isIndeterminate"
          indicator-placement="outside"
        />
        <div class="progress-meta">{{ progressText }}</div>
      </div>

      <NAlert v-if="localError || error" class="error-msg" type="error" :show-icon="false">
        {{ localError || error }}
      </NAlert>
    </div>

    <template #footer>
      <NButton
        v-if="transferring"
        secondary
        @click="emit('cancelTransfer')"
      >取消传输</NButton>
      <NButton
        v-else
        secondary
        @click="onClose"
      >关闭</NButton>
      <NButton
        type="primary"
        :loading="transferring"
        :disabled="transferring"
        @click="onStart"
      >{{ primaryLabel }}</NButton>
    </template>
  </BaseModal>
</template>

<style scoped>
.file-transfer-dialog {
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

.target-line {
  margin: 0;
  font-size: 0.8125rem;
  color: var(--kf-text-secondary, #64748b);
}

.field {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  font-size: 0.8125rem;
  font-weight: 600;
  color: var(--kf-text-primary, #334155);
}

.path-input {
  flex: 1;
  min-width: 0;
}

.progress-block {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.progress-meta {
  font-size: 0.75rem;
  color: var(--kf-text-secondary, #64748b);
}

.error-msg {
  margin: 0;
}
</style>
