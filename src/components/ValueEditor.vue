<script setup lang="ts">
import { computed } from "vue";
import * as jsYaml from "js-yaml";
import { CodeEditor } from "monaco-editor-vue3";
import { useYamlMonacoTheme } from "../stores/yamlTheme";

/** 根据内容推断 value 的格式，用于语法高亮。JSON 优先，否则尝试 YAML 结构，默认 plaintext。 */
function detectValueLanguage(value: string): "json" | "yaml" | "plaintext" {
  const t = value.trim();
  if (!t) return "plaintext";
  if (t.startsWith("{") || t.startsWith("[")) {
    try {
      JSON.parse(t);
      return "json";
    } catch {
      // 可能是无效 JSON，继续尝试 YAML
    }
  }
  if (
    t.includes("\n") ||
    /^\s*[\w-]+:\s*.+$/m.test(t) ||
    /^\s*-\s+.+$/m.test(t)
  ) {
    try {
      const parsed = jsYaml.load(t);
      if (parsed !== null && typeof parsed === "object") return "yaml";
    } catch {
      // 非结构化 YAML，用 plaintext
    }
  }
  return "plaintext";
}

const props = defineProps<{
  modelValue: string;
  fillHeight?: boolean;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", v: string): void;
}>();

const { monacoTheme } = useYamlMonacoTheme();
const language = computed(() => detectValueLanguage(props.modelValue));

const monacoOptions = computed(() => ({
  fontSize: 12,
  minimap: { enabled: false },
  automaticLayout: true,
  wordWrap: "on",
  lineNumbers: props.fillHeight ? "on" : "off",
  scrollBeyondLastLine: false,
  padding: { top: 6, bottom: 6 },
  scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
}));
</script>

<template>
  <div class="value-editor" :class="{ 'fill-height': fillHeight ?? false }">
    <div class="value-monaco-wrap">
      <CodeEditor
        :value="modelValue"
        :language="language"
        :theme="monacoTheme"
        :options="monacoOptions"
        class="value-monaco"
        @update:value="emit('update:modelValue', $event)"
      />
    </div>
  </div>
</template>

<style scoped>
.value-editor {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  width: 100%;
}
.value-editor.fill-height {
  height: 100%;
  min-height: 0;
}
.value-monaco-wrap {
  min-height: 100px;
  max-height: 200px;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  overflow: hidden;
}
.value-editor.fill-height .value-monaco-wrap {
  min-height: 200px;
  max-height: none;
  height: 100%;
}
.value-monaco-wrap:focus-within {
  border-color: #2563eb;
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.15);
}
.value-monaco {
  height: 100%;
  min-height: 100px;
}
.value-monaco :deep(.monaco-editor) {
  padding: 0;
}
</style>
