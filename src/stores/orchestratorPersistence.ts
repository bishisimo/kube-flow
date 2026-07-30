import { invoke } from "@tauri-apps/api/core";
import type {
  OrchestratorComponentMeta,
  OrchestratorImportBatch,
  OrchestratorManifest,
} from "../stores/orchestratorTypes";
import type { OrchestratorPackage } from "../stores/orchestratorPackages";

export interface OrchestratorPersistedData {
  manifests: OrchestratorManifest[];
  importBatches: OrchestratorImportBatch[];
  packages: OrchestratorPackage[];
  components: OrchestratorComponentMeta[];
}

const KEY_MANIFESTS = "kube-flow:orchestrator:manifests";
const KEY_BATCHES = "kube-flow:orchestrator:import-batches";
const KEY_PACKAGES = "kube-flow:orchestrator:packages";

function readLegacyWrapped<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw) as unknown;
    if (parsed && typeof parsed === "object" && "data" in parsed) {
      return ((parsed as { data: T }).data ?? fallback) as T;
    }
    return (parsed as T) ?? fallback;
  } catch {
    return fallback;
  }
}

function normalizeManifests(items: unknown): OrchestratorManifest[] {
  if (!Array.isArray(items)) return [];
  return items.filter((m) => m && typeof m === "object") as OrchestratorManifest[];
}

function normalizeBatches(items: unknown): OrchestratorImportBatch[] {
  if (!Array.isArray(items)) return [];
  return items.filter((m) => m && typeof m === "object") as OrchestratorImportBatch[];
}

function normalizePackages(items: unknown): OrchestratorPackage[] {
  if (!Array.isArray(items)) return [];
  return items
    .filter((p) => p && typeof p === "object")
    .map((p) => {
      const pkg = p as OrchestratorPackage;
      const versions = Array.isArray(pkg.versions)
        ? pkg.versions.map((v) => ({
            ...v,
            tag: typeof v.tag === "string" && v.tag.trim() ? v.tag.trim() : null,
          }))
        : [];
      return {
        ...pkg,
        versions,
        deployments: Array.isArray(pkg.deployments) ? pkg.deployments : [],
      } as OrchestratorPackage;
    });
}

function normalizeComponents(items: unknown): OrchestratorComponentMeta[] {
  if (!Array.isArray(items)) return [];
  return items
    .filter((c) => c && typeof c === "object")
    .map((c) => {
      const item = c as Partial<OrchestratorComponentMeta>;
      return {
        env_id: typeof item.env_id === "string" ? item.env_id : "",
        name: typeof item.name === "string" ? item.name : "",
        namespace:
          typeof item.namespace === "string" && item.namespace.trim()
            ? item.namespace.trim()
            : "default",
      };
    })
    .filter((c) => c.env_id && c.name);
}

export function readOrchestratorLegacyLocalStorage(): OrchestratorPersistedData {
  return {
    manifests: normalizeManifests(readLegacyWrapped<unknown>(KEY_MANIFESTS, [])),
    importBatches: normalizeBatches(readLegacyWrapped<unknown>(KEY_BATCHES, [])),
    packages: normalizePackages(readLegacyWrapped<unknown>(KEY_PACKAGES, [])),
    components: [],
  };
}

function hasOrchestratorContent(data: OrchestratorPersistedData): boolean {
  return (
    data.manifests.length > 0 ||
    data.importBatches.length > 0 ||
    data.packages.length > 0 ||
    data.components.length > 0
  );
}

function pickRicherOrchestratorData(
  primary: OrchestratorPersistedData,
  secondary: OrchestratorPersistedData
): OrchestratorPersistedData {
  if (!hasOrchestratorContent(secondary)) return primary;
  if (!hasOrchestratorContent(primary)) return secondary;
  const primaryScore = primary.manifests.length + primary.packages.length;
  const secondaryScore = secondary.manifests.length + secondary.packages.length;
  return secondaryScore > primaryScore ? secondary : primary;
}

async function orchestratorDataLoad(): Promise<OrchestratorPersistedData> {
  const raw = await invoke<{
    manifests?: unknown;
    importBatches?: unknown;
    packages?: unknown;
    components?: unknown;
  }>("orchestrator_data_load");
  return {
    manifests: normalizeManifests(raw.manifests),
    importBatches: normalizeBatches(raw.importBatches),
    packages: normalizePackages(raw.packages),
    components: normalizeComponents(raw.components),
  };
}

async function orchestratorDataSave(data: OrchestratorPersistedData): Promise<void> {
  await invoke("orchestrator_data_save", {
    payload: {
      manifests: data.manifests,
      importBatches: data.importBatches,
      packages: data.packages,
      components: data.components,
    },
  });
}

let hydrated = false;
let hydratePromise: Promise<void> | null = null;
let snapshotProvider: (() => OrchestratorPersistedData) | null = null;

export function registerOrchestratorPersistGetter(getter: () => OrchestratorPersistedData) {
  snapshotProvider = getter;
}

export async function hydrateOrchestratorData(
  apply: (data: OrchestratorPersistedData) => void
): Promise<void> {
  if (hydrated) return;
  if (hydratePromise) return hydratePromise;

  hydratePromise = (async () => {
    const fromDisk = await orchestratorDataLoad().catch((e) => {
      console.error("[orchestrator-data] load failed:", e);
      return {
        manifests: [],
        importBatches: [],
        packages: [],
        components: [],
      } satisfies OrchestratorPersistedData;
    });
    const legacy = readOrchestratorLegacyLocalStorage();
    const merged = pickRicherOrchestratorData(fromDisk, legacy);
    apply(merged);
    const toSave = snapshotProvider ? snapshotProvider() : merged;
    if (hasOrchestratorContent(toSave)) {
      await orchestratorDataSave(toSave).catch((e) =>
        console.error("[orchestrator-data] migrate save failed:", e)
      );
    }
    hydrated = true;
  })();

  return hydratePromise;
}

export async function ensureOrchestratorHydrated(): Promise<void> {
  if (hydrated) return;
  if (hydratePromise) await hydratePromise;
}

export async function persistOrchestratorData(data: OrchestratorPersistedData): Promise<void> {
  await ensureOrchestratorHydrated();
  try {
    await orchestratorDataSave(data);
  } catch (e) {
    console.error("[orchestrator-data] persist failed:", e);
  }
}

export function queueOrchestratorPersist() {
  if (!snapshotProvider) return;
  void persistOrchestratorData(snapshotProvider());
}

export async function flushOrchestratorData(): Promise<void> {
  if (!snapshotProvider) return;
  await persistOrchestratorData(snapshotProvider());
}
