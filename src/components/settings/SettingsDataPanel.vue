<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import {
  NAlert,
  NButton,
  NCard,
  NPopconfirm,
  NSpace,
  NTag,
} from "naive-ui";
import { kfSpace } from "../../kf";
import { logDelete, logListFiles } from "../../api/log";
import { storageDeleteSshBackups, storageGetDiskUsage, type StorageDiskUsage } from "../../api/storage";
import { useOrchestratorStore } from "../../stores/orchestrator";
import { useSnapshotCenterStore } from "../../stores/snapshotCenter";
import { countResourceSnapshots } from "../../stores/resourceSnapshots";
import {
  collectLocalStorageUsage,
  type LocalStorageCategoryUsage,
} from "../../utils/localStorageUsage";
import { formatBytes } from "../../utils/formatBytes";
import { extractErrorMessage } from "../../utils/errorMessage";

const emit = defineEmits<{
  openWorkspace: [];
  openDebug: [];
}>();

const { requestSwitchToSnapshotCenter } = useSnapshotCenterStore();
const { requestSwitchToOrchestrator, clearImportBatchesRecords } = useOrchestratorStore();

const loading = ref(false);
const diskUsage = ref<StorageDiskUsage | null>(null);
const localCategories = ref<LocalStorageCategoryUsage[]>([]);
const localTotalBytes = ref(0);
const message = ref<string | null>(null);
const messageIsError = ref(false);

const diskTotalBytes = computed(() => {
  const disk = diskUsage.value;
  if (!disk) return 0;
  const appFiles = disk.appFiles.reduce((sum, file) => sum + file.bytes, 0);
  return disk.debugLogs.totalBytes + appFiles + disk.sshBackups.totalBytes;
});

const accountedTotalBytes = computed(() => localTotalBytes.value + diskTotalBytes.value);

function showMessage(text: string, isError = false) {
  message.value = text;
  messageIsError.value = isError;
  window.setTimeout(() => {
    message.value = null;
  }, 4000);
}

async function refreshUsage() {
  loading.value = true;
  try {
    diskUsage.value = await storageGetDiskUsage();
    const local = collectLocalStorageUsage();
    localCategories.value = local.categories;
    localTotalBytes.value = local.totalBytes;
  } catch (e) {
    showMessage(extractErrorMessage(e), true);
  } finally {
    loading.value = false;
  }
}

function handleManage(action: LocalStorageCategoryUsage["manageAction"]) {
  if (action === "snapshotCenter") {
    requestSwitchToSnapshotCenter();
    return;
  }
  if (action === "orchestrator") {
    requestSwitchToOrchestrator();
    return;
  }
  if (action === "workspace") {
    emit("openWorkspace");
  }
}

async function deleteHistoricalDebugLogs() {
  loading.value = true;
  try {
    const files = await logListFiles();
    const stale = files.filter((file) => !file.isCurrent);
    let deleted = 0;
    for (const file of stale) {
      await logDelete(file.fileName);
      deleted += 1;
    }
    showMessage(deleted > 0 ? `已删除 ${deleted} 个历史调试日志文件` : "没有可删除的历史调试日志");
    await refreshUsage();
  } catch (e) {
    showMessage(extractErrorMessage(e), true);
  } finally {
    loading.value = false;
  }
}

async function deleteSshBackups() {
  loading.value = true;
  try {
    const deleted = await storageDeleteSshBackups();
    showMessage(deleted > 0 ? `已删除 ${deleted} 个 SSH 配置备份` : "没有可删除的 SSH 配置备份");
    await refreshUsage();
  } catch (e) {
    showMessage(extractErrorMessage(e), true);
  } finally {
    loading.value = false;
  }
}

function clearImportBatches() {
  clearImportBatchesRecords();
  showMessage("已清除编排导入记录");
  void refreshUsage();
}

onMounted(() => {
  void refreshUsage();
});
</script>

<template>
  <div class="settings-data-panel">
    <NCard title="存储概览" size="small" class="settings-card settings-card-wide" :bordered="true">
      <p class="card-desc">
        统计应用本地数据占用。浏览器缓存（WebView 编译缓存等）不在此列；清理 localStorage 数据后，建议完全退出并重启应用以释放底层磁盘空间。
      </p>
      <NSpace v-bind="kfSpace.buttonGroup">
        <NButton :loading="loading" @click="refreshUsage">刷新统计</NButton>
      </NSpace>
      <div class="summary-row">
        <div class="summary-item">
          <span class="summary-label">浏览器本地数据</span>
          <strong class="summary-value">{{ formatBytes(localTotalBytes) }}</strong>
        </div>
        <div class="summary-item">
          <span class="summary-label">应用目录磁盘数据</span>
          <strong class="summary-value">{{ formatBytes(diskTotalBytes) }}</strong>
        </div>
        <div class="summary-item summary-item-total">
          <span class="summary-label">已统计合计</span>
          <strong class="summary-value">{{ formatBytes(accountedTotalBytes) }}</strong>
        </div>
      </div>
      <NAlert v-if="message" class="msg-alert" :type="messageIsError ? 'error' : 'success'" :show-icon="true">
        {{ message }}
      </NAlert>
    </NCard>

    <NCard title="浏览器本地数据" size="small" class="settings-card settings-card-wide" :bordered="true">
      <p class="card-desc">保存在 WebView localStorage 中的 YAML 快照与编排数据，通常是占用增长的主要来源。</p>
      <div v-for="item in localCategories" :key="item.id" class="usage-row">
        <div class="usage-copy">
          <div class="usage-title-row">
            <span class="setting-title">{{ item.label }}</span>
            <NTag size="small" :bordered="false">{{ formatBytes(item.bytes) }}</NTag>
          </div>
          <p class="setting-desc">{{ item.description }}</p>
          <p class="usage-detail">{{ item.detail }}</p>
        </div>
        <NSpace v-bind="kfSpace.settingInline" class="usage-actions">
          <NButton
            v-if="item.manageAction === 'snapshotCenter'"
            size="small"
            @click="handleManage(item.manageAction)"
          >
            打开快照中心
          </NButton>
          <NButton
            v-else-if="item.manageAction === 'orchestrator'"
            size="small"
            @click="handleManage(item.manageAction)"
          >
            打开编排中心
          </NButton>
          <NButton
            v-else-if="item.manageAction === 'workspace'"
            size="small"
            @click="handleManage(item.manageAction)"
          >
            工作流设置
          </NButton>
          <NPopconfirm v-if="item.clearable" @positive-click="clearImportBatches">
            <template #trigger>
              <NButton size="small" type="warning" ghost :disabled="loading">清除记录</NButton>
            </template>
            仅删除导入批次元数据，不影响 Manifest 与应用包，确认继续？
          </NPopconfirm>
        </NSpace>
      </div>
      <div v-if="localCategories.find((item) => item.id === 'resource-snapshots')" class="usage-footnote">
        当前共 {{ countResourceSnapshots() }} 条资源快照。可在
        <NButton text type="primary" @click="emit('openWorkspace')">工作流</NButton>
        调整自动快照策略。
      </div>
    </NCard>

    <NCard title="应用目录磁盘数据" size="small" class="settings-card settings-card-wide" :bordered="true">
      <p v-if="diskUsage" class="card-desc card-desc-mono">目录：{{ diskUsage.appDataDir }}</p>

      <div class="usage-row">
        <div class="usage-copy">
          <div class="usage-title-row">
            <span class="setting-title">调试日志</span>
            <NTag size="small" :bordered="false">
              {{ formatBytes(diskUsage?.debugLogs.totalBytes ?? 0) }}
            </NTag>
          </div>
          <p class="setting-desc">
            每次启动会新建一个日志文件；默认日志级别为关闭，文件通常较小。
          </p>
          <p class="usage-detail">
            {{ diskUsage?.debugLogs.fileCount ?? 0 }} 个文件
            <template v-if="diskUsage?.debugLogs.path"> · {{ diskUsage.debugLogs.path }}</template>
          </p>
        </div>
        <NSpace v-bind="kfSpace.settingInline" class="usage-actions">
          <NButton size="small" @click="emit('openDebug')">调试设置</NButton>
          <NPopconfirm @positive-click="deleteHistoricalDebugLogs">
            <template #trigger>
              <NButton size="small" type="warning" ghost :disabled="loading || (diskUsage?.debugLogs.fileCount ?? 0) <= 1">
                删除历史日志
              </NButton>
            </template>
            将删除除当前会话外的所有调试日志文件，确认继续？
          </NPopconfirm>
        </NSpace>
      </div>

      <div class="usage-row">
        <div class="usage-copy">
          <div class="usage-title-row">
            <span class="setting-title">应用配置文件</span>
            <NTag size="small" :bordered="false">
              {{ formatBytes(diskUsage?.appFiles.reduce((sum, file) => sum + file.bytes, 0) ?? 0) }}
            </NTag>
          </div>
          <p class="setting-desc">环境配置、应用设置与凭证快照等，体积通常很小。</p>
          <p v-if="diskUsage?.appFiles.length" class="usage-detail">
            {{ diskUsage.appFiles.map((file) => `${file.name} ${formatBytes(file.bytes)}`).join(" · ") }}
          </p>
          <p v-else class="usage-detail">暂无配置文件</p>
        </div>
      </div>

      <div class="usage-row">
        <div class="usage-copy">
          <div class="usage-title-row">
            <span class="setting-title">SSH 配置备份</span>
            <NTag size="small" :bordered="false">
              {{ formatBytes(diskUsage?.sshBackups.totalBytes ?? 0) }}
            </NTag>
          </div>
          <p class="setting-desc">可视化编辑 ~/.ssh/config 时自动生成的备份，不影响当前配置。</p>
          <p class="usage-detail">
            {{ diskUsage?.sshBackups.fileCount ?? 0 }} 个备份文件
            <template v-if="diskUsage?.sshBackups.path"> · {{ diskUsage.sshBackups.path }}</template>
          </p>
        </div>
        <NSpace v-bind="kfSpace.settingInline" class="usage-actions">
          <NPopconfirm @positive-click="deleteSshBackups">
            <template #trigger>
              <NButton size="small" type="warning" ghost :disabled="loading || !(diskUsage?.sshBackups.fileCount)">
                删除全部备份
              </NButton>
            </template>
            将删除所有 config.kube-flow.*.bak 备份文件，确认继续？
          </NPopconfirm>
        </NSpace>
      </div>
    </NCard>
  </div>
</template>

<style scoped>
.settings-card-wide {
  max-width: 720px;
}
.card-desc-mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.8125rem;
  word-break: break-all;
}
.summary-row {
  display: flex;
  flex-wrap: wrap;
  gap: 1rem;
  margin-top: 1rem;
  padding: 0.875rem 1rem;
  border-radius: 0.75rem;
  background: var(--kf-bg-soft, #f8fafc);
  border: 1px solid var(--kf-border, #e2e8f0);
}
.summary-item {
  min-width: 140px;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}
.summary-item-total .summary-value {
  color: var(--kf-accent, #2563eb);
}
.summary-label {
  font-size: 0.8125rem;
  color: var(--kf-text-secondary, #64748b);
}
.summary-value {
  font-size: 1.125rem;
  font-weight: 600;
  color: var(--kf-text-primary, #0f172a);
}
.usage-row {
  margin-top: 1rem;
  padding-top: 1rem;
  border-top: 1px solid var(--kf-border, #e2e8f0);
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;
}
.usage-copy {
  flex: 1;
  min-width: 240px;
}
.usage-title-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
}
.usage-detail {
  margin: 0.35rem 0 0;
  font-size: 0.8125rem;
  color: #64748b;
}
.usage-actions {
  flex-shrink: 0;
}
.usage-footnote {
  margin-top: 1rem;
  font-size: 0.8125rem;
  color: #64748b;
}
.msg-alert {
  margin-top: 0.75rem;
}
.setting-title {
  font-size: 0.875rem;
  font-weight: 600;
  color: #1e293b;
}
.setting-desc {
  margin: 0.3rem 0 0;
  font-size: 0.8125rem;
  line-height: 1.5;
  color: #64748b;
}
.card-desc {
  margin: 0 0 1rem;
  font-size: 0.875rem;
  color: var(--kf-text-secondary, #64748b);
  line-height: 1.55;
}
</style>
