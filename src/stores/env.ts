/**
 * 环境与「已打开环境」状态：当前选中、已打开列表，供工作台左侧栏与内容区使用。
 */
import { ref, computed } from "vue";
import type { Environment } from "../api/env";
import type { ResolvedAliasTarget } from "../api/types/kube";
import { envList, envTouch, envDelete } from "../api/env";
import { kubeRemoveClient } from "../api/kube";
import { createStorage, type Storage } from "../utils/storage";

const ENV_VIEW_STATE_KEY_PREFIX = "kube-flow:env-view";

export interface EnvViewState {
  namespace: string | null;
  kind: string;
  nameFilter: string;
  nodeFilter: string;
  podIpFilter: string;
  labelSelector: string;
  customTarget?: ResolvedAliasTarget | null;
}

const envViewStorageCache = new Map<string, Storage<EnvViewState>>();

/** 各环境工作台视图快照（与 localStorage 同步，供跨 Tab 组件 reactive 读取）。 */
export const envViewStateById = ref<Record<string, EnvViewState>>({});
function getEnvViewStorage(envId: string): Storage<EnvViewState> {
  if (!envViewStorageCache.has(envId)) {
    envViewStorageCache.set(
      envId,
      createStorage<EnvViewState>({
        key: `${ENV_VIEW_STATE_KEY_PREFIX}:${envId}`,
        version: 5,
        fallback: {
          namespace: null,
          kind: "namespaces",
          nameFilter: "",
          nodeFilter: "all",
          podIpFilter: "",
          labelSelector: "",
          customTarget: null,
        },
        migrate: (old) => {
          const o = old as {
            namespace?: string | null;
            kind?: string;
            nameFilter?: string;
            nodeFilter?: string;
            podIpFilter?: string;
            labelSelector?: string;
            customTarget?: ResolvedAliasTarget | null;
          } | null;
          return {
            namespace: o?.namespace ?? null,
            kind: typeof o?.kind === "string" ? o.kind : "namespaces",
            nameFilter: typeof o?.nameFilter === "string" ? o.nameFilter : "",
            nodeFilter: typeof o?.nodeFilter === "string" ? o.nodeFilter : "all",
            podIpFilter: typeof o?.podIpFilter === "string" ? o.podIpFilter : "",
            labelSelector: typeof o?.labelSelector === "string" ? o.labelSelector : "",
            customTarget: o?.customTarget ?? null,
          };
        },
      })
    );
  }
  return envViewStorageCache.get(envId)!;
}

function getEnvViewStateFromStorage(envId: string): EnvViewState | null {
  const stored = getEnvViewStorage(envId).read();
  return stored.kind ? stored : null;
}

function mergeEnvViewState(envId: string, state: Partial<EnvViewState>): EnvViewState {
  const existing =
    envViewStateById.value[envId] ??
    getEnvViewStateFromStorage(envId) ?? {
      namespace: null,
      kind: "namespaces",
      nameFilter: "",
      nodeFilter: "all",
      podIpFilter: "",
      labelSelector: "",
      customTarget: null,
    };
  return { ...existing, ...state };
}

function setEnvViewStateToStorage(envId: string, state: Partial<EnvViewState>) {
  const next = mergeEnvViewState(envId, state);
  getEnvViewStorage(envId).write(next);
  envViewStateById.value = { ...envViewStateById.value, [envId]: next };
}

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

export function readEnvViewState(envId: string): EnvViewState {
  return envViewStateById.value[envId] ?? getEnvViewStateFromStorage(envId) ?? defaultEnvViewState();
}

export function resetEnvViewState(envId: string): void {
  const next = defaultEnvViewState();
  getEnvViewStorage(envId).write(next);
  envViewStateById.value = { ...envViewStateById.value, [envId]: next };
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
    openedIds.value = openedIds.value.filter((x) => x !== id);
    await kubeRemoveClient(id);
    if (currentId.value === id) {
      const rest = [...openedIds.value];
      currentId.value = rest.length > 0 ? rest[0] : null;
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
