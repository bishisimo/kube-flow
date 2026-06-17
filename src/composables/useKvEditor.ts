import { ref, watch, computed, type Ref } from "vue";
import { useAppSettingsStore } from "../stores/appSettings";
import type { KeyValueRow } from "../utils/kvValidation";
import { hasCharIssue, validateFormatBySuffix, validateControlChars } from "../utils/kvValidation";

export type { KeyValueRow };

export interface KvEditorCallbacks {
  rawYaml: () => string;
  parseYaml: (
    raw: string,
    rows: Ref<KeyValueRow[]>,
    metadata: Ref<Record<string, unknown>>,
    selectedIndex: Ref<number | null>,
  ) => Record<string, unknown> | null;
  buildYaml: (rows: KeyValueRow[], metadata: Record<string, unknown>) => string;
  emit: (...args: any[]) => void;
}

export interface KvEditorState {
  rows: Ref<KeyValueRow[]>;
  metadata: Ref<Record<string, unknown>>;
  selectedIndex: Ref<number | null>;
  formatConfirmKeys: Ref<string[]>;
  controlCharConfirmKeys: Ref<string[]>;
  sidebarCollapsed: Ref<boolean>;
  localWhitespaceOverride: Ref<boolean | null>;
  effectiveWhitespace: Ref<boolean>;
  whitespaceRenderEnabled: Ref<boolean>;
  hasEmptyRow: Ref<boolean>;
  selectedRow: Ref<KeyValueRow | null>;
  addRow: (factory: () => KeyValueRow) => void;
  removeRow: (index: number) => void;
  onSave: () => void;
  validateFormat: () => string[];
  onFormatConfirmApply: () => void;
  onFormatConfirmCancel: () => void;
  onControlCharConfirmApply: () => void;
  onControlCharConfirmCancel: () => void;
  hasCharIssue: (value: string) => boolean;
}

export function useKvEditor(cb: KvEditorCallbacks): KvEditorState {
  const rows = ref<KeyValueRow[]>([]);
  const metadata = ref<Record<string, unknown>>({});
  const selectedIndex = ref<number | null>(null);

  const formatConfirmKeys = ref<string[]>([]);
  const controlCharConfirmKeys = ref<string[]>([]);
  const sidebarCollapsed = ref(false);

  const { whitespaceRenderEnabled } = useAppSettingsStore();
  const localWhitespaceOverride = ref<boolean | null>(null);
  const effectiveWhitespace = computed(
    () => localWhitespaceOverride.value ?? whitespaceRenderEnabled.value,
  );

  const hasEmptyRow = computed(() => rows.value.some((r) => !r.key.trim()));

  const selectedRow = computed(() =>
    selectedIndex.value !== null &&
    selectedIndex.value >= 0 &&
    selectedIndex.value < rows.value.length
      ? rows.value[selectedIndex.value]
      : null,
  );

  // --- watchers ---

  watch(
    cb.rawYaml,
    (raw) => {
      cb.parseYaml(raw, rows, metadata, selectedIndex);
    },
    { immediate: true },
  );

  watch(
    [rows, metadata],
    () => {
      try {
        cb.emit("update:yaml", cb.buildYaml(rows.value, metadata.value));
      } catch {}
    },
    { deep: true },
  );

  // --- row operations ---

  function addRow(factory: () => KeyValueRow) {
    const idx = rows.value.length;
    rows.value.push(factory());
    selectedIndex.value = idx;
  }

  function removeRow(index: number) {
    rows.value.splice(index, 1);
    if (selectedIndex.value === index) {
      selectedIndex.value =
        rows.value.length > 0 ? Math.min(index, rows.value.length - 1) : null;
    } else if (selectedIndex.value !== null && selectedIndex.value > index) {
      selectedIndex.value--;
    }
  }

  // --- save pipeline ---

  function doApply() {
    try {
      cb.emit("save", cb.buildYaml(rows.value, metadata.value));
    } catch (e) {
      cb.emit("error", e instanceof Error ? e.message : String(e));
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
      cb.emit("error", `重复的 Key: ${duplicates.join(", ")}`);
      return;
    }
    const formatMismatch = validateFormatBySuffix(rows.value);
    if (formatMismatch.length > 0) {
      formatConfirmKeys.value = formatMismatch;
      return;
    }
    if (effectiveWhitespace.value) {
      const charIssues = validateControlChars(rows.value, effectiveWhitespace.value);
      if (charIssues.length > 0) {
        controlCharConfirmKeys.value = charIssues;
        return;
      }
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

  /** 独立校验各配置项的格式是否符合 key 后缀要求。更新 formatConfirmKeys 并返回不合法 key 列表。 */
  function validateFormat(): string[] {
    const formatMismatch = validateFormatBySuffix(rows.value);
    formatConfirmKeys.value = formatMismatch;
    return formatMismatch;
  }

  function onControlCharConfirmApply() {
    controlCharConfirmKeys.value = [];
    doApply();
  }

  function onControlCharConfirmCancel() {
    controlCharConfirmKeys.value = [];
  }

  return {
    rows,
    metadata,
    selectedIndex,
    formatConfirmKeys,
    controlCharConfirmKeys,
    sidebarCollapsed,
    localWhitespaceOverride,
    effectiveWhitespace,
    whitespaceRenderEnabled,
    hasEmptyRow,
    selectedRow,
    addRow,
    removeRow,
    onSave,
    validateFormat,
    onFormatConfirmApply,
    onFormatConfirmCancel,
    onControlCharConfirmApply,
    onControlCharConfirmCancel,
    hasCharIssue: (value: string) => hasCharIssue(value, effectiveWhitespace.value),
  };
}
