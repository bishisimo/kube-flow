/**
 * 终端二进制载荷编解码与尺寸估算。
 * 后端通过 base64 传输 chunk/stdin，避免 number[] JSON 膨胀。
 */

export function bytesToBase64(bytes: Uint8Array): string {
  const chunkSize = 0x8000;
  let binary = "";
  for (let i = 0; i < bytes.length; i += chunkSize) {
    const slice = bytes.subarray(i, i + chunkSize);
    binary += String.fromCharCode(...slice);
  }
  return btoa(binary);
}

export function base64ToBytes(b64: string): Uint8Array {
  const binary = atob(b64);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    out[i] = binary.charCodeAt(i);
  }
  return out;
}

/** 与 PodShellTerminal 默认字号对齐的近似单元格尺寸（启动 PTY 前估算用）。 */
const APPROX_CELL_WIDTH = 8.4;
const APPROX_CELL_HEIGHT = 17;

/**
 * 根据终端容器 DOM 估算 cols/rows。
 * 在 xterm 尚未挂载时用于 host/pod shell 启动参数。
 */
export function estimateTerminalSize(el?: HTMLElement | null): { cols: number; rows: number } {
  const width = el?.clientWidth ?? 0;
  const height = el?.clientHeight ?? 0;
  const cols = Math.max(2, Math.floor((width > 0 ? width : 960) / APPROX_CELL_WIDTH));
  const rows = Math.max(1, Math.floor((height > 0 ? height : 480) / APPROX_CELL_HEIGHT));
  return { cols, rows };
}
