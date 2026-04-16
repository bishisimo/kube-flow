import { computed, onUnmounted, ref, type Ref } from "vue";
import { kubeRefreshResourceAliases, kubeResolveResourceAlias, type ResolvedAliasTarget } from "../../../api/kube";
import type { EnvConnectionState } from "../../../stores/connection";
import { extractErrorMessage } from "../utils/extractErrorMessage";

type UseWorkbenchCustomResourceOptions = {
  currentId: Ref<string | null>;
  kindFilter: Ref<string>;
  getState: (envId: string) => EnvConnectionState;
};

/**
 * 工作台资源类型搜索中的 CRD 智能解析。
 * 负责 API 发现同步、输入防抖解析、提示文案和状态样式。
 */
export function useWorkbenchCustomResource(options: UseWorkbenchCustomResourceOptions) {
  const aliasDiscoveryForEnvId = ref<string | null>(null);
  const aliasDiscoveryLoading = ref(false);
  const aliasDiscoveryError = ref<string | null>(null);
  const customResourceQuery = ref("");
  const customResourceResolving = ref(false);
  const customResourceHits = ref<ResolvedAliasTarget[]>([]);
  const customResourceResolveMessage = ref("");

  let customResourceResolveTimer: number | null = null;

  function clearResolveTimer() {
    if (customResourceResolveTimer != null) {
      clearTimeout(customResourceResolveTimer);
      customResourceResolveTimer = null;
    }
  }

  function resetCustomResourceInline() {
    customResourceQuery.value = options.kindFilter.value.trim();
    customResourceHits.value = [];
    customResourceResolveMessage.value = "";
    clearResolveTimer();
    customResourceResolving.value = false;
  }

  async function runCustomResourceResolve() {
    const q = options.kindFilter.value.trim();
    customResourceQuery.value = q;
    const id = options.currentId.value;
    if (!q) {
      customResourceHits.value = [];
      customResourceResolveMessage.value = "";
      customResourceResolving.value = false;
      return;
    }
    if (!id || options.getState(id) !== "connected") {
      customResourceHits.value = [];
      customResourceResolveMessage.value = "请先连接环境";
      return;
    }
    if (aliasDiscoveryLoading.value) {
      customResourceResolveMessage.value = "正在同步 API 发现…";
      return;
    }
    if (aliasDiscoveryForEnvId.value !== id) {
      customResourceResolveMessage.value =
        aliasDiscoveryError.value || "发现未就绪，请稍候或点击刷新重连";
      return;
    }
    customResourceResolving.value = true;
    try {
      const hits = await kubeResolveResourceAlias(id, q, null);
      customResourceHits.value = hits;
      if (hits.length === 0) {
        customResourceResolveMessage.value = "无匹配，请检查拼写或是否已安装对应 CRD";
      } else if (hits.length === 1) {
        const h = hits[0]!;
        customResourceResolveMessage.value = `→ ${h.api_version} ${h.kind} · ${h.plural} · ${
          h.namespaced ? "命名空间资源" : "集群资源"
        }`;
      } else {
        customResourceResolveMessage.value = `多项匹配（${hits.length}），请输入更精确的短名、Kind 或 plural`;
      }
    } catch (e) {
      customResourceHits.value = [];
      customResourceResolveMessage.value = extractErrorMessage(e);
    } finally {
      customResourceResolving.value = false;
    }
  }

  function scheduleCustomResourceResolve(immediate = false) {
    const run = () => {
      void runCustomResourceResolve();
    };
    clearResolveTimer();
    if (immediate) {
      run();
      return;
    }
    customResourceResolveTimer = window.setTimeout(() => {
      customResourceResolveTimer = null;
      run();
    }, 340);
  }

  async function primeAliasDiscoveryForCurrentEnv() {
    const id = options.currentId.value;
    if (!id || options.getState(id) !== "connected") return;
    if (aliasDiscoveryForEnvId.value === id && !aliasDiscoveryError.value) {
      scheduleCustomResourceResolve(true);
      return;
    }
    aliasDiscoveryLoading.value = true;
    aliasDiscoveryError.value = null;
    try {
      await kubeRefreshResourceAliases(id);
      aliasDiscoveryForEnvId.value = id;
      scheduleCustomResourceResolve(true);
    } catch (e) {
      aliasDiscoveryError.value = extractErrorMessage(e);
      aliasDiscoveryForEnvId.value = null;
    } finally {
      aliasDiscoveryLoading.value = false;
    }
  }

  const customResourceHintLine = computed(() => {
    if (customResourceResolveMessage.value) return customResourceResolveMessage.value;
    const id = options.currentId.value;
    if (!id) return "";
    const state = options.getState(id);
    if (state === "connecting") return "连接中…";
    if (state !== "connected") return "";
    if (aliasDiscoveryLoading.value) return "正在同步 API 发现…";
    if (aliasDiscoveryError.value) return `发现同步失败：${aliasDiscoveryError.value}`;
    if (aliasDiscoveryForEnvId.value === id) return "已同步 API 发现，输入即可解析";
    return "";
  });

  const customResourceStatusClass = computed(() => {
    if (aliasDiscoveryLoading.value || customResourceResolving.value) return "toolbar-cr-hint loading";
    const line = customResourceHintLine.value;
    if (!line) return "toolbar-cr-hint muted";
    if (line.startsWith("→")) return "toolbar-cr-hint ok";
    if (line.includes("多项匹配") || line.includes("无匹配")) return "toolbar-cr-hint warn";
    if (
      line.includes("失败") ||
      line.includes("未连接") ||
      line.includes("断开") ||
      line.includes("未就绪")
    ) {
      return "toolbar-cr-hint err";
    }
    if (line.startsWith("已同步")) return "toolbar-cr-hint muted";
    if (line === "连接中…" || line.includes("正在同步")) return "toolbar-cr-hint loading";
    return "toolbar-cr-hint muted";
  });

  onUnmounted(() => {
    clearResolveTimer();
  });

  return {
    aliasDiscoveryForEnvId,
    aliasDiscoveryLoading,
    aliasDiscoveryError,
    customResourceQuery,
    customResourceResolving,
    customResourceHits,
    customResourceResolveMessage,
    customResourceHintLine,
    customResourceStatusClass,
    resetCustomResourceInline,
    scheduleCustomResourceResolve,
    primeAliasDiscoveryForCurrentEnv,
  };
}
