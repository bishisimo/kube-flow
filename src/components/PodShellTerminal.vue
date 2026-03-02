<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { listen } from "@tauri-apps/api/event";
import { kubePodExecStdin, kubePodExecResize } from "../api/kube";
import "@xterm/xterm/css/xterm.css";

const props = defineProps<{
  streamId: string | null;
}>();

const emit = defineEmits<{
  (e: "end", error?: string): void;
}>();

const terminalRef = ref<HTMLElement | null>(null);
let terminal: Terminal | null = null;
let fitAddon: FitAddon | null = null;
let unlistenChunk: (() => void) | null = null;
let unlistenEnd: (() => void) | null = null;
let resizeObserver: ResizeObserver | null = null;
let lastResizeCols = 0;
let lastResizeRows = 0;

function initTerminal() {
  if (!terminalRef.value) return;
  terminal = new Terminal({
    cursorBlink: true,
    fontSize: 14,
    fontFamily: "ui-monospace, monospace",
    theme: {
      background: "#1e293b",
      foreground: "#e2e8f0",
    },
  });
  fitAddon = new FitAddon();
  terminal.loadAddon(fitAddon);
  terminal.open(terminalRef.value);

  // 立即 fit 一次：open() 后 xterm 已同步测量字符尺寸，容器此时也已布局完成
  fitAddon.fit();

  terminal.onData((data) => {
    if (props.streamId) {
      const bytes = Array.from(new TextEncoder().encode(data));
      kubePodExecStdin(props.streamId, bytes).catch(() => {});
    }
  });

  // ResizeObserver 处理容器尺寸变化（切换侧边栏、窗口缩放、tab 从隐藏变可见）
  resizeObserver = new ResizeObserver(() => {
    fitAddon?.fit();
    trySendResize();
  });
  resizeObserver.observe(terminalRef.value);
}

function trySendResize() {
  if (!props.streamId || !terminal) return;
  const dims = fitAddon?.proposeDimensions();
  if (!dims || dims.cols <= 0 || dims.rows <= 0) return;
  if (dims.cols === lastResizeCols && dims.rows === lastResizeRows) return;
  lastResizeCols = dims.cols;
  lastResizeRows = dims.rows;
  kubePodExecResize(props.streamId, dims.cols, dims.rows).catch(() => {});
}

async function setupListeners() {
  unlistenChunk?.();
  unlistenEnd?.();
  unlistenChunk = null;
  unlistenEnd = null;
  if (!props.streamId) return;

  unlistenChunk = await listen<{ stream_id: string; chunk: string }>(
    "pod-exec-chunk",
    (ev) => {
      if (ev.payload?.stream_id !== props.streamId || !terminal) return;
      terminal.write(ev.payload.chunk);
    }
  );

  unlistenEnd = await listen<{ stream_id: string; error?: string }>(
    "pod-exec-end",
    (ev) => {
      if (ev.payload?.stream_id === props.streamId) {
        emit("end", ev.payload?.error);
      }
    }
  );
}

function fitAndResize() {
  fitAddon?.fit();
  trySendResize();
}

// 处理 streamId 变化（切换 Pod/容器），不处理初始值（由 onMounted 负责）
watch(
  () => props.streamId,
  async (id) => {
    lastResizeCols = 0;
    lastResizeRows = 0;
    await setupListeners();
    if (id) {
      // RAF 确保新布局完成后再 fit
      requestAnimationFrame(fitAndResize);
    }
  }
);

onMounted(async () => {
  initTerminal();
  await setupListeners();
  if (props.streamId) {
    trySendResize();
  }
});

onUnmounted(() => {
  unlistenChunk?.();
  unlistenEnd?.();
  resizeObserver?.disconnect();
  terminal?.dispose();
  terminal = null;
  fitAddon = null;
});
</script>

<template>
  <div ref="terminalRef" class="pod-shell-terminal" />
</template>

<style scoped>
.pod-shell-terminal {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.pod-shell-terminal :deep(.xterm) {
  height: 100%;
  overflow: hidden;
}
</style>
