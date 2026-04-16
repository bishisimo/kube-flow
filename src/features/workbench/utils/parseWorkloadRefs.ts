import * as jsYaml from "js-yaml";
import { kubeListServices, kubeGetResource } from "../../../api/kube";

export type SyncRelatedKind = "ConfigMap" | "Secret" | "Service";
export type SyncRelatedRef = { kind: SyncRelatedKind; name: string; namespace: string | null };

export async function collectAssociatedRefsFromWorkloadYaml(
  envId: string,
  yaml: string,
  defaultNamespace: string
): Promise<SyncRelatedRef[]> {
  let parsed: unknown;
  try {
    parsed = jsYaml.load(yaml);
  } catch {
    return [];
  }
  if (!parsed || typeof parsed !== "object") return [];
  const obj = parsed as Record<string, unknown>;
  const kind = typeof obj.kind === "string" ? obj.kind : "";
  if (!["Deployment", "StatefulSet", "DaemonSet", "Pod"].includes(kind)) return [];

  const metadata = obj.metadata && typeof obj.metadata === "object" ? (obj.metadata as Record<string, unknown>) : null;
  const spec = obj.spec && typeof obj.spec === "object" ? (obj.spec as Record<string, unknown>) : null;
  const template =
    spec?.template && typeof spec.template === "object" ? (spec.template as Record<string, unknown>) : null;
  const podSpec =
    kind === "Pod"
      ? spec
      : template?.spec && typeof template.spec === "object"
        ? (template.spec as Record<string, unknown>)
        : null;
  if (!podSpec) return [];

  const refs: SyncRelatedRef[] = [];
  const pushRef = (refKind: SyncRelatedKind, refName: string) => {
    const n = refName.trim();
    if (!n) return;
    refs.push({ kind: refKind, name: n, namespace: defaultNamespace || "default" });
  };

  const volumes = Array.isArray(podSpec.volumes) ? (podSpec.volumes as Array<Record<string, unknown>>) : [];
  for (const v of volumes) {
    const cm = v.configMap && typeof v.configMap === "object" ? (v.configMap as Record<string, unknown>) : null;
    if (cm && typeof cm.name === "string") pushRef("ConfigMap", cm.name);
    const sec = v.secret && typeof v.secret === "object" ? (v.secret as Record<string, unknown>) : null;
    if (sec && typeof sec.secretName === "string") pushRef("Secret", sec.secretName);
  }

  const imagePullSecrets = Array.isArray(podSpec.imagePullSecrets)
    ? (podSpec.imagePullSecrets as Array<Record<string, unknown>>)
    : [];
  for (const s of imagePullSecrets) {
    if (typeof s.name === "string") pushRef("Secret", s.name);
  }

  const containers = [
    ...(Array.isArray(podSpec.containers) ? (podSpec.containers as Array<Record<string, unknown>>) : []),
    ...(Array.isArray(podSpec.initContainers) ? (podSpec.initContainers as Array<Record<string, unknown>>) : []),
  ];
  for (const c of containers) {
    const envFrom = Array.isArray(c.envFrom) ? (c.envFrom as Array<Record<string, unknown>>) : [];
    for (const ef of envFrom) {
      const cm =
        ef.configMapRef && typeof ef.configMapRef === "object"
          ? (ef.configMapRef as Record<string, unknown>)
          : null;
      if (cm && typeof cm.name === "string") pushRef("ConfigMap", cm.name);
      const sec =
        ef.secretRef && typeof ef.secretRef === "object" ? (ef.secretRef as Record<string, unknown>) : null;
      if (sec && typeof sec.name === "string") pushRef("Secret", sec.name);
    }

    const envList = Array.isArray(c.env) ? (c.env as Array<Record<string, unknown>>) : [];
    for (const env of envList) {
      const valueFrom =
        env.valueFrom && typeof env.valueFrom === "object"
          ? (env.valueFrom as Record<string, unknown>)
          : null;
      const cmRef =
        valueFrom?.configMapKeyRef && typeof valueFrom.configMapKeyRef === "object"
          ? (valueFrom.configMapKeyRef as Record<string, unknown>)
          : null;
      if (cmRef && typeof cmRef.name === "string") pushRef("ConfigMap", cmRef.name);
      const secRef =
        valueFrom?.secretKeyRef && typeof valueFrom.secretKeyRef === "object"
          ? (valueFrom.secretKeyRef as Record<string, unknown>)
          : null;
      if (secRef && typeof secRef.name === "string") pushRef("Secret", secRef.name);
    }
  }

  const podLabels =
    kind === "Pod"
      ? (metadata?.labels as Record<string, unknown> | undefined)
      : template?.metadata && typeof template.metadata === "object"
        ? ((template.metadata as Record<string, unknown>).labels as Record<string, unknown> | undefined)
        : undefined;
  const podLabelMap: Record<string, string> = {};
  if (podLabels && typeof podLabels === "object") {
    for (const [k, v] of Object.entries(podLabels)) {
      if (typeof v === "string") podLabelMap[k] = v;
    }
  }
  const labelKeys = Object.keys(podLabelMap);
  if (labelKeys.length > 0) {
    try {
      const services = await kubeListServices(envId, defaultNamespace || "default", null);
      for (const svc of services) {
        try {
          const serviceYaml = await kubeGetResource(envId, "Service", svc.name, svc.namespace || defaultNamespace || "default");
          const serviceParsed = jsYaml.load(serviceYaml);
          if (!serviceParsed || typeof serviceParsed !== "object") continue;
          const serviceObj = serviceParsed as Record<string, unknown>;
          const serviceSpec =
            serviceObj.spec && typeof serviceObj.spec === "object"
              ? (serviceObj.spec as Record<string, unknown>)
              : null;
          const selector =
            serviceSpec?.selector && typeof serviceSpec.selector === "object"
              ? (serviceSpec.selector as Record<string, unknown>)
              : null;
          if (!selector) continue;
          const selectorEntries = Object.entries(selector).filter(([, value]) => typeof value === "string");
          if (!selectorEntries.length) continue;
          const matched = selectorEntries.every(([key, value]) => podLabelMap[key] === value);
          if (matched) {
            pushRef("Service", svc.name);
          }
        } catch {
          continue;
        }
      }
    } catch {
      // 忽略 Service 扩展解析失败，不影响主流程。
    }
  }

  const dedup = new Map<string, SyncRelatedRef>();
  for (const ref of refs) {
    dedup.set(`${ref.kind}|${ref.namespace ?? ""}|${ref.name}`, ref);
  }
  return Array.from(dedup.values());
}
