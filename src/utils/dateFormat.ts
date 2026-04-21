/** 格式化 ISO 时间字符串为本地时间（展示用）。 */
export function formatDateTime(iso: string | undefined | null): string {
  if (!iso) return "-";
  return new Date(iso).toLocaleString();
}
