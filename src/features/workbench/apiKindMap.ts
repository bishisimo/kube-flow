import type { ResourceKind } from "../../constants/resourceKinds";

/** 由扁平资源定义生成 API Kind（如 Service）→ ResourceKind id（如 services）的映射。 */
export function buildApiKindToIdMap(
  kinds: readonly { id: ResourceKind; label: string }[]
): Record<string, ResourceKind> {
  return Object.fromEntries(kinds.map((k) => [k.label, k.id])) as Record<string, ResourceKind>;
}

/** 有效的 ResourceKind id 集合，用于校验从存储恢复的值。 */
export function buildValidResourceKindSet(kinds: readonly { id: ResourceKind }[]): Set<string> {
  return new Set(kinds.map((k) => k.id));
}
