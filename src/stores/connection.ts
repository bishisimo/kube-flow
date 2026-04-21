/**
 * 连接状态与进度：SSH 隧道连接步骤、断开检测、重连。
 */
import { ref } from "vue";
import { listen } from "@tauri-apps/api/event";

export type EnvConnectionState = "connected" | "disconnected" | "connecting" | "error";

export interface ConnectionStage {
  id: string;
  label: string;
  status: "pending" | "running" | "success" | "error";
  detail?: string;
  error?: string;
}

export interface ConnectionProgressPayload {
  env_id: string;
  source: string;
  stage_id: string;
  stage_label: string;
  status: string;
  detail?: string;
  error?: string;
  overall_status: string;
}

/** 按 envId 存储连接进度（最新一次事件的累积状态） */
const connectionProgress = ref<Record<string, ConnectionProgressPayload>>({});
/** 按 envId 存储连接状态 */
const envConnectionState = ref<Record<string, EnvConnectionState>>({});
/** 按 envId 存储断开/错误时的错误信息 */
const envConnectionError = ref<Record<string, string>>({});

/** 判断是否为连接类错误（可重连）。覆盖工作台连接错误和终端会话断连错误。 */
export function isConnectionError(msg: string): boolean {
  const s = msg.toLowerCase();
  return (
    s.includes("connection refused") ||
    s.includes("connection reset") ||
    s.includes("econnrefused") ||
    s.includes("econnreset") ||
    s.includes("timeout") ||
    s.includes("timed out") ||
    s.includes("tunnel") ||
    s.includes("ssh") ||
    s.includes("network") ||
    s.includes("client error") ||
    s.includes("serviceerror") ||
    s.includes("failed to perform") ||
    s.includes("broken pipe") ||
    s.includes("eof") ||
    s.includes("disconnected") ||
    s.includes("unreachable") ||
    s.includes("transport") ||
    s.includes("tcp") ||
    s.includes("session not found")
  );
}

export function useConnectionStore() {
  const getProgress = (envId: string) => connectionProgress.value[envId];
  const getState = (envId: string): EnvConnectionState =>
    envConnectionState.value[envId] ?? "connected";
  const getError = (envId: string) => envConnectionError.value[envId];

  function setDisconnected(envId: string, error: string) {
    envConnectionState.value = { ...envConnectionState.value, [envId]: "disconnected" };
    envConnectionError.value = { ...envConnectionError.value, [envId]: error };
  }

  function setConnecting(envId: string) {
    envConnectionState.value = { ...envConnectionState.value, [envId]: "connecting" };
    envConnectionError.value = { ...envConnectionError.value, [envId]: "" };
  }

  function setConnected(envId: string) {
    envConnectionState.value = { ...envConnectionState.value, [envId]: "connected" };
    envConnectionError.value = { ...envConnectionError.value, [envId]: "" };
    // 延迟清除进度，避免闪烁
    setTimeout(() => {
      const next = { ...connectionProgress.value };
      delete next[envId];
      connectionProgress.value = next;
    }, 1000);
  }

  function setError(envId: string, error: string) {
    envConnectionState.value = { ...envConnectionState.value, [envId]: "error" };
    envConnectionError.value = { ...envConnectionError.value, [envId]: error };
  }

  function clearProgress(envId: string) {
    const next = { ...connectionProgress.value };
    delete next[envId];
    connectionProgress.value = next;
  }

  function setupConnectionProgressListener() {
    return listen<ConnectionProgressPayload>("connection-progress", (ev) => {
      const p = ev.payload;
      if (!p?.env_id) return;
      connectionProgress.value = { ...connectionProgress.value, [p.env_id]: p };
      if (p.overall_status === "connected") {
        setConnected(p.env_id);
      } else if (p.overall_status === "error") {
        setError(p.env_id, p.error ?? "连接失败");
      } else if (p.overall_status === "connecting") {
        envConnectionState.value = { ...envConnectionState.value, [p.env_id]: "connecting" };
      }
    });
  }

  return {
    connectionProgress,
    envConnectionState,
    envConnectionError,
    getProgress,
    getState,
    getError,
    setDisconnected,
    setConnecting,
    setConnected,
    setError,
    clearProgress,
    setupConnectionProgressListener,
  };
}
