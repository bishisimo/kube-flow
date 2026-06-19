import type { K8sObject } from "./types";
import { getAtPath, getPodTemplatePath, getPodTemplateSpecPath, setAtPath } from "./workloadPaths";

export interface KeyValuePair {
  key: string;
  value: string;
}

/** 结构化编辑中由表单托管的字段；Spec / Pod 细节由 YAML 分区负责。 */
export interface WorkloadDraft {
  metadata: { labels: KeyValuePair[]; annotations: KeyValuePair[] };
  serviceAccountName: string;
  restartPolicy: string;
}

function recordToPairs(obj: unknown): KeyValuePair[] {
  if (!obj || typeof obj !== "object" || Array.isArray(obj)) return [];
  return Object.entries(obj as Record<string, unknown>).map(([key, value]) => ({
    key,
    value: value == null ? "" : String(value),
  }));
}

function pairsToRecord(pairs: KeyValuePair[]): Record<string, string> {
  const out: Record<string, string> = {};
  for (const p of pairs) {
    const k = p.key.trim();
    if (!k) continue;
    out[k] = p.value;
  }
  return out;
}

export function createEmptyWorkloadDraft(): WorkloadDraft {
  return {
    metadata: { labels: [], annotations: [] },
    serviceAccountName: "",
    restartPolicy: "Always",
  };
}

/** 从资源对象解析表单壳层字段。 */
export function parseWorkloadDraft(obj: K8sObject, _kind: string): WorkloadDraft {
  const draft = createEmptyWorkloadDraft();
  const meta = obj.metadata && typeof obj.metadata === "object" ? (obj.metadata as Record<string, unknown>) : {};
  draft.metadata.labels = recordToPairs(meta.labels);
  draft.metadata.annotations = recordToPairs(meta.annotations);

  const podSpec = getAtPath(obj, getPodTemplateSpecPath(_kind)) as Record<string, unknown> | undefined;
  if (podSpec) {
    draft.serviceAccountName =
      typeof podSpec.serviceAccountName === "string" ? podSpec.serviceAccountName : "";
    draft.restartPolicy = typeof podSpec.restartPolicy === "string" ? podSpec.restartPolicy : "Always";
  }

  return draft;
}

/** 将表单壳层字段写回资源对象（YAML 托管字段由 mergeWorkloadYamlRegions 覆盖）。 */
export function applyWorkloadDraft(base: K8sObject, draft: WorkloadDraft, kind: string): K8sObject {
  const out = JSON.parse(JSON.stringify(base)) as K8sObject;

  const meta = (out.metadata ?? {}) as Record<string, unknown>;
  const labels = pairsToRecord(draft.metadata.labels);
  const annotations = pairsToRecord(draft.metadata.annotations);
  if (Object.keys(labels).length) meta.labels = labels;
  else delete meta.labels;
  if (Object.keys(annotations).length) meta.annotations = annotations;
  else delete meta.annotations;
  out.metadata = meta;

  const templatePath = getPodTemplatePath(kind);
  const template = (getAtPath(out, templatePath) ?? {}) as Record<string, unknown>;

  const podSpecPath = getPodTemplateSpecPath(kind);
  const existingPodSpec = {
    ...((getAtPath(out, podSpecPath) ?? {}) as Record<string, unknown>),
  };

  if (draft.serviceAccountName.trim()) {
    existingPodSpec.serviceAccountName = draft.serviceAccountName.trim();
  } else {
    delete existingPodSpec.serviceAccountName;
  }
  if (draft.restartPolicy) existingPodSpec.restartPolicy = draft.restartPolicy;

  setAtPath(out, podSpecPath, existingPodSpec);
  setAtPath(out, templatePath, template);

  return out;
}

/** 校验表单壳层字段；通过返回 null。 */
export function validateWorkloadDraftShell(_draft: WorkloadDraft): string | null {
  return null;
}
