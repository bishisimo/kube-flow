import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export const FILE_TRANSFER_PROGRESS_EVENT = "file-transfer-progress";
export const FILE_TRANSFER_END_EVENT = "file-transfer-end";

export interface FileTransferProgress {
  transferId: string;
  transferredBytes: number;
  totalBytes: number | null;
}

export interface FileTransferEnd {
  transferId: string;
  error?: string | null;
}

export function fileTransferCancel(transferId: string): Promise<void> {
  return invoke("file_transfer_cancel", { transferId });
}

/**
 * 先挂上进度/结束监听，再启动传输，避免极快完成时漏掉事件。
 * 后端在 invoke 返回前就可能发出进度，因此对早到的事件做按 transferId 缓冲。
 */
export async function runFileTransfer(
  start: () => Promise<string>,
  options?: {
    onProgress?: (progress: FileTransferProgress) => void;
    onStarted?: (transferId: string) => void;
  }
): Promise<string> {
  let transferId: string | null = null;
  let settled = false;
  let resolveDone!: () => void;
  let rejectDone!: (error: Error) => void;
  const done = new Promise<void>((resolve, reject) => {
    resolveDone = resolve;
    rejectDone = reject;
  });
  const earlyEnds = new Map<string, FileTransferEnd>();
  const earlyProgress = new Map<string, FileTransferProgress>();
  let unProgress: UnlistenFn | undefined;
  let unEnd: UnlistenFn | undefined;

  const settle = (payload: FileTransferEnd) => {
    if (settled) return;
    settled = true;
    const error = payload.error?.trim();
    if (error) rejectDone(new Error(error));
    else resolveDone();
  };

  try {
    unProgress = await listen<FileTransferProgress>(FILE_TRANSFER_PROGRESS_EVENT, (event) => {
      if (transferId) {
        if (event.payload.transferId === transferId) {
          options?.onProgress?.(event.payload);
        }
        return;
      }
      earlyProgress.set(event.payload.transferId, event.payload);
    });
    unEnd = await listen<FileTransferEnd>(FILE_TRANSFER_END_EVENT, (event) => {
      if (transferId) {
        if (event.payload.transferId === transferId) settle(event.payload);
        return;
      }
      earlyEnds.set(event.payload.transferId, event.payload);
    });

    transferId = await start();
    options?.onStarted?.(transferId);
    const earlyP = earlyProgress.get(transferId);
    if (earlyP) options?.onProgress?.(earlyP);
    const early = earlyEnds.get(transferId);
    if (early) settle(early);
    await done;
    return transferId;
  } finally {
    unProgress?.();
    unEnd?.();
  }
}

export function formatTransferBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

/** 将字节/秒格式化为可读网速，如 `12.3 MB/s`。 */
export function formatTransferSpeed(bytesPerSec: number): string {
  if (!Number.isFinite(bytesPerSec) || bytesPerSec <= 0) return "—";
  return `${formatTransferBytes(bytesPerSec)}/s`;
}

export function transferProgressPercent(
  transferredBytes: number,
  totalBytes: number | null | undefined
): number | null {
  if (totalBytes == null || totalBytes <= 0) return null;
  if (transferredBytes <= 0) return null;
  return Math.min(100, Math.round((transferredBytes / totalBytes) * 100));
}

/**
 * 根据进度采样估算实时网速（指数滑动平均）。
 * 不依赖第三方库；要求传输路径持续上报 transferredBytes。
 */
export function createTransferSpeedTracker(options?: {
  /** EMA 平滑系数，越大越跟瞬时速度（默认 0.35） */
  alpha?: number;
  /** 两次采样最小间隔（秒），过密则忽略（默认 0.08） */
  minIntervalSec?: number;
}) {
  const alpha = options?.alpha ?? 0.35;
  const minIntervalSec = options?.minIntervalSec ?? 0.08;
  let lastBytes = 0;
  let lastAtMs = 0;
  let ema = 0;
  let hasSample = false;

  return {
    reset() {
      lastBytes = 0;
      lastAtMs = 0;
      ema = 0;
      hasSample = false;
    },
    /** 喂入最新进度，返回当前估算网速（bytes/sec）；样本不足时返回 0。 */
    push(transferredBytes: number, nowMs = performance.now()): number {
      if (!hasSample) {
        lastBytes = transferredBytes;
        lastAtMs = nowMs;
        hasSample = true;
        return 0;
      }
      const dt = (nowMs - lastAtMs) / 1000;
      if (dt < minIntervalSec) return ema;
      const delta = transferredBytes - lastBytes;
      lastBytes = transferredBytes;
      lastAtMs = nowMs;
      if (delta < 0) {
        ema = 0;
        return 0;
      }
      const instant = delta / dt;
      ema = ema <= 0 ? instant : ema * (1 - alpha) + instant * alpha;
      return ema;
    },
    current(): number {
      return ema;
    },
  };
}
