<script setup lang="ts">
import { ref, watch } from "vue";
import { NCard } from "naive-ui";

const props = withDefaults(
  defineProps<{
    title: string;
    accent: "meta" | "spec" | "template";
    summary?: string;
    defaultExpanded?: boolean;
    collapsible?: boolean;
    /** 资源切换时递增，用于重置展开状态 */
    resetKey?: number;
  }>(),
  {
    defaultExpanded: true,
    collapsible: true,
  },
);

const expanded = ref(props.defaultExpanded);

watch(
  () => props.defaultExpanded,
  (value) => {
    expanded.value = value;
  },
);

watch(
  () => props.resetKey,
  () => {
    expanded.value = props.defaultExpanded;
  },
);

function toggle() {
  if (!props.collapsible) return;
  expanded.value = !expanded.value;
}
</script>

<template>
  <NCard
    size="small"
    class="re-module-card"
    :class="[
      `re-module-card--${accent}`,
      { 're-module-card--collapsed': collapsible && !expanded },
    ]"
  >
    <template #header>
      <component
        :is="collapsible ? 'button' : 'div'"
        :type="collapsible ? 'button' : undefined"
        class="re-module-head"
        :class="{ 're-module-head--toggle': collapsible }"
        @click="toggle"
      >
        <span class="re-module-accent" :class="`re-module-accent--${accent}`" />
        <div class="re-module-head__main">
          <div class="re-module-title">{{ title }}</div>
          <div v-if="collapsible && !expanded && summary" class="re-module-summary">{{ summary }}</div>
        </div>
        <span
          v-if="collapsible"
          class="re-module-chevron"
          :class="{ 're-module-chevron--open': expanded }"
          aria-hidden="true"
        >
          ›
        </span>
      </component>
    </template>
    <div v-if="!collapsible || expanded" class="re-module-body">
      <slot />
    </div>
  </NCard>
</template>

<style src="./resourceEditUi.css"></style>
