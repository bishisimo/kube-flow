<script setup lang="ts">
import { ref, watch, computed } from "vue";
import * as jsYaml from "js-yaml";
import ValueEditor from "./ValueEditor.vue";

export interface KeyValueRow {
  key: string;
  value: string;
}

const props = defineProps<{
  rawYaml: string;
  saving: boolean;
}>();

const emit = defineEmits<{
  (e: "save", yaml: string): void;
  (e: "error", message: string): void;
}>();

const rows = ref<KeyValueRow[]>([]);
const metadata = ref<Record<string, unknown>>({});
const binaryDataRows = ref<KeyValueRow[]>([]);
const selectedIndex = ref<number | null>(null);
const formatConfirmKeys = ref<string[]>([]);
const sidebarCollapsed = ref(false);

function parseYaml(): boolean {
  if (!props.rawYaml.trim()) {
    rows.value = [];
    binaryDataRows.value = [];
    metadata.value = {};
    selectedIndex.value = null;
    return true;
  }
  try {
    const obj = jsYaml.load(props.rawYaml) as Record<string, unknown>;
    if (!obj || typeof obj !== "object") return false;
    metadata.value = { metadata: obj.metadata };
    const data = (obj.data as Record<string, string>) || {};
    rows.value = Object.entries(data).map(([k, v]) => ({
      key: k,
      value: typeof v === "string" ? v : String(v),
    }));
    const binaryData = (obj.binaryData as Record<string, string>) || {};
    binaryDataRows.value = Object.entries(binaryData).map(([k, v]) => ({
      key: k,
      value: typeof v === "string" ? v : String(v),
    }));
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
  rows.value.push({ key: "", value: "" });
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

function getFormatHint(key: string): string {
  const ext = key.split(".").pop()?.toLowerCase() ?? "";
  if (["yaml", "yml"].includes(ext)) return "yaml";
  if (["json", "json5"].includes(ext)) return "json";
  return "";
}

/** 校验 value 是否符合 key 后缀所暗示的格式；返回不符合的 key 列表。空值视为通过。 */
function validateFormatBySuffix(rows: KeyValueRow[]): string[] {
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

function buildYaml(): string {
  const data: Record<string, string> = {};
  for (const r of rows.value) {
    const k = r.key.trim();
    if (k) data[k] = r.value;
  }
  const binaryData: Record<string, string> = {};
  for (const r of binaryDataRows.value) {
    const k = r.key.trim();
    if (k) binaryData[k] = r.value;
  }
  const meta = (metadata.value.metadata as Record<string, unknown>) || {};
  const obj: Record<string, unknown> = {
    apiVersion: "v1",
    kind: "ConfigMap",
    metadata: meta,
    data,
  };
  if (Object.keys(binaryData).length > 0) {
    obj.binaryData = binaryData;
  }
  return jsYaml.dump(obj, { lineWidth: -1 });
}

function doApply() {
  try {
    const yaml = buildYaml();
    emit("save", yaml);
  } catch (e) {
    emit("error", e instanceof Error ? e.message : String(e));
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
      <span class="kv-toolbar-title">Data · 共 {{ rows.length }} 项</span>
      <button
        type="button"
        class="btn-primary"
        :disabled="saving || hasEmptyRow"
        @click="onSave"
      >
        {{ saving ? "保存中…" : "应用" }}
      </button>
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
              <button
                type="button"
                class="kv-item-remove"
                aria-label="删除"
                @click.stop="removeRow(i)"
              >
                ×
              </button>
            </div>
          </div>
          <button type="button" class="kv-add" :disabled="hasEmptyRow" @click="addRow">
            + 添加
          </button>
        </template>
      </aside>
      <div class="kv-panel">
        <template v-if="selectedRow">
          <div class="kv-panel-header">
            <input
              v-model="selectedRow.key"
              type="text"
              class="kv-key-input"
              placeholder="例如 config.yaml"
              spellcheck="false"
            />
          </div>
          <div class="kv-panel-body">
            <ValueEditor
              :model-value="selectedRow.value"
              fill-height
              @update:model-value="selectedRow.value = $event"
            />
          </div>
        </template>
        <div v-else class="kv-empty">
          <p>选择左侧键或点击「添加」新建</p>
        </div>
      </div>
    </div>
    <div v-if="formatConfirmKeys.length" class="format-confirm-overlay" @click.self="onFormatConfirmCancel">
      <div class="format-confirm-panel" role="dialog" aria-modal="true" aria-labelledby="format-confirm-title" @click.stop>
        <h2 id="format-confirm-title" class="format-confirm-title">格式校验</h2>
        <p class="format-confirm-desc">以下配置项内容与后缀格式不符：{{ formatConfirmKeys.join("、") }}。是否仍要应用？</p>
        <div class="format-confirm-actions">
          <button type="button" class="btn-secondary" @click="onFormatConfirmCancel">取消</button>
          <button type="button" class="btn-primary" @click="onFormatConfirmApply">仍要应用</button>
        </div>
      </div>
    </div>
    <div v-if="binaryDataRows.length" class="kv-binary-section">
      <div class="kv-binary-title">BinaryData（只读）· {{ binaryDataRows.length }} 项</div>
      <div class="kv-binary-list">
        <span v-for="(r, i) in binaryDataRows" :key="i" class="kv-binary-item">{{ r.key }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.kv-editor {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: #fafbfc;
  position: relative;
}
.kv-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  background: #fff;
  border-bottom: 1px solid #e5e7eb;
  flex-shrink: 0;
}
.kv-toolbar-title {
  font-size: 0.875rem;
  font-weight: 600;
  color: #1f2937;
}
.btn-primary {
  padding: 0.4rem 1rem;
  border: none;
  border-radius: 6px;
  background: #2563eb;
  color: #fff;
  font-size: 0.8125rem;
  font-weight: 500;
  cursor: pointer;
}
.btn-primary:hover:not(:disabled) {
  background: #1d4ed8;
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
  background: var(--sidebar-bg, #fafafa);
  border-right: 1px solid var(--border-color, #e5e7eb);
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
  border-bottom: 1px solid var(--border-color, #e5e7eb);
  flex-shrink: 0;
  background: var(--sidebar-bg, #fafafa);
}
.kv-sidebar-header:hover {
  background: rgba(0, 0, 0, 0.06);
}
.kv-sidebar-icon {
  font-size: 1rem;
  color: #555;
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
  color: #374151;
  text-align: left;
  cursor: pointer;
  transition: background 0.15s;
}
.kv-item:hover {
  background: #f3f4f6;
}
.kv-item.active {
  background: #eff6ff;
  color: #1d4ed8;
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
  background: #e5e7eb;
  color: #6b7280;
}
.kv-item.active .kv-item-badge {
  background: #bfdbfe;
  color: #1d4ed8;
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
  color: #9ca3af;
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.15s;
}
.kv-item:hover .kv-item-remove {
  opacity: 1;
}
.kv-item-remove:hover {
  background: #fee2e2;
  color: #dc2626;
}
.kv-add {
  margin: 0.5rem;
  padding: 0.5rem;
  border: 1px dashed #d1d5db;
  border-radius: 6px;
  background: #fff;
  font-size: 0.8125rem;
  color: #6b7280;
  cursor: pointer;
  transition: border-color 0.15s, color 0.15s;
}
.kv-add:hover:not(:disabled) {
  border-color: #94a3b8;
  color: #475569;
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
  background: #fff;
  overflow: hidden;
}
.kv-panel-header {
  padding: 0.75rem 1rem;
  border-bottom: 1px solid #e5e7eb;
  flex-shrink: 0;
}
.kv-key-input {
  width: 100%;
  max-width: 320px;
  padding: 0.5rem 0.75rem;
  border: 1px solid #e5e7eb;
  border-radius: 6px;
  font-size: 0.875rem;
  font-family: ui-monospace, monospace;
  color: #1f2937;
}
.kv-key-input:focus {
  outline: none;
  border-color: #2563eb;
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.15);
}
.kv-key-input::placeholder {
  color: #9ca3af;
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
  align-items: center;
  justify-content: center;
  color: #9ca3af;
  font-size: 0.875rem;
}
.kv-binary-section {
  padding: 0.75rem 1rem;
  background: #f9fafb;
  border-top: 1px solid #e5e7eb;
  flex-shrink: 0;
}
.kv-binary-title {
  font-size: 0.75rem;
  font-weight: 600;
  color: #6b7280;
  margin-bottom: 0.5rem;
}
.kv-binary-list {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}
.kv-binary-item {
  padding: 0.25rem 0.5rem;
  font-size: 0.75rem;
  color: #9ca3af;
  background: #f3f4f6;
  border-radius: 4px;
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
  background: #fff;
  border-radius: 8px;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
  min-width: 320px;
}
.format-confirm-title {
  margin: 0 0 0.75rem;
  font-size: 1rem;
  font-weight: 600;
  color: #1f2937;
}
.format-confirm-desc {
  margin: 0 0 1.25rem;
  font-size: 0.875rem;
  color: #4b5563;
  line-height: 1.5;
}
.format-confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
}
.btn-secondary {
  padding: 0.4rem 1rem;
  border: 1px solid #d1d5db;
  border-radius: 6px;
  background: #fff;
  font-size: 0.8125rem;
  color: #374151;
  cursor: pointer;
}
.btn-secondary:hover {
  background: #f9fafb;
  border-color: #9ca3af;
}
</style>
