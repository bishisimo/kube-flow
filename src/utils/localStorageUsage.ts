import type { OrchestratorImportBatch, OrchestratorManifest } from "../stores/orchestratorTypes";
import type { OrchestratorPackage } from "../stores/orchestratorPackages";
import type { ResourceSnapshotItem } from "../stores/resourceSnapshots";

const PREFIX = "kube-flow:";

const KEY_RESOURCE_SNAPSHOTS = "kube-flow:resource-snapshots";
const KEY_ORCHESTRATOR_MANIFESTS = "kube-flow:orchestrator:manifests";
const KEY_ORCHESTRATOR_BATCHES = "kube-flow:orchestrator:import-batches";
const KEY_ORCHESTRATOR_PACKAGES = "kube-flow:orchestrator:packages";
const ENV_VIEW_PREFIX = "kube-flow:env-view:";

export type LocalStorageManageAction = "snapshotCenter" | "orchestrator" | "workspace";

export interface LocalStorageCategoryUsage {
  id: string;
  label: string;
  description: string;
  bytes: number;
  detail: string;
  manageAction?: LocalStorageManageAction;
  clearable?: boolean;
}

export interface LocalStorageUsageReport {
  categories: LocalStorageCategoryUsage[];
  totalBytes: number;
}

function measureKeyBytes(key: string): number {
  const raw = localStorage.getItem(key);
  if (!raw) return 0;
  return new TextEncoder().encode(raw).byteLength;
}

function readWrappedData<T>(key: string): T | null {
  const raw = localStorage.getItem(key);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (parsed && typeof parsed === "object" && "data" in parsed) {
      return (parsed as { data: T }).data;
    }
    return parsed as T;
  } catch {
    return null;
  }
}

function sumKeyBytes(keys: string[]): number {
  return keys.reduce((sum, key) => sum + measureKeyBytes(key), 0);
}

function listKubeFlowKeys(): string[] {
  const keys: string[] = [];
  for (let i = 0; i < localStorage.length; i += 1) {
    const key = localStorage.key(i);
    if (key?.startsWith(PREFIX)) keys.push(key);
  }
  return keys;
}

function summarizeResourceSnapshots(): { bytes: number; detail: string } {
  const bytes = measureKeyBytes(KEY_RESOURCE_SNAPSHOTS);
  const items = readWrappedData<ResourceSnapshotItem[]>(KEY_RESOURCE_SNAPSHOTS) ?? [];
  const manual = items.filter((item) => item.source === "manual" || item.pinned).length;
  const detail =
    items.length === 0
      ? "暂无快照"
      : `${items.length} 条快照${manual > 0 ? ` · ${manual} 条手动/置顶` : ""}`;
  return { bytes, detail };
}

function summarizeManifests(): { bytes: number; detail: string } {
  const bytes = measureKeyBytes(KEY_ORCHESTRATOR_MANIFESTS);
  const items = readWrappedData<OrchestratorManifest[]>(KEY_ORCHESTRATOR_MANIFESTS) ?? [];
  const historyCount = items.reduce((sum, item) => sum + (item.history?.length ?? 0), 0);
  const detail =
    items.length === 0
      ? "暂无 Manifest"
      : `${items.length} 个 Manifest · ${historyCount} 条历史版本`;
  return { bytes, detail };
}

function summarizePackages(): { bytes: number; detail: string } {
  const bytes = measureKeyBytes(KEY_ORCHESTRATOR_PACKAGES);
  const items = readWrappedData<OrchestratorPackage[]>(KEY_ORCHESTRATOR_PACKAGES) ?? [];
  const versionCount = items.reduce((sum, item) => sum + (item.versions?.length ?? 0), 0);
  const detail =
    items.length === 0
      ? "暂无应用包"
      : `${items.length} 个应用包 · ${versionCount} 个版本`;
  return { bytes, detail };
}

function summarizeImportBatches(): { bytes: number; detail: string } {
  const bytes = measureKeyBytes(KEY_ORCHESTRATOR_BATCHES);
  const items = readWrappedData<OrchestratorImportBatch[]>(KEY_ORCHESTRATOR_BATCHES) ?? [];
  return {
    bytes,
    detail: items.length === 0 ? "暂无导入记录" : `${items.length} 条导入批次记录`,
  };
}

/** 统计 WebView localStorage 中 kube-flow 相关数据的占用。 */
export function collectLocalStorageUsage(): LocalStorageUsageReport {
  const allKeys = listKubeFlowKeys();
  const envViewKeys = allKeys.filter((key) => key.startsWith(ENV_VIEW_PREFIX));
  const accounted = new Set<string>([
    KEY_RESOURCE_SNAPSHOTS,
    KEY_ORCHESTRATOR_MANIFESTS,
    KEY_ORCHESTRATOR_BATCHES,
    KEY_ORCHESTRATOR_PACKAGES,
    ...envViewKeys,
  ]);

  const snapshots = summarizeResourceSnapshots();
  const manifests = summarizeManifests();
  const packages = summarizePackages();
  const batches = summarizeImportBatches();
  const envViewBytes = sumKeyBytes(envViewKeys);

  const preferenceKeys = allKeys.filter((key) => !accounted.has(key));
  const preferencesBytes = sumKeyBytes(preferenceKeys);

  const categories: LocalStorageCategoryUsage[] = [
    {
      id: "resource-snapshots",
      label: "资源快照",
      description: "应用 YAML、改镜像等操作保存的快照，含完整 YAML 内容。",
      bytes: snapshots.bytes,
      detail: snapshots.detail,
      manageAction: "snapshotCenter",
    },
    {
      id: "orchestrator-manifests",
      label: "编排 Manifest",
      description: "编排中心同步/编辑的资源 YAML 及历史版本。",
      bytes: manifests.bytes,
      detail: manifests.detail,
      manageAction: "orchestrator",
    },
    {
      id: "orchestrator-packages",
      label: "编排应用包",
      description: "应用包各版本打包的资源 YAML 副本。",
      bytes: packages.bytes,
      detail: packages.detail,
      manageAction: "orchestrator",
    },
    {
      id: "orchestrator-import-batches",
      label: "编排导入记录",
      description: "批量导入 YAML 的批次元数据，通常体积较小。",
      bytes: batches.bytes,
      detail: batches.detail,
      manageAction: "orchestrator",
      clearable: batches.bytes > 0,
    },
    {
      id: "env-view-state",
      label: "工作台视图状态（旧版缓存）",
      description: "迁移前的 localStorage 残留；当前数据已写入应用目录 env-view-states.json。",
      bytes: envViewBytes,
      detail: envViewKeys.length === 0 ? "无旧版缓存" : `${envViewKeys.length} 个环境的待迁移项`,
    },
    {
      id: "preferences",
      label: "界面偏好",
      description: "主题、最近使用、抽屉宽度等 UI 偏好设置。",
      bytes: preferencesBytes,
      detail: preferenceKeys.length === 0 ? "暂无额外项" : `${preferenceKeys.length} 项`,
    },
  ];

  const totalBytes = categories.reduce((sum, item) => sum + item.bytes, 0);
  return { categories, totalBytes };
}