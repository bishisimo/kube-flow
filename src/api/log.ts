/**
 * 调试日志相关 Tauri 命令封装。
 */
import { invoke } from "@tauri-apps/api/core";

export type LogLevel = "off" | "error" | "warn" | "info" | "debug";

export const LOG_LEVELS: { value: LogLevel; label: string }[] = [
  { value: "off", label: "关闭" },
  { value: "error", label: "Error" },
  { value: "warn", label: "Warn" },
  { value: "info", label: "Info" },
  { value: "debug", label: "Debug" },
];

export async function logGetLevel(): Promise<string> {
  return invoke<string>("log_get_level");
}

export async function logSetLevel(level: string): Promise<void> {
  return invoke("log_set_level", { level });
}

export async function logRead(): Promise<string> {
  return invoke<string>("log_read");
}

export async function logClear(): Promise<void> {
  return invoke("log_clear");
}

export type LogDisplayOrder = "asc" | "desc";
export type LogDisplayFormat = "json" | "text";

export const LOG_DISPLAY_ORDERS: { value: LogDisplayOrder; label: string }[] = [
  { value: "asc", label: "正序（旧→新）" },
  { value: "desc", label: "倒序（新→旧）" },
];

export const LOG_DISPLAY_FORMATS: { value: LogDisplayFormat; label: string }[] = [
  { value: "json", label: "JSON" },
  { value: "text", label: "文本" },
];

export async function logGetDisplaySettings(): Promise<{
  order: LogDisplayOrder;
  format: LogDisplayFormat;
}> {
  const [order, format] = await invoke<[string, string]>("log_get_display_settings");
  return {
    order: order as LogDisplayOrder,
    format: format as LogDisplayFormat,
  };
}

export async function logSetDisplaySettings(
  order: LogDisplayOrder,
  format: LogDisplayFormat
): Promise<void> {
  return invoke("log_set_display_settings", { order, format });
}
