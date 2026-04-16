import * as jsYaml from "js-yaml";

export interface KubeObjectIdentity {
  kind: string;
  name: string;
  namespace: string | null;
  apiVersion: string;
  raw: Record<string, unknown>;
}

/** 解析 Kubernetes YAML，返回带类型的基础字段；解析失败或缺少 kind/name 时返回 null。 */
export function parseKubeObject(yamlStr: string): KubeObjectIdentity | null {
  try {
    const parsed = jsYaml.load(yamlStr);
    if (!parsed || typeof parsed !== "object") return null;
    const obj = parsed as Record<string, unknown>;
    const kind = typeof obj.kind === "string" ? obj.kind.trim() : "";
    const apiVersion = typeof obj.apiVersion === "string" ? obj.apiVersion.trim() : "";
    const metadata =
      obj.metadata && typeof obj.metadata === "object"
        ? (obj.metadata as Record<string, unknown>)
        : null;
    const name = metadata && typeof metadata.name === "string" ? metadata.name.trim() : "";
    const namespace =
      metadata && typeof metadata.namespace === "string" && metadata.namespace.trim()
        ? metadata.namespace.trim()
        : null;
    if (!kind || !name) return null;
    return { kind, name, namespace, apiVersion, raw: obj };
  } catch {
    return null;
  }
}

/** 仅删除 managedFields（展示用）。 */
export function stripManagedFields(yamlStr: string): string {
  if (!yamlStr) return "";
  try {
    const obj = jsYaml.load(yamlStr) as Record<string, unknown>;
    if (!obj || typeof obj !== "object") return yamlStr;
    if (obj.metadata && typeof obj.metadata === "object") {
      const meta = { ...(obj.metadata as Record<string, unknown>) };
      delete meta.managedFields;
      obj.metadata = meta;
    }
    return jsYaml.dump(obj, { lineWidth: -1 });
  } catch {
    return yamlStr;
  }
}
