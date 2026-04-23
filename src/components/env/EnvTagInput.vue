<script setup lang="ts">
/**
 * 标签输入控件：回车或逗号提交当前草稿，点击标签上的关闭图标移除。
 *
 * 双向绑定形式：v-model 对接 string[]。
 */
import { computed, ref } from "vue";
import { NInput, NTag } from "naive-ui";

const props = defineProps<{
  modelValue: string[];
  placeholder?: string;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", value: string[]): void;
}>();

const draft = ref("");

const tags = computed({
  get: () => props.modelValue ?? [],
  set: (value: string[]) => emit("update:modelValue", value),
});

function addFromDraft() {
  const text = draft.value.trim();
  if (!text) return;
  if (tags.value.includes(text)) {
    draft.value = "";
    return;
  }
  tags.value = [...tags.value, text];
  draft.value = "";
}

function removeTag(tag: string) {
  tags.value = tags.value.filter((t) => t !== tag);
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Enter" || event.key === "," || event.key === "，") {
    event.preventDefault();
    addFromDraft();
  }
}
</script>

<template>
  <div class="env-tag-input" :class="{ disabled }">
    <NTag
      v-for="t in tags"
      :key="t"
      closable
      round
      size="small"
      type="success"
      :disabled="disabled"
      class="env-tag-chip"
      @close="removeTag(t)"
    >
      {{ t }}
    </NTag>
    <NInput
      v-model:value="draft"
      size="small"
      class="env-tag-draft"
      :placeholder="placeholder ?? '输入后按 Enter 或逗号添加'"
      :disabled="disabled"
      :bordered="false"
      @keydown="onKeydown"
      @blur="addFromDraft"
    />
  </div>
</template>

<style scoped>
.env-tag-input {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.4rem;
  min-height: 34px;
  padding: 4px 8px;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  background: #fff;
  transition: border-color 0.15s, box-shadow 0.15s;
}
.env-tag-input:focus-within {
  border-color: #2563eb;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.12);
}
.env-tag-input.disabled {
  opacity: 0.6;
  pointer-events: none;
}
.env-tag-chip {
  flex: 0 0 auto;
}
.env-tag-draft {
  flex: 1;
  min-width: 9rem;
}
.env-tag-draft :deep(.n-input__input-el) {
  padding: 0;
}
</style>
