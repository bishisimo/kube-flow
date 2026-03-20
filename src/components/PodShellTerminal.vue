<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { listen } from "@tauri-apps/api/event";
import { kubePodExecStdin, kubePodExecResize } from "../api/kube";
import { hostShellResize, hostShellStdin } from "../api/terminal";
import "@xterm/xterm/css/xterm.css";

const props = defineProps<{
  streamId: string | null;
  mode?: "pod" | "host";
}>();

const emit = defineEmits<{
  (e: "end", payload: { streamId: string; error?: string }): void;
}>();

const terminalRef = ref<HTMLElement | null>(null);
let terminal: Terminal | null = null;
let fitAddon: FitAddon | null = null;
let unlistenChunk: (() => void) | null = null;
let unlistenEnd: (() => void) | null = null;
let resizeObserver: ResizeObserver | null = null;
let stdinWriteQueue: Promise<void> = Promise.resolve();
let inputFlushTimer: number | null = null;
let pendingInputBytes: number[] = [];
const inputEncoder = new TextEncoder();
let lastResizeCols = 0;
let lastResizeRows = 0;
const INPUT_FLUSH_MS = 8;

function clearInputBuffer() {
  if (inputFlushTimer !== null) {
    window.clearTimeout(inputFlushTimer);
    inputFlushTimer = null;
  }
  pendingInputBytes = [];
}

function scheduleInputFlush() {
  if (inputFlushTimer !== null) return;
  inputFlushTimer = window.setTimeout(() => {
    inputFlushTimer = null;
    flushInputBuffer();
  }, INPUT_FLUSH_MS);
}

function enqueueInputBytes(bytes: Uint8Array | number[]) {
  if (!bytes.length) return;
  if (bytes instanceof Uint8Array) {
    pendingInputBytes.push(...Array.from(bytes));
  } else {
    pendingInputBytes.push(...bytes);
  }
  scheduleInputFlush();
}

function flushInputBuffer() {
  if (!pendingInputBytes.length) return;
  const streamId = props.streamId;
  if (!streamId) {
    pendingInputBytes = [];
    return;
  }
  const batch = pendingInputBytes;
  pendingInputBytes = [];
  const writeStdin = props.mode === "host" ? hostShellStdin : kubePodExecStdin;
  stdinWriteQueue = stdinWriteQueue
    .catch(() => {})
    .then(() => writeStdin(streamId, batch))
    .catch(() => {});
}

function sanitizeTerminalInput(text: string): string {
  return text
    .replace(/\u00A0/g, " ")
    .replace(/[\u200B-\u200D\u2060]/g, "")
    .replace(/[\u202A-\u202E\u2066-\u2069]/g, "")
    .replace(/\uFEFF/g, "");
}

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
    const sanitized = sanitizeTerminalInput(data);
    if (!sanitized) return;
    enqueueInputBytes(inputEncoder.encode(sanitized));
  });

  terminal.onBinary((data) => {
    const bytes = Array.from(data).map((ch) => ch.charCodeAt(0));
    enqueueInputBytes(bytes);
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
  const resizeTerminal = props.mode === "host" ? hostShellResize : kubePodExecResize;
  resizeTerminal(props.streamId, dims.cols, dims.rows).catch(() => {});
}

async function setupListeners() {
  unlistenChunk?.();
  unlistenEnd?.();
  unlistenChunk = null;
  unlistenEnd = null;
  if (!props.streamId) return;

  const chunkEvent = props.mode === "host" ? "host-shell-chunk" : "pod-exec-chunk";
  const endEvent = props.mode === "host" ? "host-shell-end" : "pod-exec-end";

  unlistenChunk = await listen<{
    stream_id: string;
    chunk_bytes: number[];
  }>(
    chunkEvent,
    (ev) => {
      if (ev.payload?.stream_id !== props.streamId || !terminal) return;
      terminal.write(new Uint8Array(ev.payload.chunk_bytes));
    }
  );

  unlistenEnd = await listen<{ stream_id: string; error?: string }>(
    endEvent,
    (ev) => {
      if (ev.payload?.stream_id === props.streamId) {
        emit("end", {
          streamId: ev.payload.stream_id,
          error: ev.payload?.error,
        });
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
    clearInputBuffer();
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
  clearInputBuffer();
  unlistenChunk?.();
  unlistenEnd?.();
  resizeObserver?.disconnect();
  terminal?.dispose();
  terminal = null;
  fitAddon = null;
  stdinWriteQueue = Promise.resolve();
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
