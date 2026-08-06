<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { listen } from "@tauri-apps/api/event";
import { kubePodExecAck, kubePodExecStdin, kubePodExecResize } from "../api/kube";
import { hostShellAck, hostShellResize, hostShellStdin } from "../api/terminal";
import { base64ToBytes, bytesToBase64 } from "../utils/terminalBytes";
import "@xterm/xterm/css/xterm.css";

const props = withDefaults(
  defineProps<{
    streamId: string | null;
    mode?: "pod" | "host";
    active?: boolean;
    /** 递增时清空屏幕（例如切换 Pod/容器）；streamId 变化本身保留交互历史 */
    historyEpoch?: number;
  }>(),
  {
    mode: "pod",
    active: true,
    historyEpoch: 0,
  }
);

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
let pendingInput: Uint8Array[] = [];
let pendingInputBytes = 0;
const inputEncoder = new TextEncoder();
let lastResizeCols = 0;
let lastResizeRows = 0;
let listenerSetupSeq = 0;
let fitRaf = 0;
let resizeDebounceTimer: number | null = null;
let lastFitWidth = 0;
let lastFitHeight = 0;
let sawStream = false;
const INPUT_FLUSH_MS = 8;
const RESIZE_DEBOUNCE_MS = 48;

/** 输出写队列：watermark 背压，避免大刷屏卡住输入。 */
const WRITE_HIGH = 256 * 1024;
const ACK_BATCH = 64 * 1024;
let pendingChunks: Uint8Array[] = [];
let writeWatermark = 0;
let writeBusy = false;
let ackPending = 0;
let ackFlushTimer: number | null = null;

function clearInputBuffer() {
  if (inputFlushTimer !== null) {
    window.clearTimeout(inputFlushTimer);
    inputFlushTimer = null;
  }
  pendingInput = [];
  pendingInputBytes = 0;
}

function clearOutputQueue() {
  pendingChunks = [];
  writeWatermark = 0;
  writeBusy = false;
  ackPending = 0;
  if (ackFlushTimer !== null) {
    window.clearTimeout(ackFlushTimer);
    ackFlushTimer = null;
  }
}

function scheduleInputFlush() {
  if (inputFlushTimer !== null) return;
  inputFlushTimer = window.setTimeout(() => {
    inputFlushTimer = null;
    flushInputBuffer();
  }, INPUT_FLUSH_MS);
}

function containsControlByte(bytes: Uint8Array): boolean {
  for (let i = 0; i < bytes.length; i += 1) {
    const b = bytes[i];
    // ESC / 常见控制字符：立刻 flush，避免 vim ESC 超时与按键粘连
    if (b < 0x20 || b === 0x7f) return true;
  }
  return false;
}

function enqueueInputBytes(bytes: Uint8Array) {
  if (!bytes.length) return;
  pendingInput.push(bytes);
  pendingInputBytes += bytes.length;
  if (containsControlByte(bytes) || pendingInputBytes >= 256) {
    if (inputFlushTimer !== null) {
      window.clearTimeout(inputFlushTimer);
      inputFlushTimer = null;
    }
    flushInputBuffer();
    return;
  }
  scheduleInputFlush();
}

function takePendingInput(): Uint8Array {
  if (pendingInput.length === 1) {
    const only = pendingInput[0];
    pendingInput = [];
    pendingInputBytes = 0;
    return only;
  }
  const merged = new Uint8Array(pendingInputBytes);
  let offset = 0;
  for (const chunk of pendingInput) {
    merged.set(chunk, offset);
    offset += chunk.length;
  }
  pendingInput = [];
  pendingInputBytes = 0;
  return merged;
}

function flushInputBuffer() {
  if (!pendingInputBytes) return;
  const streamId = props.streamId;
  if (!streamId) {
    clearInputBuffer();
    return;
  }
  const batch = takePendingInput();
  const dataB64 = bytesToBase64(batch);
  const writeStdin = props.mode === "host" ? hostShellStdin : kubePodExecStdin;
  stdinWriteQueue = stdinWriteQueue
    .catch(() => {})
    .then(() => writeStdin(streamId, dataB64))
    .catch(() => {});
}

function flushAck() {
  if (ackFlushTimer !== null) {
    window.clearTimeout(ackFlushTimer);
    ackFlushTimer = null;
  }
  const streamId = props.streamId;
  const bytes = ackPending;
  ackPending = 0;
  if (!streamId || bytes <= 0) return;
  const sendAck = props.mode === "host" ? hostShellAck : kubePodExecAck;
  sendAck(streamId, bytes).catch(() => {});
}

function scheduleAck(bytes: number) {
  ackPending += bytes;
  if (ackPending >= ACK_BATCH) {
    flushAck();
    return;
  }
  if (ackFlushTimer !== null) return;
  ackFlushTimer = window.setTimeout(() => {
    ackFlushTimer = null;
    flushAck();
  }, 32);
}

function pumpWrite() {
  if (writeBusy || !terminal || !pendingChunks.length) return;
  if (writeWatermark > WRITE_HIGH) return;
  writeBusy = true;
  const chunk = pendingChunks.shift()!;
  writeWatermark += chunk.length;
  terminal.write(chunk, () => {
    writeWatermark = Math.max(0, writeWatermark - chunk.length);
    writeBusy = false;
    scheduleAck(chunk.length);
    pumpWrite();
  });
}

function enqueueOutputBytes(bytes: Uint8Array) {
  if (!bytes.length) return;
  pendingChunks.push(bytes);
  pumpWrite();
}

function sanitizeTerminalInput(text: string): string {
  return text
    .replace(/\u00A0/g, " ")
    .replace(/[\u200B-\u200D\u2060]/g, "")
    .replace(/[\u202A-\u202E\u2066-\u2069]/g, "")
    .replace(/\uFEFF/g, "");
}

function resetTerminalView() {
  clearOutputQueue();
  terminal?.reset();
  sawStream = false;
}

/** 导出当前缓冲区内的纯文本交互历史，供保存或外部编辑器打开。 */
function getHistoryText(): string {
  if (!terminal) return "";
  const buffer = terminal.buffer.active;
  const lines: string[] = [];
  const total = buffer.length;
  for (let i = 0; i < total; i += 1) {
    const line = buffer.getLine(i);
    if (line) lines.push(line.translateToString(true));
  }
  while (lines.length && !lines[lines.length - 1].trim()) {
    lines.pop();
  }
  return lines.join("\n");
}

function writeReconnectMarker() {
  if (!terminal || !sawStream) return;
  terminal.writeln("\x1b[90m── 会话已重新连接，上方为断开前的交互历史 ──\x1b[0m");
}

async function tryLoadWebgl(term: Terminal) {
  try {
    const mod = await import("@xterm/addon-webgl");
    const addon = new mod.WebglAddon();
    term.loadAddon(addon);
  } catch {
    // WebGL 不可用时保持 canvas/dom 渲染
  }
}

function scheduleFitAndResize(force = false) {
  if (fitRaf) cancelAnimationFrame(fitRaf);
  fitRaf = requestAnimationFrame(() => {
    fitRaf = 0;
    fitAndResize(force);
  });
}

function fitAndResize(force = false) {
  if (!fitAddon || !terminal || !terminalRef.value) return;
  const width = terminalRef.value.clientWidth;
  const height = terminalRef.value.clientHeight;
  if (!force && width === lastFitWidth && height === lastFitHeight && lastResizeCols > 0) {
    return;
  }
  if (width <= 0 || height <= 0) return;
  lastFitWidth = width;
  lastFitHeight = height;
  fitAddon.fit();
  trySendResize();
}

function initTerminal() {
  if (!terminalRef.value) return;
  terminal = new Terminal({
    cursorBlink: true,
    fontSize: 14,
    fontFamily: "ui-monospace, monospace",
    scrollback: 5000,
    theme: {
      background: "#1e293b",
      foreground: "#e2e8f0",
    },
  });
  fitAddon = new FitAddon();
  terminal.loadAddon(fitAddon);
  terminal.open(terminalRef.value);
  void tryLoadWebgl(terminal);

  // 立即 fit 一次：open() 后 xterm 已同步测量字符尺寸，容器此时也已布局完成
  fitAndResize(true);

  terminal.onData((data) => {
    const sanitized = sanitizeTerminalInput(data);
    if (!sanitized) return;
    enqueueInputBytes(inputEncoder.encode(sanitized));
  });

  terminal.onBinary((data) => {
    const bytes = new Uint8Array(data.length);
    for (let i = 0; i < data.length; i += 1) {
      bytes[i] = data.charCodeAt(i) & 0xff;
    }
    enqueueInputBytes(bytes);
  });

  // ResizeObserver：尺寸稳定后再 fit，避免刚挂载时多次布局抖动
  resizeObserver = new ResizeObserver(() => {
    if (resizeDebounceTimer !== null) {
      window.clearTimeout(resizeDebounceTimer);
    }
    resizeDebounceTimer = window.setTimeout(() => {
      resizeDebounceTimer = null;
      scheduleFitAndResize();
    }, RESIZE_DEBOUNCE_MS);
  });
  resizeObserver.observe(terminalRef.value);
}

function trySendResize() {
  if (!props.streamId || !terminal) return;
  const cols = terminal.cols;
  const rows = terminal.rows;
  if (cols <= 0 || rows <= 0) return;
  if (cols === lastResizeCols && rows === lastResizeRows) return;
  lastResizeCols = cols;
  lastResizeRows = rows;
  const resizeTerminal = props.mode === "host" ? hostShellResize : kubePodExecResize;
  resizeTerminal(props.streamId, cols, rows).catch(() => {});
}

async function setupListeners() {
  const seq = ++listenerSetupSeq;
  unlistenChunk?.();
  unlistenEnd?.();
  unlistenChunk = null;
  unlistenEnd = null;
  if (!props.streamId) return;

  const chunkEvent = props.mode === "host" ? "host-shell-chunk" : "pod-exec-chunk";
  const endEvent = props.mode === "host" ? "host-shell-end" : "pod-exec-end";

  const nextUnlistenChunk = await listen<{
    stream_id: string;
    chunk_b64?: string;
    chunk_bytes?: number[];
  }>(chunkEvent, (ev) => {
    if (ev.payload?.stream_id !== props.streamId || !terminal) return;
    if (ev.payload.chunk_b64) {
      enqueueOutputBytes(base64ToBytes(ev.payload.chunk_b64));
      return;
    }
    // 兼容旧事件（热替换期间）
    if (ev.payload.chunk_bytes?.length) {
      enqueueOutputBytes(new Uint8Array(ev.payload.chunk_bytes));
    }
  });

  if (seq !== listenerSetupSeq) {
    nextUnlistenChunk();
    return;
  }

  const nextUnlistenEnd = await listen<{ stream_id: string; error?: string }>(
    endEvent,
    (ev) => {
      if (ev.payload?.stream_id === props.streamId) {
        flushAck();
        emit("end", {
          streamId: ev.payload.stream_id,
          error: ev.payload?.error,
        });
      }
    }
  );

  if (seq !== listenerSetupSeq) {
    nextUnlistenChunk();
    nextUnlistenEnd();
    return;
  }

  unlistenChunk = nextUnlistenChunk;
  unlistenEnd = nextUnlistenEnd;
}

watch(
  () => props.historyEpoch,
  () => {
    resetTerminalView();
  }
);

// streamId 变化只重绑监听与尺寸，不清空屏幕历史
watch(
  () => props.streamId,
  async (id, prev) => {
    clearInputBuffer();
    clearOutputQueue();
    lastResizeCols = 0;
    lastResizeRows = 0;
    if (id && prev && id !== prev) {
      writeReconnectMarker();
    } else if (id && !prev && sawStream) {
      writeReconnectMarker();
    }
    if (id) {
      sawStream = true;
      scheduleFitAndResize(true);
    }
    await setupListeners();
    if (id) {
      scheduleFitAndResize(true);
    }
  }
);

watch(
  () => props.active,
  (active) => {
    if (!active || !props.streamId) return;
    scheduleFitAndResize(true);
  }
);

onMounted(async () => {
  initTerminal();
  if (props.streamId) {
    sawStream = true;
    scheduleFitAndResize(true);
  }
  await setupListeners();
});

onUnmounted(() => {
  listenerSetupSeq += 1;
  flushAck();
  clearInputBuffer();
  clearOutputQueue();
  if (fitRaf) cancelAnimationFrame(fitRaf);
  if (resizeDebounceTimer !== null) {
    window.clearTimeout(resizeDebounceTimer);
    resizeDebounceTimer = null;
  }
  unlistenChunk?.();
  unlistenEnd?.();
  resizeObserver?.disconnect();
  terminal?.dispose();
  terminal = null;
  fitAddon = null;
  stdinWriteQueue = Promise.resolve();
});

defineExpose({
  getHistoryText,
  fitAndResize,
});
</script>

<template>
  <div ref="terminalRef" class="pod-shell-terminal" />
</template>

<style scoped>
.pod-shell-terminal {
  flex: 1;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  contain: layout paint;
}
.pod-shell-terminal :deep(.xterm) {
  height: 100%;
  overflow: hidden;
}
</style>
