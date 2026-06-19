<script setup lang="ts">
import { ref, computed } from "vue";
import * as jsYaml from "js-yaml";
import { NButton, NInput, useMessage } from "naive-ui";
import BaseModal from "./base/BaseModal.vue";
import ValueEditor from "./ValueEditor.vue";
import KvBulkImportModal from "./KvBulkImportModal.vue";
import { useKvEditor, type KeyValueRow } from "../composables/useKvEditor";
import { getFormatHint, dumpInlineScalar, renderSection } from "../utils/kvValidation";


interface SecretRow extends KeyValueRow {
  rawBase64: string;
}

const props = withDefaults(
  defineProps<{
    rawYaml: string;
    saving: boolean;
    hideApply?: boolean;
  }>(),
  { hideApply: false },
);

const emit = defineEmits<{
  (e: "save", yaml: string): void;
  (e: "error", message: string): void;
  (e: "update:yaml", yaml: string): void;
}>();

const secretType = ref("Opaque");
const bulkImportVisible = ref(false);
const message = useMessage();

function decodeBase64(s: string): string {
  try {
    const binary = atob(s);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
    return new TextDecoder().decode(bytes);
  } catch {
    try {
      return atob(s);
    } catch {
      return s;
    }
  }
}

function encodeBase64(s: string): string {
  try {
    const bytes = new TextEncoder().encode(s);
    let binary = "";
    for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
    return btoa(binary);
  } catch {
    return "";
  }
}

const {
  rows,
  selectedIndex,
  formatConfirmKeys,
  controlCharConfirmKeys,
  sidebarCollapsed,
  effectiveWhitespace,
  localWhitespaceOverride,
  whitespaceRenderEnabled,
  hasEmptyRow,
  selectedRow,
  addRow,
  removeRow,
  replaceRows,
  onSave,
  validateFormat,
  onFormatConfirmApply,
  onFormatConfirmCancel,
  onControlCharConfirmApply,
  onControlCharConfirmCancel,
  hasCharIssue,
} = useKvEditor({
  rawYaml: () => props.rawYaml,
  parseYaml(raw, rowsRef, metadataRef, selectedIndexRef) {
    if (!raw.trim()) {
      rowsRef.value = [];
      metadataRef.value = {};
      secretType.value = "Opaque";
      selectedIndexRef.value = null;
      return null;
    }
    try {
      const obj = jsYaml.load(raw) as Record<string, unknown>;
      if (!obj || typeof obj !== "object") return null;
      metadataRef.value = { metadata: obj.metadata };
      secretType.value = (obj.type as string) || "Opaque";
      const data = (obj.data as Record<string, string>) || {};
      const stringData = (obj.stringData as Record<string, string>) || {};
      const keys = new Set([...Object.keys(data), ...Object.keys(stringData)]);
      rowsRef.value = [...keys].map((k) => {
        const raw = data[k];
        const plain = stringData[k] ?? decodeBase64(raw || "");
        const rawBase64 = raw || (plain ? encodeBase64(plain) : "");
        return {
          key: k,
          value: typeof plain === "string" ? plain : String(plain),
          rawBase64: rawBase64 || "",
        };
      });
      if (rowsRef.value.length > 0 && selectedIndexRef.value === null) {
        selectedIndexRef.value = 0;
      }
      return {};
    } catch {
      return null;
    }
  },
  buildYaml(currentRows, currentMetadata) {
    const meta = (currentMetadata.metadata as Record<string, unknown>) || {};
    const metadataYaml = jsYaml.dump(meta, { lineWidth: -1 }).trimEnd();
    const lines = [
      "apiVersion: v1",
      "kind: Secret",
      "metadata:",
      ...metadataYaml.split("\n").map((line: string) => `  ${line}`),
      `type: ${dumpInlineScalar(secretType.value)}`,
      ...renderSection("stringData", currentRows),
    ];
    return `${lines.join("\n")}\n`;
  },
  emit,
});

function onFormatCheck() {
  const invalid = validateFormat();
  if (invalid.length === 0) {
    window.alert("所有配置项格式正确");
  }
}

const secretSelectedRow = computed(() => selectedRow.value as SecretRow | null);

defineExpose({
  save: onSave,
  hasEmptyRow,
});

function toSecretRows(imported: KeyValueRow[]): SecretRow[] {
  return imported.map((row) => ({
    key: row.key,
    value: row.value,
    rawBase64: "",
  }));
}

function onBulkImport(payload: {
  rows: KeyValueRow[];
  added: number;
  skipped: number;
  overwritten: number;
}) {
  const secretRows = toSecretRows(payload.rows);
  const lastImported = secretRows[secretRows.length - 1]?.key ?? null;
  replaceRows(secretRows, lastImported);
  const parts: string[] = [];
  if (payload.added) parts.push(`新增 ${payload.added}`);
  if (payload.overwritten) parts.push(`覆盖 ${payload.overwritten}`);
  if (payload.skipped) parts.push(`跳过 ${payload.skipped}`);
  message.success(parts.length ? `导入完成：${parts.join("，")}` : "没有可导入的配置项");
}
</script>

<template>
  <div class="kv-editor">
    <div class="kv-toolbar">
      <span class="kv-toolbar-title">Type: {{ secretType }} · Data（stringData 明文，提交时自动 base64）· 共 {{ rows.length }} 项</span>
      <div class="kv-toolbar-actions">
        <NButton
          size="small"
          :secondary="!effectiveWhitespace"
          :type="effectiveWhitespace ? 'primary' : 'default'"
          class="ws-toggle-btn"
          title="显示/隐藏空白字符"
          @click="localWhitespaceOverride = !(localWhitespaceOverride ?? whitespaceRenderEnabled)"
        >
          <template #icon>
            <span class="ws-toggle-icon">¶</span>
          </template>
        </NButton>
        <NButton
          size="small"
          title="格式校验"
          :disabled="hasEmptyRow"
          @click="onFormatCheck"
        >
          <template #icon>
            <span class="format-check-icon">✓</span>
          </template>
        </NButton>
        <NButton size="small" secondary @click="bulkImportVisible = true">
          批量导入
        </NButton>
        <NButton
          v-if="!hideApply"
          type="primary"
          :disabled="saving || hasEmptyRow"
          :loading="saving"
          @click="onSave"
        >
          应用
        </NButton>
      </div>
    </div>
    <div class="kv-main">
      <aside class="kv-sidebar" :class="{ collapsed: sidebarCollapsed }">
        <div class="kv-sidebar-header" @click="sidebarCollapsed = !sidebarCollapsed">
          <span class="kv-sidebar-icon" aria-hidden="true">{{ sidebarCollapsed ? "»" : "«" }}</span>
          <span v-if="!sidebarCollapsed" class="kv-sidebar-title">配置项</span>
        </div>
        <template v-if="!sidebarCollapsed">
          <div class="kv-list">
            <div
              v-for="(row, i) in rows"
              :key="i"
              role="button"
              tabindex="0"
              class="kv-item"
              :class="{ active: selectedIndex === i }"
              @click="selectedIndex = i"
              @keydown.enter.space.prevent="selectedIndex = i"
            >
              <span class="kv-item-name">{{ row.key || "(未命名)" }}</span>
              <span v-if="hasCharIssue(row.value)" class="kv-item-badge kv-item-badge-warn" title="含有控制字符或行尾空白">⚠</span>
              <span v-if="getFormatHint(row.key)" class="kv-item-badge">{{ getFormatHint(row.key) }}</span>
              <NButton
                text
                type="error"
                size="tiny"
                class="kv-item-remove"
                aria-label="删除"
                @click.stop="removeRow(i)"
              >
                ×
              </NButton>
            </div>
          </div>
          <NButton
            v-if="rows.length > 0"
            quaternary
            block
            class="kv-add"
            :disabled="hasEmptyRow"
            @click="addRow(() => ({ key: '', value: '', rawBase64: '' }))"
          >
            + 添加配置项
          </NButton>
        </template>
      </aside>
      <div class="kv-panel">
        <template v-if="secretSelectedRow">
          <div class="kv-panel-header">
            <NInput
              v-model:value="secretSelectedRow.key"
              class="kv-key-input"
              size="small"
              placeholder="例如 config.yaml"
              spellcheck="false"
            />
          </div>
          <div class="kv-panel-body">
            <ValueEditor
              :model-value="secretSelectedRow.value"
              fill-height
              :show-whitespace="effectiveWhitespace"
              @update:model-value="secretSelectedRow.value = $event"
            />
          </div>
        </template>
        <div v-else class="kv-empty">
          <p>暂无配置项</p>
          <NButton type="primary" secondary size="small" @click="addRow(() => ({ key: '', value: '', rawBase64: '' }))">
            + 添加配置项
          </NButton>
        </div>
      </div>
    </div>
    <KvBulkImportModal
      :visible="bulkImportVisible"
      :existing-rows="rows"
      @close="bulkImportVisible = false"
      @import="onBulkImport"
    />
    <BaseModal
      :visible="formatConfirmKeys.length > 0"
      title="格式校验"
      width="480px"
      @close="onFormatConfirmCancel"
    >
      <p class="format-confirm-desc">以下配置项内容与后缀格式不符：{{ formatConfirmKeys.join("、") }}。是否仍要应用？</p>
      <template #footer>
        <NButton secondary @click="onFormatConfirmCancel">取消</NButton>
        <NButton type="primary" @click="onFormatConfirmApply">仍要应用</NButton>
      </template>
    </BaseModal>
    <BaseModal
      :visible="controlCharConfirmKeys.length > 0"
      title="控制字符警告"
      width="480px"
      @close="onControlCharConfirmCancel"
    >
      <p class="format-confirm-desc">以下配置项含有非常规控制字符或行尾空白，可能导致 kubectl 编辑时显示异常：{{ controlCharConfirmKeys.join("、") }}。是否仍要应用？</p>
      <template #footer>
        <NButton secondary @click="onControlCharConfirmCancel">取消</NButton>
        <NButton type="primary" @click="onControlCharConfirmApply">仍要应用</NButton>
      </template>
    </BaseModal>
  </div>
</template>

<style scoped>
.kv-editor {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--kf-bg-soft);
  position: relative;
}
.kv-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  background: var(--kf-surface-strong);
  border-bottom: 1px solid var(--kf-border);
  flex-shrink: 0;
}
.kv-toolbar-title {
  font-size: 0.875rem;
  font-weight: 600;
  color: var(--kf-text-primary);
}
.kv-toolbar-actions {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
.ws-toggle-btn {
  font-size: 0.875rem;
}
.ws-toggle-icon {
  font-family: serif;
  font-size: 1rem;
}
.kv-main {
  flex: 1;
  min-height: 0;
  display: flex;
  overflow: hidden;
}
.kv-sidebar {
  width: 220px;
  min-width: 220px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: var(--sidebar-bg, var(--kf-bg-soft));
  border-right: 1px solid var(--border-color, var(--kf-border));
  transition: min-width 0.2s, width 0.2s;
}
.kv-sidebar.collapsed {
  width: 40px;
  min-width: 40px;
}
.kv-sidebar-header {
  padding: 0.75rem;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  cursor: pointer;
  user-select: none;
  border-bottom: 1px solid var(--border-color, var(--kf-border));
  flex-shrink: 0;
  background: var(--sidebar-bg, var(--kf-bg-soft));
}
.kv-sidebar-header:hover {
  background: var(--wb-row-hover, rgba(0, 0, 0, 0.06));
}
.kv-sidebar-icon {
  font-size: 1rem;
  color: var(--kf-text-secondary);
  flex-shrink: 0;
}
.kv-sidebar-title {
  font-size: 0.875rem;
  font-weight: 500;
}
.kv-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0.5rem;
}
.kv-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
  padding: 0.5rem 0.6rem;
  margin-bottom: 0.25rem;
  border: none;
  border-radius: 6px;
  background: transparent;
  font-size: 0.8125rem;
  color: var(--kf-text-primary);
  text-align: left;
  cursor: pointer;
  transition: background 0.15s;
}
.kv-item:hover {
  background: var(--kf-bg-elevated);
}
.kv-item.active {
  background: var(--kf-primary-soft);
  color: var(--kf-primary);
  font-weight: 500;
}
.kv-item-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.kv-item-badge {
  flex-shrink: 0;
  padding: 0.1rem 0.4rem;
  border-radius: 4px;
  font-size: 0.6875rem;
  font-weight: 500;
  text-transform: uppercase;
  background: var(--kf-border);
  color: var(--kf-text-secondary);
}
.kv-item.active .kv-item-badge {
  background: color-mix(in srgb, var(--kf-primary) 32%, var(--kf-bg-soft));
  color: var(--kf-primary);
}
.kv-item-badge-warn {
  background: var(--kf-warning-soft, #fef3c7);
  color: var(--kf-warning, #b45309);
  font-size: 0.75rem;
  text-transform: none;
}
.kv-item.active .kv-item-badge-warn {
  background: var(--kf-warning-soft, #fef3c7);
  color: var(--kf-warning, #b45309);
}
.kv-item-remove {
  flex-shrink: 0;
  width: 1.25rem;
  height: 1.25rem;
  padding: 0;
  border: none;
  border-radius: 4px;
  background: transparent;
  font-size: 1rem;
  line-height: 1;
  color: var(--kf-text-muted);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.15s;
}
.kv-item:hover .kv-item-remove {
  opacity: 1;
}
.kv-item-remove:hover {
  background: var(--kf-danger-soft);
  color: var(--kf-danger);
}
.kv-add {
  margin: 0.5rem;
  padding: 0.5rem;
  border: 1px dashed var(--kf-border);
  border-radius: 6px;
  background: var(--kf-surface-strong);
  font-size: 0.8125rem;
  color: var(--kf-text-secondary);
  cursor: pointer;
  transition: border-color 0.15s, color 0.15s;
}
.kv-add:hover:not(:disabled) {
  border-color: var(--kf-border-strong);
  color: var(--kf-text-primary);
}
.kv-add:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}
.kv-panel {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  background: var(--kf-surface-strong);
  overflow: hidden;
}
.kv-panel-header {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid var(--kf-border);
  flex-shrink: 0;
}
.kv-key-input {
  flex: 1;
  max-width: 320px;
}
.kv-key-input :deep(.n-input__input-el) {
  font-family: ui-monospace, monospace;
}
.kv-panel-body {
  flex: 1;
  min-height: 0;
  padding: 1rem;
  overflow: hidden;
}
.kv-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.65rem;
  color: var(--kf-text-muted);
  font-size: 0.875rem;
}
.format-confirm-desc {
  margin: 0 0 1.25rem;
  font-size: 0.875rem;
  color: var(--kf-text-secondary);
  line-height: 1.5;
}
</style>
