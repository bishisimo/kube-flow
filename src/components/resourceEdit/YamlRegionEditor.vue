<script setup lang="ts">
import { computed, onMounted } from "vue";
import { CodeEditor } from "monaco-editor-vue3";
import { useYamlMonacoTheme } from "../../stores/yamlTheme";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    minHeight?: string;
    height?: string;
    readonly?: boolean;
  }>(),
  {
    minHeight: "220px",
    readonly: false,
  },
);

const emit = defineEmits<{
  (e: "update:modelValue", value: string): void;
}>();

const { monacoTheme } = useYamlMonacoTheme();

const editorHeight = computed(() => props.height ?? props.minHeight);

const monacoOptions = computed(() => ({
  fontSize: 13,
  minimap: { enabled: false },
  automaticLayout: true,
  wordWrap: "on" as const,
  lineNumbers: "on" as const,
  scrollBeyondLastLine: false,
  readOnly: props.readonly,
  tabSize: 2,
}));

onMounted(() => {
  window.dispatchEvent(new Event("resize"));
});
</script>

<template>
  <div
    class="yaml-region-editor"
    :style="{ height: editorHeight, minHeight: editorHeight }"
  >
    <CodeEditor
      :value="modelValue"
      language="yaml"
      :theme="monacoTheme"
      :options="monacoOptions"
      class="yaml-region-monaco"
      @update:value="emit('update:modelValue', $event)"
    />
  </div>
</template>

<style scoped>
.yaml-region-editor {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--kf-border, #e2e8f0);
  border-radius: 8px;
  overflow: hidden;
  background: var(--kf-surface-strong, #fff);
  min-width: 0;
}

.yaml-region-monaco {
  flex: 1;
  width: 100%;
  height: 100%;
  min-height: 0;
}

.yaml-region-monaco :deep(.monaco-editor),
.yaml-region-monaco :deep(.monaco-editor .overflow-guard) {
  height: 100% !important;
}

.yaml-region-monaco :deep(.monaco-editor) {
  outline: none;
}
</style>
