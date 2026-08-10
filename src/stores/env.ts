/**
 * 环境与「已打开环境」状态：当前选中、已打开列表，供工作台左侧栏与内容区使用。
 */
import { ref, computed } from "vue";
import type { Environment } from "../api/env";
import type { ResolvedAliasTarget } from "../api/types/kube";
import {
  envViewStateDelete,
  envViewStateList,
  envViewStateSet,
  type EnvViewState,
} from "../api/envViewState";
import { envList, envTouch, envDelete } from "../api/env";
import { kubeRemoveClient } from "../api/kube";

export type { EnvViewState } from "../api/envViewState";

const ENV_VIEW_STATE_KEY_PREFIX = "kube-flow:env-view";

/** 各环境工作台视图快照（与 app data JSON 同步，供跨 Tab 组件 reactive 读取）。 */
export const envViewStateById = ref<Record<string, EnvViewState>>({});

let hydrated = false;
let hydratePromise: Promise<void> | null = null;

function defaultEnvViewState(): EnvViewState {
  return {
    namespace: null,
    kind: "namespaces",
    nameFilter: "",
    nodeFilter: "all",
    podIpFilter: "",
    labelSelector: "",
    customTarget: null,
  };
}

/** 从旧版 localStorage 读取单环境视图状态（仅用于一次性迁移）。 */
function readLegacyEnvViewStateFromLocalStorage(envId: string): EnvViewState | null {
  try {
    const raw = localStorage.getItem(`${ENV_VIEW_STATE_KEY_PREFIX}:${envId}`);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as { v?: number; data?: unknown } | EnvViewState;
    const data =
      parsed && typeof parsed === "object" && "data" in parsed && parsed.data
        ? (parsed as { data: EnvViewState }).data
        : (parsed as EnvViewState);
    if (!data || typeof data !== "object" || typeof data.kind !== "string" || !data.kind) {
      return null;
    }
    return {
      namespace: data.namespace ?? null,
      kind: data.kind,
      nameFilter: typeof data.nameFilter === "string" ? data.nameFilter : "",
      nodeFilter: typeof data.nodeFilter === "string" ? data.nodeFilter : "all",
      podIpFilter: typeof data.podIpFilter === "string" ? data.podIpFilter : "",
      labelSelector: typeof data.labelSelector === "string" ? data.labelSelector : "",
      customTarget: data.customTarget ?? null,
    };
  } catch {
    return null;
  }
}

function listLegacyEnvViewStateEnvIds(): string[] {
  const prefix = `${ENV_VIEW_STATE_KEY_PREFIX}:`;
  const ids: string[] = [];
  for (let i = 0; i < localStorage.length; i++) {
    const key = localStorage.key(i);
    if (key?.startsWith(prefix)) ids.push(key.slice(prefix.length));
  }
  return ids;
}

function mergeEnvViewState(envId: string, state: Partial<EnvViewState>): EnvViewState {
  const existing = envViewStateById.value[envId] ?? defaultEnvViewState();
  return { ...existing, ...state };
}

async function persistEnvViewState(envId: string, state: EnvViewState): Promise<void> {
  try {
    await envViewStateSet(envId, state);
  } catch (e) {
    console.error("[env-view-state] persist failed:", e);
  }
}

function setEnvViewStateToStorage(envId: string, state: Partial<EnvViewState>) {
  const next = mergeEnvViewState(envId, state);
  envViewStateById.value = { ...envViewStateById.value, [envId]: next };
  void persistEnvViewState(envId, next);
}

/**
 * 从 app data 目录加载各环境工作台视图状态；启动时调用一次。
 * 若磁盘无数据，会尝试从旧版 localStorage 迁移并写回磁盘。
 */
export async function hydrateEnvViewStates(): Promise<void> {
  if (hydrated) return;
  if (hydratePromise) return hydratePromise;

  hydratePromise = (async () => {
    const fromDisk = await envViewStateList().catch((e) => {
      console.error("[env-view-state] load failed:", e);
      return {} as Record<string, EnvViewState>;
    });

    const merged: Record<string, EnvViewState> = { ...fromDisk };
    const legacyEnvIds = new Set([
      ...Object.keys(fromDisk),
      ...listLegacyEnvViewStateEnvIds(),
    ]);

    for (const envId of legacyEnvIds) {
      const legacy = readLegacyEnvViewStateFromLocalStorage(envId);
      if (!legacy) continue;
      const disk = fromDisk[envId];
      const shouldMigrate = !disk || (disk.kind === "namespaces" && legacy.kind !== "namespaces");
      if (shouldMigrate) {
        merged[envId] = legacy;
        await persistEnvViewState(envId, legacy);
      }
    }

    envViewStateById.value = merged;
    hydrated = true;
  })();

  return hydratePromise;
}

export async function ensureEnvViewStatesHydrated(): Promise<void> {
  return hydrateEnvViewStates();
}

/** 退出或切环境前确保指定环境的视图状态已写入磁盘。 */
export async function flushEnvViewState(envId: string): Promise<void> {
  await ensureEnvViewStatesHydrated();
  const state = envViewStateById.value[envId];
  if (!state) return;
  await persistEnvViewState(envId, state);
}

export function readEnvViewState(envId: string): EnvViewState {
  return envViewStateById.value[envId] ?? defaultEnvViewState();
}

export function resetEnvViewState(envId: string): void {
  const next = defaultEnvViewState();
  setEnvViewStateToStorage(envId, next);
}

const environments = ref<Environment[]>([]);
/** 已打开环境的 id 列表，顺序为打开顺序，不随切换或 last_used_at 变化 */
const openedIds = ref<string[]>([]);
const currentId = ref<string | null>(null);

/**
 * 工作台跨组件导航请求：由命令面板等外部入口写入，Main.vue 监听并调用 navigateTo。
 * 若 envId 与当前不同，会先切换环境。写入后由消费方清空。
 */
export const workbenchPendingNav = ref<{
  envId?: string;
  kind?: string;
  namespace?: string | null;
  nameFilter?: string;
  customTarget?: ResolvedAliasTarget | null;
  /** 为 true 时 Main 在导航后聚焦资源列表并启用键盘选行（由命令面板 ⌘Enter 提交导航触发） */
  focusResourceList?: boolean;
} | null>(null);

/**
 * 各环境的命名空间列表快照：由工作台在拉取后同步写入，命令面板等外部消费方只读。
 * 仅在工作台访问过该环境之后才有值；未访问过的环境此处为 undefined。
 */
export const namespacesByEnv = ref<Record<string, string[]>>({});

export function setEnvNamespaces(envId: string, names: string[]) {
  namespacesByEnv.value = { ...namespacesByEnv.value, [envId]: names };
}

export function useEnvStore() {
  const openedEnvs = computed(() =>
    openedIds.value
      .map((id) => environments.value.find((e) => e.id === id))
      .filter((e): e is Environment => e != null)
  );
  const currentEnv = computed(
    () => (currentId.value ? environments.value.find((e) => e.id === currentId.value) ?? null : null)
  );

  async function loadEnvironments() {
    environments.value = await envList();
  }

  function openEnv(id: string) {
    if (!openedIds.value.includes(id)) {
      openedIds.value = [...openedIds.value, id];
    }
    currentId.value = id;
  }

  function setCurrent(id: string) {
    if (openedIds.value.includes(id)) currentId.value = id;
  }

  async function closeEnv(id: string) {
    // 先同步切走 UI 状态，再后台拆客户端；避免 kubeRemoveClient 卡住时侧栏关不掉。
    openedIds.value = openedIds.value.filter((x) => x !== id);
    if (currentId.value === id) {
      const rest = [...openedIds.value];
      currentId.value = rest.length > 0 ? rest[0] : null;
    }
    try {
      await kubeRemoveClient(id);
    } catch (e) {
      console.warn("[env] closeEnv remove client failed:", e);
    }
  }

  function touchEnv(id: string): void {
    void envTouch(id);
  }

  async function removeEnv(id: string) {
    await kubeRemoveClient(id);
    await envDelete(id);
    openedIds.value = openedIds.value.filter((x) => x !== id);
    if (currentId.value === id) {
      const rest = [...openedIds.value];
      currentId.value = rest.length > 0 ? rest[0] : null;
    }
    await envViewStateDelete(id).catch((e) => console.warn("[env-view-state] delete failed:", e));
    delete envViewStateById.value[id];
    envViewStateById.value = { ...envViewStateById.value };
    await loadEnvironments();
  }

  function getEnvViewState(envId: string): EnvViewState {
    return readEnvViewState(envId);
  }

  function setEnvViewState(envId: string, state: Partial<EnvViewState>) {
    setEnvViewStateToStorage(envId, state);
  }

  return {
    environments,
    openedIds,
    currentId,
    openedEnvs,
    currentEnv,
    loadEnvironments,
    openEnv,
    setCurrent,
    closeEnv,
    touchEnv,
    removeEnv,
    getEnvViewState,
    setEnvViewState,
  };
}
