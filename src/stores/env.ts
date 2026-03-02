/**
 * 环境与「已打开环境」状态：当前选中、已打开列表，供工作台左侧栏与内容区使用。
 */
import { ref, computed } from "vue";
import type { Environment } from "../api/env";
import { envList, envTouch, envDelete } from "../api/env";
import { kubeRemoveClient } from "../api/kube";

const ENV_VIEW_STATE_KEY = "kube-flow:env-view";

export interface EnvViewState {
  namespace: string | null;
  kind: string;
}

function getEnvViewStateFromStorage(envId: string): EnvViewState | null {
  try {
    const raw = localStorage.getItem(`${ENV_VIEW_STATE_KEY}:${envId}`);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as { namespace?: string | null; kind?: string };
    if (parsed && typeof parsed === "object") {
      return {
        namespace: parsed.namespace ?? null,
        kind: typeof parsed.kind === "string" ? parsed.kind : "namespaces",
      };
    }
  } catch {}
  return null;
}

function setEnvViewStateToStorage(envId: string, state: Partial<EnvViewState>) {
  try {
    const existing = getEnvViewStateFromStorage(envId) ?? { namespace: null, kind: "namespaces" };
    const next = { ...existing, ...state };
    localStorage.setItem(`${ENV_VIEW_STATE_KEY}:${envId}`, JSON.stringify(next));
  } catch {}
}

const environments = ref<Environment[]>([]);
/** 已打开环境的 id 列表，顺序为打开顺序，不随切换或 last_used_at 变化 */
const openedIds = ref<string[]>([]);
const currentId = ref<string | null>(null);

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

  async function touchEnv(id: string) {
    await envTouch(id);
    await loadEnvironments();
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

  function getEnvViewState(envId: string): EnvViewState | null {
    return getEnvViewStateFromStorage(envId);
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
