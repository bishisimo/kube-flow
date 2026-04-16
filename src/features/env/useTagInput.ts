import { ref, type Ref } from "vue";

export function useTagInput(tags: Ref<string[]>) {
  const draft = ref("");

  function addFromDraft() {
    const t = draft.value.trim();
    if (t && !tags.value.includes(t)) {
      tags.value = [...tags.value, t];
    }
    draft.value = "";
  }

  function removeTag(tag: string) {
    tags.value = tags.value.filter((t) => t !== tag);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === "," || e.key === "，") {
      e.preventDefault();
      addFromDraft();
    }
  }

  return { draft, addFromDraft, removeTag, onKeydown };
}
