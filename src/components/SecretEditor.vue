<script setup lang="ts">
import { ref, watch, computed } from "vue";
import * as jsYaml from "js-yaml";
import { NButton, NInput } from "naive-ui";
import BaseModal from "./base/BaseModal.vue";
import { extractErrorMessage } from "../utils/errorMessage";
import ValueEditor from "./ValueEditor.vue";

export interface SecretRow {
  key: string;
  value: string;
  rawBase64: string;
}

const props = defineProps<{
  rawYaml: string;
  saving: boolean;
}>();

const emit = defineEmits<{
  (e: "save", yaml: string): void;
  (e: "error", message: string): void;
  (e: "update:yaml", yaml: string): void;
}>();

const rows = ref<SecretRow[]>([]);
const metadata = ref<Record<string, unknown>>({});
const secretType = ref("Opaque");
const selectedIndex = ref<number | null>(null);
const showDecoded = ref(false);
const formatConfirmKeys = ref<string[]>([]);
const sidebarCollapsed = ref(false);

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

function parseYaml(): boolean {
  if (!props.rawYaml.trim()) {
    rows.value = [];
    metadata.value = {};
    selectedIndex.value = null;
    return true;
  }
  try {
    const obj = jsYaml.load(props.rawYaml) as Record<string, unknown>;
    if (!obj || typeof obj !== "object") return false;
    metadata.value = { metadata: obj.metadata };
    secretType.value = (obj.type as string) || "Opaque";
    const data = (obj.data as Record<string, string>) || {};
    const stringData = (obj.stringData as Record<string, string>) || {};
    const keys = new Set([...Object.keys(data), ...Object.keys(stringData)]);
    rows.value = [...keys].map((k) => {
      const raw = data[k];
      const plain = stringData[k] ?? decodeBase64(raw || "");
      const rawBase64 = raw || (plain ? encodeBase64(plain) : "");
      return {
        key: k,
        value: typeof plain === "string" ? plain : String(plain),
        rawBase64: rawBase64 || "",
      };
    });
    if (rows.value.length > 0 && selectedIndex.value === null) {
      selectedIndex.value = 0;
    }
    return true;
  } catch {
    return false;
  }
}

watch(
  () => props.rawYaml,
  () => parseYaml(),
  { immediate: true }
);

watch(
  [rows, metadata, secretType],
  () => {
    try {
      emit("update:yaml", buildYaml());
    } catch {}
  },
  { deep: true, immediate: true }
);

const hasEmptyRow = computed(() =>
  rows.value.some((r) => !r.key.trim())
);

const selectedRow = computed(() =>
  selectedIndex.value !== null && selectedIndex.value >= 0 && selectedIndex.value < rows.value.length
    ? rows.value[selectedIndex.value]
    : null
);

function addRow() {
  const idx = rows.value.length;
  rows.value.push({ key: "", value: "", rawBase64: "" });
  selectedIndex.value = idx;
}

function removeRow(index: number) {
  rows.value.splice(index, 1);
  if (selectedIndex.value === index) {
    selectedIndex.value = rows.value.length > 0 ? Math.min(index, rows.value.length - 1) : null;
  } else if (selectedIndex.value !== null && selectedIndex.value > index) {
    selectedIndex.value--;
  }
}

function getDisplayValue(row: SecretRow): string {
  return showDecoded.value ? row.value : (row.rawBase64 || (row.value ? encodeBase64(row.value) : ""));
}

function getFormatHint(key: string): string {
  const ext = key.split(".").pop()?.toLowerCase() ?? "";
  if (["yaml", "yml"].includes(ext)) return "yaml";
  if (["json", "json5"].includes(ext)) return "json";
  return "";
}

/** 校验 value 是否符合 key 后缀所暗示的格式；返回不符合的 key 列表。空值视为通过。 */
function validateFormatBySuffix(rows: SecretRow[]): string[] {
  const invalid: string[] = [];
  for (const r of rows) {
    const k = r.key.trim();
    if (!k) continue;
    const fmt = getFormatHint(k);
    const v = r.value.trim();
    if (!v) continue;
    if (fmt === "yaml") {
      try {
        jsYaml.load(v);
      } catch {
        invalid.push(k);
      }
    } else if (fmt === "json") {
      try {
        JSON.parse(v);
      } catch {
        invalid.push(k);
      }
    }
  }
  return invalid;
}

function dumpInlineScalar(value: string): string {
  return jsYaml.dump(value, { lineWidth: -1 }).trim();
}

function renderStringEntry(key: string, value: string, indent = "  "): string[] {
  const renderedKey = dumpInlineScalar(key);
  if (!value.includes("\n")) {
    return [`${indent}${renderedKey}: ${dumpInlineScalar(value)}`];
  }
  const lines = value.split("\n");
  const hasTrailingNewline = value.endsWith("\n");
  const header = `${indent}${renderedKey}: |${hasTrailingNewline ? "" : "-"}`;
  return [header, ...lines.map((line) => `${indent}  ${line}`)];
}

function renderSection(name: string, entries: SecretRow[]): string[] {
  const validEntries = entries.filter((r) => r.key.trim());
  if (validEntries.length === 0) return [];
  const lines = [`${name}:`];
  for (const row of validEntries) {
    lines.push(...renderStringEntry(row.key.trim(), row.value));
  }
  return lines;
}

function buildYaml(): string {
  const meta = (metadata.value.metadata as Record<string, unknown>) || {};
  const metadataYaml = jsYaml.dump(meta, { lineWidth: -1 }).trimEnd();
  const lines = [
    "apiVersion: v1",
    "kind: Secret",
    "metadata:",
    ...metadataYaml.split("\n").map((line) => `  ${line}`),
    `type: ${dumpInlineScalar(secretType.value)}`,
    ...renderSection("stringData", rows.value),
  ];
  return `${lines.join("\n")}\n`;
}

function doApply() {
  try {
    const yaml = buildYaml();
    emit("save", yaml);
  } catch (e) {
    emit("error", extractErrorMessage(e));
  }
}

function onSave() {
  const dup = new Map<string, number>();
  for (const r of rows.value) {
    const k = r.key.trim();
    if (!k) continue;
    dup.set(k, (dup.get(k) || 0) + 1);
  }
  const duplicates = [...dup.entries()].filter(([, c]) => c > 1).map(([k]) => k);
  if (duplicates.length) {
    emit("error", `重复的 Key: ${duplicates.join(", ")}`);
    return;
  }
  const formatMismatch = validateFormatBySuffix(rows.value);
  if (formatMismatch.length > 0) {
    formatConfirmKeys.value = formatMismatch;
    return;
  }
  doApply();
}

function onFormatConfirmApply() {
  formatConfirmKeys.value = [];
  doApply();
}

function onFormatConfirmCancel() {
  formatConfirmKeys.value = [];
}
</script>

<template>
  <div class="kv-editor">
    <div class="kv-toolbar">
      <span class="kv-toolbar-title">Type: {{ secretType }} · Data（stringData 明文，提交时自动 base64）· 共 {{ rows.length }} 项</span>
      <div class="kv-toolbar-actions">
        <NButton
          size="small"
          :secondary="!showDecoded"
          :type="showDecoded ? 'primary' : 'default'"
          class="kv-parse-btn"
          @click="showDecoded = !showDecoded"
        >
          {{ showDecoded ? "原始" : "解析" }}
        </NButton>
        <NButton type="primary" :disabled="saving || hasEmptyRow" :loading="saving" @click="onSave">
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
          <NButton quaternary block class="kv-add" :disabled="hasEmptyRow" @click="addRow">+ 添加</NButton>
        </template>
      </aside>
      <div class="kv-panel">
        <template v-if="selectedRow">
          <div class="kv-panel-header">
            <NInput
              v-model:value="selectedRow.key"
              class="kv-key-input"
              size="small"
              placeholder="例如 config.yaml"
              spellcheck="false"
            />
          </div>
          <div class="kv-panel-body">
            <template v-if="showDecoded">
              <ValueEditor
                :model-value="selectedRow.value"
                fill-height
                @update:model-value="selectedRow.value = $event"
              />
            </template>
            <div v-else class="kv-raw-panel">
              <pre class="kv-raw-content">{{ getDisplayValue(selectedRow) || "(空)" }}</pre>
            </div>
          </div>
        </template>
        <div v-else class="kv-empty">
          <p>选择左侧键或点击「添加」新建</p>
        </div>
      </div>
    </div>
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
.kv-parse-btn {
  padding: 0.4rem 0.75rem;
  border: 1px solid var(--kf-border);
  border-radius: 6px;
  background: var(--kf-surface-strong);
  font-size: 0.8125rem;
  color: var(--kf-text-secondary);
  cursor: pointer;
}
.kv-parse-btn:hover {
  background: var(--kf-bg-soft);
  border-color: var(--kf-border-strong);
}
.kv-parse-btn.active {
  background: var(--kf-primary-soft);
  border-color: var(--kf-primary);
  color: var(--kf-primary);
}
.btn-primary {
  padding: 0.4rem 1rem;
  border: none;
  border-radius: 6px;
  background: var(--kf-primary);
  color: #fff;
  font-size: 0.8125rem;
  font-weight: 500;
  cursor: pointer;
}
.btn-primary:hover:not(:disabled) {
  opacity: 0.92;
}
.btn-primary:disabled {
  opacity: 0.6;
  cursor: not-allowed;
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
.kv-raw-panel {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 1rem;
  background: var(--kf-bg-soft);
  border-radius: 8px;
  border: 1px solid var(--kf-border);
}
.kv-raw-content {
  margin: 0;
  font-family: ui-monospace, monospace;
  font-size: 0.8125rem;
  line-height: 1.6;
  color: var(--kf-text-primary);
  white-space: pre-wrap;
  word-break: break-all;
}
.kv-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--kf-text-muted);
  font-size: 0.875rem;
}
.format-confirm-overlay {
  position: absolute;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.4);
}
.format-confirm-panel {
  padding: 1.25rem;
  background: var(--kf-surface-strong);
  border-radius: 8px;
  box-shadow: var(--kf-shadow-md);
  min-width: 320px;
}
.format-confirm-title {
  margin: 0 0 0.75rem;
  font-size: 1rem;
  font-weight: 600;
  color: var(--kf-text-primary);
}
.format-confirm-desc {
  margin: 0 0 1.25rem;
  font-size: 0.875rem;
  color: var(--kf-text-secondary);
  line-height: 1.5;
}
.format-confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
}
.btn-secondary {
  padding: 0.4rem 1rem;
  border: 1px solid var(--kf-border);
  border-radius: 6px;
  background: var(--kf-surface-strong);
  font-size: 0.8125rem;
  color: var(--kf-text-primary);
  cursor: pointer;
}
.btn-secondary:hover {
  background: var(--kf-bg-soft);
  border-color: var(--kf-border-strong);
}
</style>
