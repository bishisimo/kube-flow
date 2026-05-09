<script setup lang="ts">
/**
 * 轻量虚拟滚动列表 —— 针对固定行高的日志场景优化。
 *
 * 只渲染可视区域 ± buffer 条目，通过 translateY 定位。
 * 暴露 scrollToIndex / scrollToBottom / isAtBottom 供外部控制。
 */
import { ref, computed, onMounted, onBeforeUnmount, watch, nextTick, type PropType } from "vue";

export interface VirtualLogItem {
  index: number;
}

const props = defineProps({
  items: { type: Array as PropType<VirtualLogItem[]>, required: true },
  /** 单行渲染函数，返回 HTML 字符串 */
  renderLine: { type: Function as PropType<(item: VirtualLogItem) => string>, required: true },
  /** 行高 (px)，需与 CSS line-height 一致 */
  itemHeight: { type: Number, default: 20 },
  /** 可视区域外上下各预渲染的行数 */
  buffer: { type: Number, default: 20 },
  /** 传入 class 给内部滚动容器 */
  contentClass: { type: String, default: "" },
  /** 传入 style 给内部滚动容器 */
  contentStyle: { type: Object as PropType<Record<string, string>>, default: () => ({}) },
});

const emit = defineEmits<{
  (e: "scroll"): void;
}>();

const containerRef = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const containerHeight = ref(600);

const totalHeight = computed(() => props.items.length * props.itemHeight);

const visibleRange = computed(() => {
  const start = Math.max(0, Math.floor(scrollTop.value / props.itemHeight) - props.buffer);
  const visibleCount = Math.ceil(containerHeight.value / props.itemHeight);
  const end = Math.min(props.items.length, start + visibleCount + props.buffer * 2);
  return { start, end };
});

const visibleItems = computed(() =>
  props.items.slice(visibleRange.value.start, visibleRange.value.end)
);

const offsetY = computed(() => visibleRange.value.start * props.itemHeight);

const isAtBottom = ref(true);
const AT_BOTTOM_THRESHOLD = 50;

function checkAtBottom() {
  const el = containerRef.value;
  if (!el) return;
  isAtBottom.value = el.scrollHeight - el.scrollTop - el.clientHeight < AT_BOTTOM_THRESHOLD;
}

function onScroll() {
  const el = containerRef.value;
  if (!el) return;
  scrollTop.value = el.scrollTop;
  containerHeight.value = el.clientHeight;
  checkAtBottom();
  emit("scroll");
}

/** 滚动到指定行索引 */
function scrollToIndex(index: number) {
  const el = containerRef.value;
  if (!el) return;
  el.scrollTop = index * props.itemHeight;
}

/** 滚动到底部 */
function scrollToBottom() {
  const el = containerRef.value;
  if (!el) return;
  el.scrollTop = el.scrollHeight;
}

/** 容器尺寸变化时重新计算 */
function onResize() {
  const el = containerRef.value;
  if (el) containerHeight.value = el.clientHeight;
}

let resizeObserver: ResizeObserver | null = null;

onMounted(() => {
  const el = containerRef.value;
  if (el) {
    containerHeight.value = el.clientHeight;
    resizeObserver = new ResizeObserver(() => onResize());
    resizeObserver.observe(el);
  }
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
});

// 当 items 数量变化且处于底部时，自动滚到底
watch(
  () => props.items.length,
  async () => {
    if (isAtBottom.value) {
      await nextTick();
      scrollToBottom();
    }
  }
);

defineExpose({ scrollToIndex, scrollToBottom, isAtBottom, containerRef });
</script>

<template>
  <div
    ref="containerRef"
    class="virtual-log-container"
    :class="contentClass"
    :style="contentStyle"
    @scroll="onScroll"
  >
    <div
      class="virtual-log-spacer"
      :style="{ paddingTop: offsetY + 'px', paddingBottom: (totalHeight - offsetY - visibleItems.length * itemHeight) + 'px' }"
    >
      <div
        v-for="item in visibleItems"
        :key="item.index"
        class="virtual-log-line"
        :style="{ height: itemHeight + 'px', lineHeight: itemHeight + 'px' }"
        v-html="renderLine(item)"
      />
    </div>
  </div>
</template>

<style scoped>
.virtual-log-container {
  overflow: auto;
  position: relative;
}
.virtual-log-spacer {
  position: relative;
}
.virtual-log-line {
  white-space: pre;
  box-sizing: border-box;
  padding-right: 2rem;
}
</style>
