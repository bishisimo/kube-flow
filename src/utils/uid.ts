/** 生成带前缀的唯一 ID。格式：{prefix}-{timestamp}-{random6}。 */
export function uid(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}
