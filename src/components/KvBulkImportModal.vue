<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NButton, NInput, NRadio, NRadioGroup, NTag } from "naive-ui";
import BaseModal from "./base/BaseModal.vue";
import {
  applyKvImport,
  buildImportPreview,
  entriesFromFiles,
  importModeLabel,
  parseKvImportPaste,
  type KvImportConflictPolicy,
  type KvImportDetectMode,
  type KvImportEntry,
} from "../utils/kvImport";
import type { KeyValueRow } from "../utils/kvValidation";

const props = defineProps<{
  visible: boolean;
  existingRows: KeyValueRow[];
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "import", payload: { rows: KeyValueRow[]; added: number; skipped: number; overwritten: number }): void;
}>();

const pasteText = ref("");
const defaultKey = ref("config.yaml");
const conflictPolicy = ref<KvImportConflictPolicy>("skip");
const detectMode = ref<KvImportDetectMode>("single-file");
const fileEntries = ref<KvImportEntry[]>([]);
const parseError = ref<string | null>(null);
const fileInputRef = ref<HTMLInputElement | null>(null);

const existingKeys = computed(() =>
  props.existingRows.map((r) => r.key.trim()).filter(Boolean),
);

const pasteEntries = computed<KvImportEntry[]>(() => {
  if (!pasteText.value.trim()) return [];
  const result = parseKvImportPaste(pasteText.value, defaultKey.value);
  detectMode.value = result.mode;
  parseError.value = result.error ?? null;
  return result.entries;
});

const allEntries = computed(() => [...fileEntries.value, ...pasteEntries.value]);

const preview = computed(() => buildImportPreview(allEntries.value, existingKeys.value));

const importableCount = computed(() => {
  if (conflictPolicy.value === "skip") {
    return preview.value.filter((p) => p.status === "new").length;
  }
  return preview.value.filter((p) => p.status !== "invalid").length;
});

const skippedConflictCount = computed(
  () => preview.value.filter((p) => p.status === "conflict").length,
);

const modeHint = computed(() => {
  if (fileEntries.value.length) {
    return `已选择 ${fileEntries.value.length} 个文件`;
  }
  if (!pasteText.value.trim()) return "粘贴 YAML data 段、多键映射或单个文件内容";
  return `识别为：${importModeLabel(detectMode.value)}`;
});

function resetForm() {
  pasteText.value = "";
  defaultKey.value = "config.yaml";
  conflictPolicy.value = "skip";
  fileEntries.value = [];
  parseError.value = null;
  detectMode.value = "single-file";
}

watch(
  () => props.visible,
  (open) => {
    if (open) resetForm();
  },
);

function onPickFiles() {
  fileInputRef.value?.click();
}

async function onFilesChange(event: Event) {
  const input = event.target as HTMLInputElement;
  const files = input.files ? [...input.files] : [];
  input.value = "";
  if (!files.length) return;
  try {
    fileEntries.value = await entriesFromFiles(files);
    detectMode.value = "files";
    parseError.value = null;
  } catch {
    parseError.value = "读取文件失败，请重试。";
  }
}

function clearFiles() {
  fileEntries.value = [];
}

function confirmImport() {
  if (!importableCount.value) return;
  const validEntries = allEntries.value.filter((e) => e.key.trim());
  const result = applyKvImport(props.existingRows, validEntries, conflictPolicy.value);
  emit("import", {
    rows: result.rows,
    added: result.added,
    skipped: result.skipped,
    overwritten: result.overwritten,
  });
  emit("close");
}

function valuePreview(value: string): string {
  const oneLine = value.replace(/\s+/g, " ").trim();
  if (oneLine.length <= 56) return oneLine || "（空）";
  return `${oneLine.slice(0, 56)}…`;
}
</script>

<template>
  <BaseModal :visible="visible" title="批量导入配置项" width="720px" @close="emit('close')">
    <div class="bulk-import">
      <p class="bulk-import-desc">
        在保留现有配置项的前提下合并导入。可粘贴 <code>data</code> 片段、多键 YAML，或把整份文件内容作为单个键（如 <code>config.yaml</code>）。
      </p>

      <div class="bulk-import-actions">
        <input
          ref="fileInputRef"
          type="file"
          class="bulk-import-file-input"
          multiple
          accept=".yaml,.yml,.json,.properties,.env,.toml,.ini,.conf,.txt,*/*"
          @change="onFilesChange"
        />
        <NButton size="small" secondary @click="onPickFiles">选择本地文件</NButton>
        <NButton v-if="fileEntries.length" size="small" quaternary @click="clearFiles">清除文件</NButton>
        <span class="bulk-import-mode-hint">{{ modeHint }}</span>
      </div>

      <label class="bulk-import-field">
        <span class="bulk-import-label">单文件键名</span>
        <NInput
          v-model:value="defaultKey"
          size="small"
          placeholder="config.yaml"
          spellcheck="false"
        />
        <span class="bulk-import-field-hint">粘贴整份文件内容且无法识别为多键时，将使用此键名。</span>
      </label>

      <label class="bulk-import-field">
        <span class="bulk-import-label">粘贴内容</span>
        <textarea
          v-model="pasteText"
          class="bulk-import-textarea"
          placeholder="示例 1（单文件）&#10;粘贴 config.yaml 全文…&#10;&#10;示例 2（多条）&#10;app.yaml: |&#10;  foo: bar&#10;config.json: |&#10;  {&quot;enabled&quot;: true}"
          spellcheck="false"
        />
      </label>

      <div class="bulk-import-policy">
        <span class="bulk-import-label">键名冲突时</span>
        <NRadioGroup v-model:value="conflictPolicy" size="small">
          <NRadio value="skip">保留已有，跳过冲突项</NRadio>
          <NRadio value="overwrite">覆盖已有项</NRadio>
        </NRadioGroup>
      </div>

      <p v-if="parseError" class="bulk-import-error">{{ parseError }}</p>

      <div v-if="preview.length" class="bulk-import-preview">
        <div class="bulk-import-preview-head">
          <span>预览 · {{ preview.length }} 项</span>
          <span v-if="conflictPolicy === 'skip' && skippedConflictCount">
            将跳过 {{ skippedConflictCount }} 个已存在键
          </span>
        </div>
        <div class="bulk-import-preview-list">
          <div v-for="(item, index) in preview" :key="`${item.key}-${index}`" class="bulk-import-preview-row">
            <span class="bulk-import-preview-key">{{ item.key }}</span>
            <NTag
              size="small"
              :type="item.status === 'new' ? 'success' : item.status === 'conflict' ? 'warning' : 'error'"
              :bordered="false"
            >
              {{
                item.status === "new"
                  ? "新增"
                  : item.status === "conflict"
                    ? conflictPolicy === "skip"
                      ? "跳过"
                      : "覆盖"
                    : item.reason || "无效"
              }}
            </NTag>
            <span class="bulk-import-preview-value">{{ valuePreview(item.value) }}</span>
          </div>
        </div>
      </div>
    </div>

    <template #footer>
      <NButton secondary @click="emit('close')">取消</NButton>
      <NButton type="primary" :disabled="importableCount === 0" @click="confirmImport">
        导入 {{ importableCount }} 项
      </NButton>
    </template>
  </BaseModal>
</template>

<style scoped>
.bulk-import {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
  max-height: min(62vh, 560px);
  overflow: auto;
}

.bulk-import-desc {
  margin: 0;
  font-size: 0.8125rem;
  color: var(--kf-text-secondary);
  line-height: 1.55;
}

.bulk-import-desc code {
  font-family: ui-monospace, monospace;
  font-size: 0.78rem;
  padding: 0.05rem 0.3rem;
  border-radius: 4px;
  background: var(--kf-bg-soft);
}

.bulk-import-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.bulk-import-file-input {
  display: none;
}

.bulk-import-mode-hint {
  font-size: 0.75rem;
  color: var(--kf-text-muted);
}

.bulk-import-field {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.bulk-import-label {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--kf-text-secondary);
}

.bulk-import-field-hint {
  font-size: 0.72rem;
  color: var(--kf-text-muted);
}

.bulk-import-textarea {
  width: 100%;
  min-height: 160px;
  resize: vertical;
  padding: 0.65rem 0.75rem;
  border: 1px solid var(--kf-border);
  border-radius: 8px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.78rem;
  line-height: 1.5;
  color: var(--kf-text-primary);
  background: var(--kf-bg-soft);
}

.bulk-import-textarea:focus {
  outline: none;
  border-color: color-mix(in srgb, var(--kf-primary) 45%, var(--kf-border));
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--kf-primary) 16%, transparent);
}

.bulk-import-policy {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.bulk-import-error {
  margin: 0;
  font-size: 0.78rem;
  color: var(--kf-danger);
}

.bulk-import-preview {
  border: 1px solid var(--kf-border);
  border-radius: 10px;
  overflow: hidden;
  background: var(--kf-bg-soft);
}

.bulk-import-preview-head {
  display: flex;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.45rem 0.65rem;
  font-size: 0.72rem;
  color: var(--kf-text-secondary);
  border-bottom: 1px solid var(--kf-border);
}

.bulk-import-preview-list {
  max-height: 180px;
  overflow: auto;
}

.bulk-import-preview-row {
  display: grid;
  grid-template-columns: minmax(6rem, 0.34fr) auto minmax(0, 1fr);
  gap: 0.45rem;
  align-items: center;
  padding: 0.4rem 0.65rem;
  border-bottom: 1px solid color-mix(in srgb, var(--kf-border) 70%, transparent);
  font-size: 0.75rem;
}

.bulk-import-preview-row:last-child {
  border-bottom: none;
}

.bulk-import-preview-key {
  font-family: ui-monospace, monospace;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bulk-import-preview-value {
  color: var(--kf-text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
