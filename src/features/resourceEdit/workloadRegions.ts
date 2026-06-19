import * as jsYaml from "js-yaml";
import type { K8sObject } from "./types";
import { getAtPath, getPodTemplatePath, getPodTemplateSpecPath, setAtPath } from "./workloadPaths";

/** YAML 分区语义分组（用于 UI Tab 过滤）。 */
export type WorkloadYamlRegionGroup = "workload" | "scheduling" | "pod" | "resource" | "template";

/** 工作负载结构化编辑中的 YAML 语义分区。 */
export type WorkloadYamlRegionKind =
  | "workloadSpec"
  | "templateMetadata"
  | "containers"
  | "initContainers"
  | "volumes"
  | "nodeSelector"
  | "tolerations"
  | "affinity"
  | "podSettings"
  | "securityContext"
  | "imagePullSecrets"
  | "dnsConfig"
  | "hostAliases";

export type WorkloadYamlRegionScope = "pod" | "resource" | "template";

export type WorkloadYamlRegionMode =
  | "split-array"
  | "whole-array"
  | "whole-object"
  | "pod-settings";

export interface WorkloadYamlRegionDef {
  id: WorkloadYamlRegionKind;
  scope: WorkloadYamlRegionScope;
  group: WorkloadYamlRegionGroup;
  label: string;
  hint: string;
  /** Pod Spec 叶子键；resource/template scope 不使用 */
  podKey: string;
  mode: WorkloadYamlRegionMode;
  /** 为空时从目标对象删除该字段 */
  optional: boolean;
  itemLabel: (item: unknown, index: number) => string;
  emptyItemYaml: string;
  emptyYaml: string;
}

const POD_SETTINGS_KEYS = [
  "terminationGracePeriodSeconds",
  "dnsPolicy",
  "priorityClassName",
  "runtimeClassName",
  "hostNetwork",
  "automountServiceAccountToken",
] as const;

const CONTAINER_EMPTY = `name: ""
image: ""
`;

function containerItemLabel(item: unknown, _index: number, fallback: string): string {
  if (item && typeof item === "object") {
    const obj = item as Record<string, unknown>;
    const name = typeof obj.name === "string" ? obj.name.trim() : "";
    const image = typeof obj.image === "string" ? obj.image.trim() : "";
    if (name && image) return `${name}`;
    if (name) return name;
    if (image) return image;
  }
  return fallback;
}

export function containerItemImage(itemYaml: string): string {
  try {
    const parsed = jsYaml.load(itemYaml);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      const image = (parsed as Record<string, unknown>).image;
      return typeof image === "string" ? image.trim() : "";
    }
  } catch {
    /* 留给 YAML 校验 */
  }
  return "";
}

export const WORKLOAD_YAML_REGIONS: WorkloadYamlRegionDef[] = [
  {
    id: "workloadSpec",
    scope: "resource",
    group: "resource",
    label: "Workload Spec",
    hint: "replicas、strategy、schedule 等（不含 Pod template）",
    podKey: "",
    mode: "whole-object",
    optional: true,
    itemLabel: () => "workloadSpec",
    emptyItemYaml: "",
    emptyYaml: "{}\n",
  },
  {
    id: "templateMetadata",
    scope: "template",
    group: "template",
    label: "Template Metadata",
    hint: "Pod template 的 labels 与 annotations",
    podKey: "",
    mode: "whole-object",
    optional: true,
    itemLabel: () => "templateMetadata",
    emptyItemYaml: "",
    emptyYaml: "{}\n",
  },
  {
    id: "initContainers",
    scope: "pod",
    group: "workload",
    label: "Init 容器",
    hint: "每个 Init 容器为独立 YAML 对象",
    podKey: "initContainers",
    mode: "split-array",
    optional: true,
    itemLabel: (item, index) => containerItemLabel(item, index, `Init ${index + 1}`),
    emptyItemYaml: CONTAINER_EMPTY,
    emptyYaml: "[]\n",
  },
  {
    id: "containers",
    scope: "pod",
    group: "workload",
    label: "容器",
    hint: "编辑该容器完整 YAML（含探针、资源、挂载等）",
    podKey: "containers",
    mode: "split-array",
    optional: false,
    itemLabel: (item, index) => containerItemLabel(item, index, `容器 ${index + 1}`),
    emptyItemYaml: CONTAINER_EMPTY,
    emptyYaml: "[]\n",
  },
  {
    id: "volumes",
    scope: "pod",
    group: "workload",
    label: "Volumes",
    hint: "YAML 数组；每项为一个 Volume 对象",
    podKey: "volumes",
    mode: "whole-array",
    optional: true,
    itemLabel: () => "volumes",
    emptyItemYaml: "",
    emptyYaml: "[]\n",
  },
  {
    id: "nodeSelector",
    scope: "pod",
    group: "scheduling",
    label: "nodeSelector",
    hint: "YAML 对象（key-value 映射）",
    podKey: "nodeSelector",
    mode: "whole-object",
    optional: true,
    itemLabel: () => "nodeSelector",
    emptyItemYaml: "",
    emptyYaml: "{}\n",
  },
  {
    id: "tolerations",
    scope: "pod",
    group: "scheduling",
    label: "Tolerations",
    hint: "YAML 数组；每项为一个 Toleration 对象",
    podKey: "tolerations",
    mode: "whole-array",
    optional: true,
    itemLabel: () => "tolerations",
    emptyItemYaml: "",
    emptyYaml: "[]\n",
  },
  {
    id: "affinity",
    scope: "pod",
    group: "scheduling",
    label: "Affinity",
    hint: "nodeAffinity / podAffinity / podAntiAffinity",
    podKey: "affinity",
    mode: "whole-object",
    optional: true,
    itemLabel: () => "affinity",
    emptyItemYaml: "",
    emptyYaml: "{}\n",
  },
  {
    id: "podSettings",
    scope: "pod",
    group: "pod",
    label: "Pod 设置",
    hint: "terminationGracePeriodSeconds、dnsPolicy、priorityClassName 等",
    podKey: "",
    mode: "pod-settings",
    optional: true,
    itemLabel: () => "podSettings",
    emptyItemYaml: "",
    emptyYaml: "{}\n",
  },
  {
    id: "securityContext",
    scope: "pod",
    group: "pod",
    label: "Security Context",
    hint: "Pod 级 securityContext 对象",
    podKey: "securityContext",
    mode: "whole-object",
    optional: true,
    itemLabel: () => "securityContext",
    emptyItemYaml: "",
    emptyYaml: "{}\n",
  },
  {
    id: "imagePullSecrets",
    scope: "pod",
    group: "pod",
    label: "imagePullSecrets",
    hint: "YAML 数组；每项含 name 字段",
    podKey: "imagePullSecrets",
    mode: "whole-array",
    optional: true,
    itemLabel: () => "imagePullSecrets",
    emptyItemYaml: "",
    emptyYaml: "[]\n",
  },
  {
    id: "dnsConfig",
    scope: "pod",
    group: "pod",
    label: "dnsConfig",
    hint: "nameservers / searches / options；通常配合 dnsPolicy: None",
    podKey: "dnsConfig",
    mode: "whole-object",
    optional: true,
    itemLabel: () => "dnsConfig",
    emptyItemYaml: "",
    emptyYaml: "{}\n",
  },
  {
    id: "hostAliases",
    scope: "pod",
    group: "pod",
    label: "hostAliases",
    hint: "YAML 数组；每项含 ip 与 hostnames",
    podKey: "hostAliases",
    mode: "whole-array",
    optional: true,
    itemLabel: () => "hostAliases",
    emptyItemYaml: "",
    emptyYaml: "[]\n",
  },
];

export interface WorkloadYamlItemState {
  id: string;
  label: string;
  yaml: string;
}

export interface WorkloadYamlRegionState {
  id: WorkloadYamlRegionKind;
  label: string;
  mode: WorkloadYamlRegionMode;
  yaml: string;
  items: WorkloadYamlItemState[];
  baselineYaml: string;
  baselineItems: string[];
}

function regionDef(id: WorkloadYamlRegionKind): WorkloadYamlRegionDef | undefined {
  return WORKLOAD_YAML_REGIONS.find((r) => r.id === id);
}

function dumpYaml(value: unknown): string {
  if (value === undefined) return "";
  return jsYaml.dump(value, { lineWidth: -1, noRefs: true });
}

function newItemId(): string {
  return `item-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
}

function readPodValue(obj: K8sObject, kind: string, podKey: string): unknown {
  const podSpec = getAtPath(obj, getPodTemplateSpecPath(kind));
  if (!podSpec || typeof podSpec !== "object") return undefined;
  return (podSpec as Record<string, unknown>)[podKey];
}

function readWorkloadSpec(obj: K8sObject, kind: string): Record<string, unknown> {
  const spec = obj.spec && typeof obj.spec === "object" ? { ...(obj.spec as Record<string, unknown>) } : {};
  if (kind === "CronJob") {
    const jobTemplate = spec.jobTemplate;
    if (jobTemplate && typeof jobTemplate === "object") {
      const jt = jobTemplate as Record<string, unknown>;
      const jtSpec =
        jt.spec && typeof jt.spec === "object" ? { ...(jt.spec as Record<string, unknown>) } : {};
      delete jtSpec.template;
      if (Object.keys(jtSpec).length) spec.jobTemplate = { spec: jtSpec };
      else delete spec.jobTemplate;
    }
    return spec;
  }
  delete spec.template;
  return spec;
}

function mergeResourceSpecRegion(
  out: K8sObject,
  kind: string,
  value: unknown,
  empty: boolean,
): void {
  const existingSpec =
    out.spec && typeof out.spec === "object" ? { ...(out.spec as Record<string, unknown>) } : {};

  if (kind === "CronJob") {
    const existingJt =
      existingSpec.jobTemplate && typeof existingSpec.jobTemplate === "object"
        ? (existingSpec.jobTemplate as Record<string, unknown>)
        : {};
    const existingJtSpec =
      existingJt.spec && typeof existingJt.spec === "object"
        ? (existingJt.spec as Record<string, unknown>)
        : {};
    const preservedTemplate = existingJtSpec.template;

    if (empty) {
      if (preservedTemplate !== undefined) {
        out.spec = { jobTemplate: { spec: { template: preservedTemplate } } };
      } else {
        delete out.spec;
      }
      return;
    }

    const merged = { ...(value as Record<string, unknown>) };
    const mergedJt =
      merged.jobTemplate && typeof merged.jobTemplate === "object"
        ? (merged.jobTemplate as Record<string, unknown>)
        : {};
    const mergedJtSpec =
      mergedJt.spec && typeof mergedJt.spec === "object"
        ? { ...(mergedJt.spec as Record<string, unknown>) }
        : {};
    if (preservedTemplate !== undefined) mergedJtSpec.template = preservedTemplate;
    if (Object.keys(mergedJtSpec).length) merged.jobTemplate = { spec: mergedJtSpec };
    else delete merged.jobTemplate;
    out.spec = merged;
    return;
  }

  const preservedKey = "template";
  const preserved = existingSpec[preservedKey];
  if (empty) {
    if (preserved !== undefined) out.spec = { [preservedKey]: preserved };
    else delete out.spec;
    return;
  }

  const merged = { ...(value as Record<string, unknown>) };
  if (preserved !== undefined) merged[preservedKey] = preserved;
  out.spec = merged;
}

function readTemplateMetadata(obj: K8sObject, kind: string): Record<string, unknown> {
  const template = getAtPath(obj, getPodTemplatePath(kind));
  if (!template || typeof template !== "object") return {};
  const meta = (template as Record<string, unknown>).metadata;
  if (!meta || typeof meta !== "object" || Array.isArray(meta)) return {};
  const m = meta as Record<string, unknown>;
  const out: Record<string, unknown> = {};
  if (m.labels && typeof m.labels === "object" && !Array.isArray(m.labels)) {
    out.labels = m.labels;
  }
  if (m.annotations && typeof m.annotations === "object" && !Array.isArray(m.annotations)) {
    out.annotations = m.annotations;
  }
  return out;
}

function readRegionRaw(obj: K8sObject, kind: string, def: WorkloadYamlRegionDef): unknown {
  if (def.scope === "resource") return readWorkloadSpec(obj, kind);
  if (def.scope === "template") return readTemplateMetadata(obj, kind);
  if (def.mode === "pod-settings") return readPodSettings(obj, kind);
  return readPodValue(obj, kind, def.podKey);
}

function isEmptyValue(mode: WorkloadYamlRegionMode, value: unknown): boolean {
  if (mode === "whole-array") return !Array.isArray(value) || value.length === 0;
  if (mode === "whole-object" || mode === "pod-settings") {
    return value == null || (typeof value === "object" && !Array.isArray(value) && !Object.keys(value).length);
  }
  return false;
}

/** 从完整对象抽出各 YAML 分区的可编辑状态。 */
export function extractWorkloadYamlRegions(obj: K8sObject, kind: string): WorkloadYamlRegionState[] {
  return WORKLOAD_YAML_REGIONS.map((def) => {
    const raw = readRegionRaw(obj, kind, def);

    if (def.mode === "split-array") {
      const list = Array.isArray(raw) ? raw : [];
      const items = list.map((entry, index) => ({
        id: newItemId(),
        label: def.itemLabel(entry, index),
        yaml: dumpYaml(entry),
      }));
      return {
        id: def.id,
        label: def.label,
        mode: def.mode,
        yaml: "",
        items,
        baselineYaml: "",
        baselineItems: items.map((i) => i.yaml),
      };
    }

    const yaml = isEmptyValue(def.mode, raw) ? def.emptyYaml : dumpYaml(raw);
    return {
      id: def.id,
      label: def.label,
      mode: def.mode,
      yaml,
      items: [],
      baselineYaml: yaml,
      baselineItems: [],
    };
  });
}

export function regionsForGroup(
  regions: WorkloadYamlRegionState[],
  group: WorkloadYamlRegionGroup,
): WorkloadYamlRegionState[] {
  const ids = new Set(WORKLOAD_YAML_REGIONS.filter((d) => d.group === group).map((d) => d.id));
  return regions.filter((r) => ids.has(r.id));
}

export function isWorkloadYamlRegionDirty(region: WorkloadYamlRegionState): boolean {
  if (region.mode === "split-array") {
    if (region.items.length !== region.baselineItems.length) return true;
    return region.items.some((item, i) => item.yaml !== region.baselineItems[i]);
  }
  return region.yaml !== region.baselineYaml;
}

export function areWorkloadYamlRegionsDirty(regions: WorkloadYamlRegionState[]): boolean {
  return regions.some(isWorkloadYamlRegionDirty);
}

export function markWorkloadYamlRegionsClean(regions: WorkloadYamlRegionState[]): WorkloadYamlRegionState[] {
  return regions.map((region) => {
    if (region.mode === "split-array") {
      return {
        ...region,
        baselineItems: region.items.map((i) => i.yaml),
      };
    }
    return {
      ...region,
      baselineYaml: region.yaml,
    };
  });
}

function parseYamlDocument(yaml: string, label: string): { value: unknown; error: string | null } {
  const text = yaml.trim();
  if (!text) return { value: null, error: `${label} 不能为空。` };
  try {
    return { value: jsYaml.load(yaml), error: null };
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return { value: null, error: `${label} YAML 解析失败：${msg}` };
  }
}

function resolveRegionValue(
  region: WorkloadYamlRegionState,
  def: WorkloadYamlRegionDef,
): { value: unknown; error: string | null; empty: boolean } {
  if (def.mode === "split-array") {
    const list: unknown[] = [];
    for (let i = 0; i < region.items.length; i++) {
      const item = region.items[i];
      const label = `${def.label}「${item.label}」`;
      const { value, error } = parseYamlDocument(item.yaml, label);
      if (error) return { value: null, error, empty: false };
      if (value == null || typeof value !== "object" || Array.isArray(value)) {
        return { value: null, error: `${label} 必须是对象。`, empty: false };
      }
      list.push(value);
    }
    return { value: list, error: null, empty: list.length === 0 };
  }

  const text = region.yaml.trim();
  if (!text || text === def.emptyYaml.trim()) {
    return { value: def.mode === "whole-array" ? [] : {}, error: null, empty: true };
  }

  const { value, error } = parseYamlDocument(region.yaml, def.label);
  if (error) return { value: null, error, empty: false };

  if (def.mode === "whole-array") {
    if (!Array.isArray(value)) return { value: null, error: `${def.label} 必须是 YAML 数组。`, empty: false };
    return { value, error: null, empty: value.length === 0 };
  }

  if (value == null || typeof value !== "object" || Array.isArray(value)) {
    return { value: null, error: `${def.label} 必须是 YAML 对象。`, empty: false };
  }
  return { value, error: null, empty: isEmptyValue(def.mode === "pod-settings" ? "whole-object" : def.mode, value) };
}

function readPodSettings(obj: K8sObject, kind: string): Record<string, unknown> {
  const podSpec = getAtPath(obj, getPodTemplateSpecPath(kind));
  if (!podSpec || typeof podSpec !== "object") return {};
  const spec = podSpec as Record<string, unknown>;
  const out: Record<string, unknown> = {};
  for (const key of POD_SETTINGS_KEYS) {
    if (spec[key] !== undefined) out[key] = spec[key];
  }
  return out;
}

function validateVolumeMountReferences(regions: WorkloadYamlRegionState[]): string | null {
  const volumesRegion = regions.find((r) => r.id === "volumes");
  const volumeNames = new Set<string>();

  if (volumesRegion) {
    const volDef = regionDef("volumes");
    if (volDef) {
      const { value, error, empty } = resolveRegionValue(volumesRegion, volDef);
      if (error) return error;
      if (!empty) {
        for (const entry of value as unknown[]) {
          if (!entry || typeof entry !== "object") continue;
          const name = typeof (entry as { name?: unknown }).name === "string"
            ? (entry as { name: string }).name.trim()
            : "";
          if (name) volumeNames.add(name);
        }
      }
    }
  }

  const checkContainerRegion = (region: WorkloadYamlRegionState): string | null => {
    for (const item of region.items) {
      let parsed: unknown;
      try {
        parsed = jsYaml.load(item.yaml);
      } catch {
        continue;
      }
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) continue;
      const label = item.label.trim() || "未命名容器";
      const mounts = Array.isArray((parsed as Record<string, unknown>).volumeMounts)
        ? ((parsed as Record<string, unknown>).volumeMounts as unknown[])
        : [];
      for (const mount of mounts) {
        if (!mount || typeof mount !== "object" || Array.isArray(mount)) continue;
        const m = mount as Record<string, unknown>;
        const volName = typeof m.name === "string" ? m.name.trim() : "";
        const mountPath = typeof m.mountPath === "string" ? m.mountPath.trim() : "";
        if (volName && !volumeNames.has(volName)) {
          return `容器「${label}」的挂载引用了未定义的 Volume「${volName}」。`;
        }
        if (mountPath && !mountPath.startsWith("/")) {
          return `容器「${label}」的 mountPath 必须以 / 开头：${mountPath}`;
        }
      }
    }
    return null;
  };

  for (const id of ["containers", "initContainers"] as const) {
    const region = regions.find((r) => r.id === id);
    if (!region) continue;
    const err = checkContainerRegion(region);
    if (err) return err;
  }

  return null;
}

/** 校验各 YAML 分区语法与基本结构；通过返回 null。 */
export function validateWorkloadYamlRegions(regions: WorkloadYamlRegionState[]): string | null {
  let hasBusinessContainer = false;

  for (const region of regions) {
    const def = regionDef(region.id);
    if (!def) continue;

    const { value, error, empty } = resolveRegionValue(region, def);
    if (error) return error;

    if (region.id === "containers") {
      const list = value as unknown[];
      if (!list.length) return "至少需要一个业务容器。";
      hasBusinessContainer = true;
      for (const entry of list) {
        const obj = entry as Record<string, unknown>;
        const name = typeof obj.name === "string" ? obj.name.trim() : "";
        const image = typeof obj.image === "string" ? obj.image.trim() : "";
        if (!name) return "业务容器名称不能为空。";
        if (!image) return `容器「${name}」缺少镜像。`;
      }
    }

    if (region.id === "initContainers" && !empty) {
      for (const entry of value as unknown[]) {
        const obj = entry as Record<string, unknown>;
        const name = typeof obj.name === "string" ? obj.name.trim() : "";
        if (!name) return "Init 容器名称不能为空。";
      }
    }

    if (region.id === "volumes" && !empty) {
      const names = new Set<string>();
      for (const entry of value as unknown[]) {
        if (!entry || typeof entry !== "object") return "Volume 项必须是对象。";
        const name = typeof (entry as { name?: unknown }).name === "string"
          ? (entry as { name: string }).name.trim()
          : "";
        if (!name) return "Volume 名称不能为空。";
        if (names.has(name)) return `Volume 名称重复：${name}`;
        names.add(name);
      }
    }

    if (region.id === "tolerations" && !empty) {
      for (const entry of value as unknown[]) {
        if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
          return "Toleration 项必须是对象。";
        }
      }
    }

    if (region.id === "imagePullSecrets" && !empty) {
      for (const entry of value as unknown[]) {
        const name =
          entry && typeof entry === "object" && typeof (entry as { name?: unknown }).name === "string"
            ? (entry as { name: string }).name.trim()
            : "";
        if (!name) return "imagePullSecrets 名称不能为空。";
      }
    }

    if (region.id === "hostAliases" && !empty) {
      for (const entry of value as unknown[]) {
        if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
          return "hostAliases 项必须是对象。";
        }
        const obj = entry as Record<string, unknown>;
        const ip = typeof obj.ip === "string" ? obj.ip.trim() : "";
        const hostnames = Array.isArray(obj.hostnames) ? obj.hostnames : [];
        if (!ip || !hostnames.length) {
          return "hostAliases 条目需要 ip 与至少一个 hostname。";
        }
      }
    }

    if (region.id === "dnsConfig" && !empty) {
      const obj = value as Record<string, unknown>;
      const options = Array.isArray(obj.options) ? obj.options : [];
      for (const opt of options) {
        if (!opt || typeof opt !== "object") continue;
        const name = typeof (opt as { name?: unknown }).name === "string"
          ? (opt as { name: string }).name.trim()
          : "";
        if (!name) return "dnsConfig option 的 name 不能为空。";
      }
    }
  }

  if (!hasBusinessContainer) return "至少需要一个业务容器。";

  return validateVolumeMountReferences(regions);
}

/**
 * 将 YAML 分区写回工作负载对象。
 * 调用方应先合并表单草稿，再调用本函数覆盖 YAML 托管字段。
 */
export function mergeWorkloadYamlRegions(
  base: K8sObject,
  kind: string,
  regions: WorkloadYamlRegionState[],
): { obj: K8sObject; error: string | null } {
  const out = JSON.parse(JSON.stringify(base)) as K8sObject;
  const podSpecPath = getPodTemplateSpecPath(kind);
  if (!podSpecPath.length) {
    return { obj: out, error: `不支持的 Kind：${kind}` };
  }

  const existingPodSpec = {
    ...((getAtPath(out, podSpecPath) ?? {}) as Record<string, unknown>),
  };

  for (const region of regions) {
    const def = regionDef(region.id);
    if (!def) continue;

    const { value, error, empty } = resolveRegionValue(region, def);
    if (error) return { obj: out, error };

    if (def.scope === "resource") {
      mergeResourceSpecRegion(out, kind, value, empty);
      continue;
    }

    if (def.scope === "template") {
      const templatePath = getPodTemplatePath(kind);
      const template = { ...((getAtPath(out, templatePath) ?? {}) as Record<string, unknown>) };
      if (empty) {
        delete template.metadata;
      } else {
        const v = value as Record<string, unknown>;
        const meta: Record<string, unknown> = {};
        if (
          v.labels &&
          typeof v.labels === "object" &&
          !Array.isArray(v.labels) &&
          Object.keys(v.labels as object).length
        ) {
          meta.labels = v.labels;
        }
        if (
          v.annotations &&
          typeof v.annotations === "object" &&
          !Array.isArray(v.annotations) &&
          Object.keys(v.annotations as object).length
        ) {
          meta.annotations = v.annotations;
        }
        if (Object.keys(meta).length) template.metadata = meta;
        else delete template.metadata;
      }
      setAtPath(out, templatePath, template);
      continue;
    }

    if (def.mode === "pod-settings") {
      for (const key of POD_SETTINGS_KEYS) {
        delete existingPodSpec[key];
      }
      if (!empty && value && typeof value === "object" && !Array.isArray(value)) {
        for (const [key, val] of Object.entries(value as Record<string, unknown>)) {
          if ((POD_SETTINGS_KEYS as readonly string[]).includes(key)) {
            existingPodSpec[key] = val;
          }
        }
      }
      continue;
    }

    if (def.optional && empty) {
      delete existingPodSpec[def.podKey];
      continue;
    }

    if (def.id === "containers") {
      existingPodSpec[def.podKey] = value;
    } else if (!empty) {
      existingPodSpec[def.podKey] = value;
    } else {
      delete existingPodSpec[def.podKey];
    }
  }

  setAtPath(out, podSpecPath, existingPodSpec);
  return { obj: out, error: null };
}

export function addYamlRegionItem(
  region: WorkloadYamlRegionState,
  def: WorkloadYamlRegionDef,
): WorkloadYamlRegionState {
  if (region.mode !== "split-array") return region;
  const index = region.items.length;
  const item: WorkloadYamlItemState = {
    id: newItemId(),
    label: def.itemLabel({}, index),
    yaml: def.emptyItemYaml,
  };
  return { ...region, items: [...region.items, item] };
}

export function removeYamlRegionItem(
  region: WorkloadYamlRegionState,
  itemId: string,
): WorkloadYamlRegionState {
  if (region.mode !== "split-array") return region;
  return {
    ...region,
    items: region.items.filter((i) => i.id !== itemId),
  };
}

export function updateYamlRegionItem(
  region: WorkloadYamlRegionState,
  itemId: string,
  yaml: string,
): WorkloadYamlRegionState {
  if (region.mode !== "split-array") return region;
  const def = regionDef(region.id);
  return {
    ...region,
    items: region.items.map((item) => {
      if (item.id !== itemId) return item;
      let label = item.label;
      if (def) {
        try {
          const parsed = jsYaml.load(yaml);
          const idx = region.items.findIndex((i) => i.id === itemId);
          label = def.itemLabel(parsed, idx >= 0 ? idx : 0);
        } catch {
          /* 语法错误留给校验 */
        }
      }
      return { ...item, yaml, label };
    }),
  };
}

export function patchYamlRegion(
  regions: WorkloadYamlRegionState[],
  id: WorkloadYamlRegionKind,
  next: WorkloadYamlRegionState,
): WorkloadYamlRegionState[] {
  return regions.map((r) => (r.id === id ? next : r));
}

export function workloadObjectToYaml(obj: K8sObject): string {
  return jsYaml.dump(obj, { lineWidth: -1, noRefs: true });
}
