/**
 * EnvManage 视图编排：整合环境列表/筛选/打开/删除/终端跳转等操作，使视图层保持专注。
 */
import { ref, computed, onMounted } from "vue";
import {
  envList,
  envListSshTunnels,
  envSetCurrentContext,
  effectiveContext,
  type Environment,
  type SshTunnel,
} from "../../api/env";
import { kubeRemoveClient } from "../../api/kube";
import { useEnvStore } from "../../stores/env";
import { useShellStore } from "../../stores/shell";
import { getNodeTerminalStrategy } from "../../stores/nodeTerminalStrategy";

function envMatchesQuery(env: Environment, tunnel: SshTunnel | undefined, query: string): boolean {
  // query 已由调用方 trim + toLowerCase
  if (!query) return true;
  const haystack = [
    env.display_name,
    env.source === "ssh_tunnel" ? "ssh" : "本地",
    env.source,
    ...(env.tags ?? []),
    ...(env.contexts ?? []).flatMap((c) => [
      c.context_name,
      c.display_name ?? "",
      c.cluster_name ?? "",
      c.default_namespace ?? "",
    ]),
    env.kubeconfig_path ?? "",
    tunnel?.ssh_host ?? "",
    tunnel?.name ?? "",
    tunnel?.remote_kubeconfig_path ?? "",
  ]
    .join("\n")
    .toLowerCase();
  return haystack.includes(query);
}

export function useEnvManage() {
  const envStore = useEnvStore();
  const shellStore = useShellStore();

  const environments = ref<Environment[]>([]);
  const sshTunnels = ref<SshTunnel[]>([]);
  const listLoading = ref(false);
  const selectedFilterTags = ref<string[]>([]);
  const searchQuery = ref("");

  const allTags = computed<string[]>(() => {
    const set = new Set<string>();
    for (const env of environments.value) {
      for (const t of env.tags ?? []) if (t.trim()) set.add(t.trim());
    }
    return [...set].sort((a, b) => a.localeCompare(b));
  });

  const hasActiveFilter = computed(
    () => selectedFilterTags.value.length > 0 || Boolean(searchQuery.value.trim())
  );

  const filteredEnvironments = computed<Environment[]>(() => {
    const selected = new Set(selectedFilterTags.value);
    const query = searchQuery.value.trim().toLowerCase();
    return environments.value.filter((env) => {
      if (selected.size > 0 && !(env.tags ?? []).some((t) => selected.has(t.trim()))) {
        return false;
      }
      if (!query) return true;
      const tunnel =
        env.source === "ssh_tunnel" && env.ssh_tunnel_id
          ? sshTunnels.value.find((t) => t.id === env.ssh_tunnel_id)
          : undefined;
      return envMatchesQuery(env, tunnel, query);
    });
  });

  async function loadList() {
    listLoading.value = true;
    try {
      const [envs, tunnels] = await Promise.all([envList(), envListSshTunnels()]);
      environments.value = envs;
      sshTunnels.value = tunnels;
      await envStore.loadEnvironments();
    } finally {
      listLoading.value = false;
    }
  }

  function getTunnelForEnv(env: Environment): SshTunnel | undefined {
    if (env.source !== "ssh_tunnel" || !env.ssh_tunnel_id) return undefined;
    return sshTunnels.value.find((t) => t.id === env.ssh_tunnel_id);
  }

  function currentContextLabel(env: Environment): string {
    const name = effectiveContext(env);
    if (!name) return "—";
    const ctx = env.contexts.find((c) => c.context_name === name);
    return ctx?.cluster_name ?? name;
  }

  function strategyEnabled(envId: string): boolean {
    return Boolean(getNodeTerminalStrategy(envId)?.enabled);
  }

  function toggleFilterTag(tag: string) {
    const cur = selectedFilterTags.value;
    selectedFilterTags.value = cur.includes(tag) ? cur.filter((t) => t !== tag) : [...cur, tag];
  }

  function clearFilter() {
    selectedFilterTags.value = [];
    searchQuery.value = "";
  }

  async function switchContext(env: Environment, contextName: string) {
    try {
      await envSetCurrentContext(env.id, contextName);
      await kubeRemoveClient(env.id);
      await loadList();
    } catch (e) {
      console.error(e);
    }
  }

  async function removeEnv(id: string) {
    try {
      await envStore.removeEnv(id);
      await loadList();
    } catch (e) {
      console.error(e);
    }
  }

  function useEnvAndEmit(env: Environment, emit: () => void) {
    envStore.openEnv(env.id);
    emit();
  }

  function openEnvTerminal(env: Environment) {
    shellStore.pendingOpen.value = {
      kind: "host",
      envId: env.id,
      envName: env.display_name,
      hostLabel: `${env.display_name} 主机`,
    };
    shellStore.requestSwitchToShell();
  }

  onMounted(() => {
    void loadList();
  });

  return {
    environments,
    sshTunnels,
    listLoading,
    allTags,
    selectedFilterTags,
    searchQuery,
    hasActiveFilter,
    filteredEnvironments,
    loadList,
    getTunnelForEnv,
    currentContextLabel,
    strategyEnabled,
    toggleFilterTag,
    clearFilter,
    switchContext,
    removeEnv,
    useEnvAndEmit,
    openEnvTerminal,
  };
}
