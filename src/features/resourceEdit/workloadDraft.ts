import * as jsYaml from "js-yaml";
import type { K8sObject } from "./types";
import { getAtPath, getPodTemplateSpecPath, setAtPath } from "./workloadPaths";

export interface KeyValuePair {
  key: string;
  value: string;
}

export interface PortDraft {
  name: string;
  containerPort: number | null;
  protocol: string;
}

export interface VolumeMountDraft {
  name: string;
  mountPath: string;
  readOnly: boolean;
}

export interface EnvDraft {
  name: string;
  value: string;
}

export interface ContainerDraft {
  name: string;
  image: string;
  imagePullPolicy: string;
  commandText: string;
  argsText: string;
  env: EnvDraft[];
  cpuRequest: string;
  memoryRequest: string;
  cpuLimit: string;
  memoryLimit: string;
  ports: PortDraft[];
  volumeMounts: VolumeMountDraft[];
}

export type VolumeType = "emptyDir" | "configMap" | "secret" | "pvc" | "hostPath";

export interface VolumeDraft {
  name: string;
  type: VolumeType;
  configMapName: string;
  secretName: string;
  pvcName: string;
  hostPath: string;
}

export interface TolerationDraft {
  key: string;
  operator: string;
  value: string;
  effect: string;
}

export interface WorkloadDraft {
  metadata: { labels: KeyValuePair[]; annotations: KeyValuePair[] };
  replicas: number | null;
  strategyType: string;
  maxSurge: string;
  maxUnavailable: string;
  updateStrategyType: string;
  serviceName: string;
  schedule: string;
  suspend: boolean;
  concurrencyPolicy: string;
  parallelism: number | null;
  completions: number | null;
  backoffLimit: number | null;
  templateLabels: KeyValuePair[];
  templateAnnotations: KeyValuePair[];
  initContainers: ContainerDraft[];
  containers: ContainerDraft[];
  volumes: VolumeDraft[];
  serviceAccountName: string;
  restartPolicy: string;
  nodeSelector: KeyValuePair[];
  tolerations: TolerationDraft[];
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

function linesToArray(text: string): string[] {
  return text
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
}

function arrayToLines(arr: unknown): string {
  if (!Array.isArray(arr)) return "";
  return arr.map((v) => String(v)).join("\n");
}

function parseContainer(raw: Record<string, unknown>): ContainerDraft {
  const resources =
    raw.resources && typeof raw.resources === "object"
      ? (raw.resources as Record<string, unknown>)
      : {};
  const requests =
    resources.requests && typeof resources.requests === "object"
      ? (resources.requests as Record<string, unknown>)
      : {};
  const limits =
    resources.limits && typeof resources.limits === "object"
      ? (resources.limits as Record<string, unknown>)
      : {};

  const envList = Array.isArray(raw.env) ? raw.env : [];
  const portsList = Array.isArray(raw.ports) ? raw.ports : [];
  const mountsList = Array.isArray(raw.volumeMounts) ? raw.volumeMounts : [];

  return {
    name: typeof raw.name === "string" ? raw.name : "",
    image: typeof raw.image === "string" ? raw.image : "",
    imagePullPolicy: typeof raw.imagePullPolicy === "string" ? raw.imagePullPolicy : "IfNotPresent",
    commandText: arrayToLines(raw.command),
    argsText: arrayToLines(raw.args),
    env: envList
      .filter((e): e is Record<string, unknown> => !!e && typeof e === "object")
      .map((e) => ({
        name: typeof e.name === "string" ? e.name : "",
        value: typeof e.value === "string" ? e.value : "",
      })),
    cpuRequest: typeof requests.cpu === "string" ? requests.cpu : "",
    memoryRequest: typeof requests.memory === "string" ? requests.memory : "",
    cpuLimit: typeof limits.cpu === "string" ? limits.cpu : "",
    memoryLimit: typeof limits.memory === "string" ? limits.memory : "",
    ports: portsList
      .filter((p): p is Record<string, unknown> => !!p && typeof p === "object")
      .map((p) => ({
        name: typeof p.name === "string" ? p.name : "",
        containerPort: typeof p.containerPort === "number" ? p.containerPort : null,
        protocol: typeof p.protocol === "string" ? p.protocol : "TCP",
      })),
    volumeMounts: mountsList
      .filter((m): m is Record<string, unknown> => !!m && typeof m === "object")
      .map((m) => ({
        name: typeof m.name === "string" ? m.name : "",
        mountPath: typeof m.mountPath === "string" ? m.mountPath : "",
        readOnly: m.readOnly === true,
      })),
  };
}

function buildContainer(draft: ContainerDraft): Record<string, unknown> {
  const out: Record<string, unknown> = {
    name: draft.name.trim(),
    image: draft.image.trim(),
    imagePullPolicy: draft.imagePullPolicy || "IfNotPresent",
  };
  const command = linesToArray(draft.commandText);
  const args = linesToArray(draft.argsText);
  if (command.length) out.command = command;
  if (args.length) out.args = args;

  const env = draft.env
    .filter((e) => e.name.trim())
    .map((e) => ({ name: e.name.trim(), value: e.value }));
  if (env.length) out.env = env;

  const requests: Record<string, string> = {};
  const limits: Record<string, string> = {};
  if (draft.cpuRequest.trim()) requests.cpu = draft.cpuRequest.trim();
  if (draft.memoryRequest.trim()) requests.memory = draft.memoryRequest.trim();
  if (draft.cpuLimit.trim()) limits.cpu = draft.cpuLimit.trim();
  if (draft.memoryLimit.trim()) limits.memory = draft.memoryLimit.trim();
  if (Object.keys(requests).length || Object.keys(limits).length) {
    out.resources = {
      ...(Object.keys(requests).length ? { requests } : {}),
      ...(Object.keys(limits).length ? { limits } : {}),
    };
  }

  const ports = draft.ports
    .filter((p) => p.containerPort != null)
    .map((p) => ({
      ...(p.name.trim() ? { name: p.name.trim() } : {}),
      containerPort: p.containerPort,
      protocol: p.protocol || "TCP",
    }));
  if (ports.length) out.ports = ports;

  const volumeMounts = draft.volumeMounts
    .filter((m) => m.name.trim() && m.mountPath.trim())
    .map((m) => ({
      name: m.name.trim(),
      mountPath: m.mountPath.trim(),
      ...(m.readOnly ? { readOnly: true } : {}),
    }));
  if (volumeMounts.length) out.volumeMounts = volumeMounts;

  return out;
}

function parseVolume(raw: Record<string, unknown>): VolumeDraft {
  const name = typeof raw.name === "string" ? raw.name : "";
  if (raw.emptyDir != null) {
    return { name, type: "emptyDir", configMapName: "", secretName: "", pvcName: "", hostPath: "" };
  }
  if (raw.configMap && typeof raw.configMap === "object") {
    const cm = raw.configMap as Record<string, unknown>;
    return {
      name,
      type: "configMap",
      configMapName: typeof cm.name === "string" ? cm.name : "",
      secretName: "",
      pvcName: "",
      hostPath: "",
    };
  }
  if (raw.secret && typeof raw.secret === "object") {
    const sec = raw.secret as Record<string, unknown>;
    return {
      name,
      type: "secret",
      configMapName: "",
      secretName: typeof sec.secretName === "string" ? sec.secretName : "",
      pvcName: "",
      hostPath: "",
    };
  }
  if (raw.persistentVolumeClaim && typeof raw.persistentVolumeClaim === "object") {
    const pvc = raw.persistentVolumeClaim as Record<string, unknown>;
    return {
      name,
      type: "pvc",
      configMapName: "",
      secretName: "",
      pvcName: typeof pvc.claimName === "string" ? pvc.claimName : "",
      hostPath: "",
    };
  }
  if (raw.hostPath && typeof raw.hostPath === "object") {
    const hp = raw.hostPath as Record<string, unknown>;
    return {
      name,
      type: "hostPath",
      configMapName: "",
      secretName: "",
      pvcName: "",
      hostPath: typeof hp.path === "string" ? hp.path : "",
    };
  }
  return { name, type: "emptyDir", configMapName: "", secretName: "", pvcName: "", hostPath: "" };
}

function buildVolume(draft: VolumeDraft): Record<string, unknown> {
  const out: Record<string, unknown> = { name: draft.name.trim() };
  switch (draft.type) {
    case "configMap":
      out.configMap = { name: draft.configMapName.trim() };
      break;
    case "secret":
      out.secret = { secretName: draft.secretName.trim() };
      break;
    case "pvc":
      out.persistentVolumeClaim = { claimName: draft.pvcName.trim() };
      break;
    case "hostPath":
      out.hostPath = { path: draft.hostPath.trim() };
      break;
    default:
      out.emptyDir = {};
  }
  return out;
}

function getTemplatePath(kind: string): string[] {
  return kind === "CronJob"
    ? ["spec", "jobTemplate", "spec", "template"]
    : ["spec", "template"];
}

function emptyContainer(): ContainerDraft {
  return {
    name: "",
    image: "",
    imagePullPolicy: "IfNotPresent",
    commandText: "",
    argsText: "",
    env: [],
    cpuRequest: "",
    memoryRequest: "",
    cpuLimit: "",
    memoryLimit: "",
    ports: [],
    volumeMounts: [],
  };
}

export function createEmptyWorkloadDraft(): WorkloadDraft {
  return {
    metadata: { labels: [], annotations: [] },
    replicas: 1,
    strategyType: "RollingUpdate",
    maxSurge: "25%",
    maxUnavailable: "25%",
    updateStrategyType: "RollingUpdate",
    serviceName: "",
    schedule: "",
    suspend: false,
    concurrencyPolicy: "Allow",
    parallelism: 1,
    completions: 1,
    backoffLimit: 6,
    templateLabels: [],
    templateAnnotations: [],
    initContainers: [],
    containers: [emptyContainer()],
    volumes: [],
    serviceAccountName: "",
    restartPolicy: "Always",
    nodeSelector: [],
    tolerations: [],
  };
}

export function parseWorkloadDraft(obj: K8sObject, kind: string): WorkloadDraft {
  const draft = createEmptyWorkloadDraft();
  const meta = obj.metadata && typeof obj.metadata === "object" ? (obj.metadata as Record<string, unknown>) : {};
  draft.metadata.labels = recordToPairs(meta.labels);
  draft.metadata.annotations = recordToPairs(meta.annotations);

  const spec = obj.spec && typeof obj.spec === "object" ? (obj.spec as Record<string, unknown>) : {};

  if (kind === "Deployment" || kind === "StatefulSet") {
    draft.replicas = typeof spec.replicas === "number" ? spec.replicas : 1;
  }
  if (kind === "Deployment") {
    const strategy =
      spec.strategy && typeof spec.strategy === "object" ? (spec.strategy as Record<string, unknown>) : {};
    draft.strategyType = typeof strategy.type === "string" ? strategy.type : "RollingUpdate";
    const rolling =
      strategy.rollingUpdate && typeof strategy.rollingUpdate === "object"
        ? (strategy.rollingUpdate as Record<string, unknown>)
        : {};
    draft.maxSurge = rolling.maxSurge != null ? String(rolling.maxSurge) : "25%";
    draft.maxUnavailable = rolling.maxUnavailable != null ? String(rolling.maxUnavailable) : "25%";
  }
  if (kind === "StatefulSet") {
    draft.serviceName = typeof spec.serviceName === "string" ? spec.serviceName : "";
    const us =
      spec.updateStrategy && typeof spec.updateStrategy === "object"
        ? (spec.updateStrategy as Record<string, unknown>)
        : {};
    draft.updateStrategyType = typeof us.type === "string" ? us.type : "RollingUpdate";
  }
  if (kind === "DaemonSet") {
    const us =
      spec.updateStrategy && typeof spec.updateStrategy === "object"
        ? (spec.updateStrategy as Record<string, unknown>)
        : {};
    draft.updateStrategyType = typeof us.type === "string" ? us.type : "RollingUpdate";
  }
  if (kind === "Job") {
    draft.parallelism = typeof spec.parallelism === "number" ? spec.parallelism : 1;
    draft.completions = typeof spec.completions === "number" ? spec.completions : 1;
    draft.backoffLimit = typeof spec.backoffLimit === "number" ? spec.backoffLimit : 6;
  }
  if (kind === "CronJob") {
    draft.schedule = typeof spec.schedule === "string" ? spec.schedule : "";
    draft.suspend = spec.suspend === true;
    draft.concurrencyPolicy =
      typeof spec.concurrencyPolicy === "string" ? spec.concurrencyPolicy : "Allow";
    const jobSpec =
      spec.jobTemplate && typeof spec.jobTemplate === "object"
        ? ((spec.jobTemplate as Record<string, unknown>).spec as Record<string, unknown> | undefined)
        : undefined;
    if (jobSpec) {
      draft.parallelism = typeof jobSpec.parallelism === "number" ? jobSpec.parallelism : 1;
      draft.completions = typeof jobSpec.completions === "number" ? jobSpec.completions : 1;
      draft.backoffLimit = typeof jobSpec.backoffLimit === "number" ? jobSpec.backoffLimit : 6;
    }
  }

  const template = getAtPath(obj, getTemplatePath(kind)) as Record<string, unknown> | undefined;
  const templateMeta =
    template?.metadata && typeof template.metadata === "object"
      ? (template.metadata as Record<string, unknown>)
      : {};
  draft.templateLabels = recordToPairs(templateMeta.labels);
  draft.templateAnnotations = recordToPairs(templateMeta.annotations);

  const podSpec = getAtPath(obj, getPodTemplateSpecPath(kind)) as Record<string, unknown> | undefined;
  if (podSpec) {
    const initList = Array.isArray(podSpec.initContainers) ? podSpec.initContainers : [];
    const containerList = Array.isArray(podSpec.containers) ? podSpec.containers : [];
    draft.initContainers = initList
      .filter((c): c is Record<string, unknown> => !!c && typeof c === "object")
      .map(parseContainer);
    draft.containers = containerList.length
      ? containerList
          .filter((c): c is Record<string, unknown> => !!c && typeof c === "object")
          .map(parseContainer)
      : [emptyContainer()];

    const volList = Array.isArray(podSpec.volumes) ? podSpec.volumes : [];
    draft.volumes = volList
      .filter((v): v is Record<string, unknown> => !!v && typeof v === "object")
      .map(parseVolume);

    draft.serviceAccountName =
      typeof podSpec.serviceAccountName === "string" ? podSpec.serviceAccountName : "";
    draft.restartPolicy = typeof podSpec.restartPolicy === "string" ? podSpec.restartPolicy : "Always";
    draft.nodeSelector = recordToPairs(podSpec.nodeSelector);

    const tolList = Array.isArray(podSpec.tolerations) ? podSpec.tolerations : [];
    draft.tolerations = tolList
      .filter((t): t is Record<string, unknown> => !!t && typeof t === "object")
      .map((t) => ({
        key: typeof t.key === "string" ? t.key : "",
        operator: typeof t.operator === "string" ? t.operator : "Equal",
        value: typeof t.value === "string" ? t.value : "",
        effect: typeof t.effect === "string" ? t.effect : "",
      }));
  }

  return draft;
}

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

  const spec = (out.spec ?? {}) as Record<string, unknown>;

  if (kind === "Deployment" || kind === "StatefulSet") {
    spec.replicas = draft.replicas ?? 1;
  }
  if (kind === "Deployment") {
    spec.strategy = {
      type: draft.strategyType || "RollingUpdate",
      ...(draft.strategyType === "RollingUpdate"
        ? {
            rollingUpdate: {
              maxSurge: draft.maxSurge || "25%",
              maxUnavailable: draft.maxUnavailable || "25%",
            },
          }
        : {}),
    };
  }
  if (kind === "StatefulSet") {
    if (draft.serviceName.trim()) spec.serviceName = draft.serviceName.trim();
    spec.updateStrategy = { type: draft.updateStrategyType || "RollingUpdate" };
  }
  if (kind === "DaemonSet") {
    spec.updateStrategy = { type: draft.updateStrategyType || "RollingUpdate" };
  }
  if (kind === "Job") {
    spec.parallelism = draft.parallelism ?? 1;
    spec.completions = draft.completions ?? 1;
    if (draft.backoffLimit != null) spec.backoffLimit = draft.backoffLimit;
  }
  if (kind === "CronJob") {
    spec.schedule = draft.schedule;
    spec.suspend = draft.suspend;
    spec.concurrencyPolicy = draft.concurrencyPolicy || "Allow";
    const jobTemplate = (spec.jobTemplate ?? {}) as Record<string, unknown>;
    const jobSpec = (jobTemplate.spec ?? {}) as Record<string, unknown>;
    jobSpec.parallelism = draft.parallelism ?? 1;
    jobSpec.completions = draft.completions ?? 1;
    if (draft.backoffLimit != null) jobSpec.backoffLimit = draft.backoffLimit;
    jobTemplate.spec = jobSpec;
    spec.jobTemplate = jobTemplate;
  }
  out.spec = spec;

  const templatePath = getTemplatePath(kind);
  const template = (getAtPath(out, templatePath) ?? {}) as Record<string, unknown>;
  const templateMeta = (template.metadata ?? {}) as Record<string, unknown>;
  const tLabels = pairsToRecord(draft.templateLabels);
  const tAnnotations = pairsToRecord(draft.templateAnnotations);
  if (Object.keys(tLabels).length) templateMeta.labels = tLabels;
  else delete templateMeta.labels;
  if (Object.keys(tAnnotations).length) templateMeta.annotations = tAnnotations;
  else delete templateMeta.annotations;
  template.metadata = templateMeta;

  const podSpecPath = getPodTemplateSpecPath(kind);
  const existingPodSpec = {
    ...((getAtPath(out, podSpecPath) ?? {}) as Record<string, unknown>),
  };

  existingPodSpec.initContainers = draft.initContainers
    .filter((c) => c.name.trim() || c.image.trim())
    .map(buildContainer);
  existingPodSpec.containers = draft.containers
    .filter((c) => c.name.trim() || c.image.trim())
    .map(buildContainer);
  if (!Array.isArray(existingPodSpec.containers) || !existingPodSpec.containers.length) {
    existingPodSpec.containers = [buildContainer(emptyContainer())];
  }

  if (draft.volumes.length) {
    existingPodSpec.volumes = draft.volumes.filter((v) => v.name.trim()).map(buildVolume);
  } else {
    delete existingPodSpec.volumes;
  }

  if (draft.serviceAccountName.trim()) {
    existingPodSpec.serviceAccountName = draft.serviceAccountName.trim();
  } else {
    delete existingPodSpec.serviceAccountName;
  }
  if (draft.restartPolicy) existingPodSpec.restartPolicy = draft.restartPolicy;

  const nodeSelector = pairsToRecord(draft.nodeSelector);
  if (Object.keys(nodeSelector).length) existingPodSpec.nodeSelector = nodeSelector;
  else delete existingPodSpec.nodeSelector;

  const tolerations = draft.tolerations
    .filter((t) => t.key.trim() || t.effect.trim())
    .map((t) => {
      const item: Record<string, string> = {};
      if (t.key.trim()) item.key = t.key.trim();
      if (t.operator.trim()) item.operator = t.operator.trim();
      if (t.value.trim()) item.value = t.value.trim();
      if (t.effect.trim()) item.effect = t.effect.trim();
      return item;
    });
  if (tolerations.length) existingPodSpec.tolerations = tolerations;
  else delete existingPodSpec.tolerations;

  setAtPath(out, podSpecPath, existingPodSpec);
  setAtPath(out, templatePath, template);

  return out;
}

export function workloadDraftToYaml(base: K8sObject, draft: WorkloadDraft, kind: string): string {
  const merged = applyWorkloadDraft(base, draft, kind);
  return jsYaml.dump(merged, { lineWidth: -1 });
}
