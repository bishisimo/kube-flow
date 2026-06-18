<script setup lang="ts">
import ContainerFormCard from "./ContainerFormCard.vue";
import ItemTabsEditor from "./ItemTabsEditor.vue";
import type { ContainerDraft } from "../../features/resourceEdit/workloadDraft";

defineProps<{
  containers: ContainerDraft[];
  init?: boolean;
  emptyHint?: string;
}>();

const emit = defineEmits<{
  (e: "update:containers", value: ContainerDraft[]): void;
}>();

function emptyContainer(): ContainerDraft {
  return {
    name: "",
    image: "",
    imagePullPolicy: "IfNotPresent",
    commandText: "",
    argsText: "",
    env: [],
    cpuRequest: "",
    memoryRequest: "",
    cpuLimit: "",
    memoryLimit: "",
    ports: [],
    volumeMounts: [],
  };
}

function tabLabel(container: ContainerDraft, index: number) {
  return container.name.trim() || `容器 ${index + 1}`;
}
</script>

<template>
  <ItemTabsEditor
    :items="containers"
    :create-item="emptyContainer"
    :get-label="tabLabel"
    :variant="init ? 'init' : 'container'"
    :empty-hint="emptyHint ?? '暂无容器'"
    add-label="添加容器"
    @update:items="emit('update:containers', $event)"
  >
    <template #default="{ item, index, update }">
      <ContainerFormCard
        embedded
        :container="item"
        :index="index"
        :init="init"
        @update:container="update"
      />
    </template>
  </ItemTabsEditor>
</template>
