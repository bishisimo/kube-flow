/**
 * 命令面板的动态候选项：从 env/shell/logCenter 等 store 派生命令条目，
 * 以及 `@env` TokenSpec + "针对已选环境的动作列表" 这一两段式交互所需的
 * executor / provider 组合。具体注册/注销由 AppShell 统一管理。
 */
import { computed, type ComputedRef } from "vue";
import type { CommandItem, TokenSpec, TokenValueCandidate } from "./types";
import { useCommandPalette } from "./useCommandPalette";
import { fuzzyMatch } from "./fuzzy";
import { useEnvStore, workbenchPendingNav, namespacesByEnv } from "../../stores/env";
import { useShellStore } from "../../stores/shell";
import { useLogCenterStore } from "../../stores/logCenter";
import { RESOURCE_KINDS_FLAT, RESOURCE_GROUPS } from "../../constants/resourceKinds";

export interface WorkbenchBridge {
  /** 外部设置的目标 tab（AppShell 注入） */
  setTab: (tab: "env" | "main" | "orchestrator" | "shell" | "settings" | "logCenter") => void;
}

/* ---------------- 工具：读取 @env token ---------------- */

function useEnvTokenValue(): ComputedRef<string | null> {
  const { tokens } = useCommandPalette();
  return computed(
    () => tokens.value.find((t) => t.symbol === "@" && t.key === "env")?.value.raw ?? null,
  );
}

function useTermTokenValue(): ComputedRef<string | null> {
  const { tokens } = useCommandPalette();
  return computed(
    () => tokens.value.find((t) => t.symbol === "@" && t.key === "term")?.value.raw ?? null,
  );
}

function useLogTokenValue(): ComputedRef<string | null> {
  const { tokens } = useCommandPalette();
  return computed(
    () => tokens.value.find((t) => t.symbol === "@" && t.key === "log")?.value.raw ?? null,
  );
}

function fuzzyFilter<T>(query: string, items: T[], getText: (t: T) => string, max = 120): T[] {
  const q = query.trim();
  if (!q) return items.slice(0, max);
  const scored: Array<{ item: T; score: number }> = [];
  for (const item of items) {
    const m = fuzzyMatch(q, getText(item));
    if (m.score > 0) scored.push({ item, score: m.score });
  }
  scored.sort((a, b) => b.score - a.score);
  return scored.slice(0, max).map((x) => x.item);
}

/* ---------------- 命令候选 Provider ---------------- */

/**
 * 已打开环境的"切换"命令候选。
 * 当 tokens 中已含 `@env` 时隐藏——此刻候选列表由 env 动作 provider 接管。
 */
export function createEnvSwitchProvider(bridge: WorkbenchBridge) {
  const { openedEnvs, currentId, setCurrent } = useEnvStore();
  const envToken = useEnvTokenValue();
  return computed<CommandItem[]>(() => {
    if (envToken.value) return [];
    return openedEnvs.value.map((env) => ({
      id: `env:${env.id}`,
      title: env.display_name,
      subtitle:
        env.source === "ssh_tunnel"
          ? "SSH 环境"
          : env.current_context ?? env.contexts[0]?.context_name ?? "本地 kubeconfig",
      hint: currentId.value === env.id ? "当前" : undefined,
      section: "已打开环境",
      domain: "env:opened",
      category: "nav",
      icon: "🌐",
      keywords: [env.display_name, ...(env.tags ?? []), env.source],
      pinned: currentId.value === env.id,
      weight: currentId.value === env.id ? 2 : 8,
      order: currentId.value === env.id ? 0 : 10,
      run: () => {
        setCurrent(env.id);
        bridge.setTab("main");
      },
    }));
  });
}

/**
 * 尚未打开环境的"打开"命令候选：从全量 environments 中排除已打开的，
 * 执行时加入打开列表、置为当前环境，并切到工作台。
 */
export function createEnvOpenProvider(bridge: WorkbenchBridge) {
  const { environments, openedIds, openEnv } = useEnvStore();
  const envToken = useEnvTokenValue();
  return computed<CommandItem[]>(() => {
    if (envToken.value) return [];
    const opened = new Set(openedIds.value);
    return environments.value
      .filter((env) => !opened.has(env.id))
      .map((env) => ({
        id: `env:open:${env.id}`,
        title: `打开 ${env.display_name}`,
        subtitle:
          env.source === "ssh_tunnel"
            ? "SSH 环境"
            : env.current_context ?? env.contexts[0]?.context_name ?? "本地 kubeconfig",
        section: "可打开环境",
        domain: "env:closed",
        category: "action" as const,
        icon: "＋",
        keywords: [env.display_name, "open", "打开", "connect", "连接", ...(env.tags ?? []), env.source],
        weight: 7,
        order: 10,
        run: () => {
          openEnv(env.id);
          bridge.setTab("main");
        },
      }));
  });
}

/**
 * 基于 tokens 中已落下的 `@env=<id>` 生成"针对该环境的动作"候选集合。
 * 触发后命令面板的自由文本区可直接对这组动作做模糊搜索。
 */
export function createEnvTokenActionsProvider(bridge: WorkbenchBridge) {
  const { environments, openedIds, openEnv, setCurrent, closeEnv, currentId } = useEnvStore();
  const envToken = useEnvTokenValue();
  return computed<CommandItem[]>(() => {
    const envId = envToken.value;
    if (!envId) return [];
    const env = environments.value.find((e) => e.id === envId);
    if (!env) return [];
    const isOpen = openedIds.value.includes(env.id);
    const isCurrent = currentId.value === env.id;
    const out: CommandItem[] = [];

    if (isOpen) {
      out.push({
        id: `env:act:workbench:${env.id}`,
        title: isCurrent ? `前往 ${env.display_name} 的工作台` : `切换到 ${env.display_name} 的工作台`,
        section: "环境动作",
        domain: "env:actions",
        category: "nav",
        icon: "🗂️",
        keywords: ["workbench", "工作台", "switch", "切换"],
        weight: 50,
        order: 0,
        run: () => {
          setCurrent(env.id);
          bridge.setTab("main");
        },
      });
    } else {
      out.push({
        id: `env:act:open:${env.id}`,
        title: `打开 ${env.display_name} 的工作台`,
        section: "环境动作",
        domain: "env:actions",
        category: "action",
        icon: "＋",
        keywords: ["open", "打开", "connect", "连接"],
        weight: 50,
        order: 0,
        run: () => {
          openEnv(env.id);
          bridge.setTab("main");
        },
      });
    }

    out.push({
      id: `env:act:host-terminal:${env.id}`,
      title: `打开 ${env.display_name} 的主机终端`,
      section: "环境动作",
      domain: "env:actions",
      category: "action",
      icon: "🖥️",
      keywords: ["terminal", "host", "shell", "终端", "主机"],
      weight: 45,
      order: 10,
      run: () => {
        useShellStore().pendingOpen.value = {
          kind: "host",
          envId: env.id,
          envName: env.display_name,
          hostLabel: `${env.display_name} 主机`,
        };
        bridge.setTab("shell");
      },
    });

    out.push({
      id: `env:act:orchestrator:${env.id}`,
      title: `前往 ${env.display_name} 的编排中心`,
      section: "环境动作",
      domain: "env:actions",
      category: "nav",
      icon: "🧩",
      keywords: ["orchestrator", "编排", "manifest"],
      weight: 42,
      order: 20,
      run: () => {
        if (!isOpen) openEnv(env.id);
        else setCurrent(env.id);
        bridge.setTab("orchestrator");
      },
    });

    out.push({
      id: `env:act:logs:${env.id}`,
      title: `前往 ${env.display_name} 的日志中心`,
      section: "环境动作",
      domain: "env:actions",
      category: "nav",
      icon: "📜",
      keywords: ["log", "logs", "日志"],
      weight: 42,
      order: 30,
      run: () => {
        if (!isOpen) openEnv(env.id);
        else setCurrent(env.id);
        bridge.setTab("logCenter");
      },
    });

    if (isOpen) {
      out.push({
        id: `env:act:close:${env.id}`,
        title: `关闭 ${env.display_name}`,
        subtitle: "从已打开列表移除",
        section: "环境动作",
        domain: "env:actions",
        category: "action",
        icon: "✕",
        keywords: ["close", "关闭", "断开", "disconnect"],
        weight: 30,
        order: 40,
        run: () => {
          void closeEnv(env.id);
        },
      });
    }

    return out;
  });
}

/** 资源类型切换命令（@），仅在工作台上下文加权。 */
export function createKindSwitchProvider(bridge: WorkbenchBridge) {
  const { currentId } = useEnvStore();
  return computed<CommandItem[]>(() => {
    if (!currentId.value) return [];
    const envId = currentId.value;
    const groupLabel = new Map<string, string>();
    for (const g of RESOURCE_GROUPS) {
      for (const k of g.kinds) groupLabel.set(k.id, g.label);
    }
    return RESOURCE_KINDS_FLAT.map((k) => ({
      id: `kind:${k.id}`,
      title: k.label,
      subtitle: groupLabel.get(k.id) ?? "",
      hint: k.id,
      section: "资源类型",
      domain: "kind:builtin",
      category: "nav",
      icon: "📦",
      keywords: [k.id, k.label],
      context: "main",
      weight: 5,
      order: 20,
      run: () => {
        workbenchPendingNav.value = { envId, kind: k.id, focusResourceList: true };
        bridge.setTab("main");
      },
    }));
  });
}

/** 命名空间切换命令（@），仅在当前环境已拉取过命名空间列表时出现。 */
export function createNamespaceSwitchProvider(bridge: WorkbenchBridge) {
  const { currentId } = useEnvStore();
  return computed<CommandItem[]>(() => {
    const envId = currentId.value;
    if (!envId) return [];
    const names = namespacesByEnv.value[envId] ?? [];
    const out: CommandItem[] = [];
    out.push({
      id: "ns:__all__",
      title: "所有命名空间",
      subtitle: "清除命名空间筛选",
      section: "命名空间",
      domain: "ns:all",
      category: "nav",
      icon: "∀",
      keywords: ["all", "全部", "*"],
      context: "main",
      weight: 4,
      order: 0,
      run: () => {
        workbenchPendingNav.value = { envId, namespace: null, focusResourceList: true };
        bridge.setTab("main");
      },
    });
    for (const name of names) {
      out.push({
        id: `ns:${envId}:${name}`,
        title: name,
        section: "命名空间",
        domain: "ns:list",
        category: "nav",
        icon: "◈",
        keywords: [name],
        context: "main",
        weight: 3,
        order: 10,
        run: () => {
          workbenchPendingNav.value = { envId, namespace: name, focusResourceList: true };
          bridge.setTab("main");
        },
      });
    }
    return out;
  });
}

/** 终端会话（#），对现有会话聚焦。 */
export function createShellSessionProvider(bridge: WorkbenchBridge) {
  const { sessions, setCurrent, currentSessionId, removeSession } = useShellStore();
  const termToken = useTermTokenValue();
  return computed<CommandItem[]>(() => {
    if (termToken.value) return [];
    const out: CommandItem[] = [];
    for (const s of sessions.value) {
      const label =
        s.kind === "host"
          ? s.hostLabel || `${s.envName} 主机`
          : `${s.podName ?? "Pod"}${s.container ? ` / ${s.container}` : ""}`;
      const ns = s.namespace ? `${s.namespace}` : "";
      out.push({
        id: `shell:focus:${s.id}`,
        title: label,
        subtitle: [s.envName, ns].filter(Boolean).join(" · "),
        hint: currentSessionId.value === s.id ? "当前" : s.status,
        section: s.kind === "host" ? "主机终端" : "Pod 终端",
        domain: s.kind === "host" ? "shell:host" : "shell:pod",
        category: "session",
        icon: s.kind === "host" ? "🖥️" : "⬢",
        keywords: [label, s.envName, s.kind, s.namespace ?? ""],
        run: () => {
          setCurrent(s.id);
          bridge.setTab("shell");
        },
      });
      out.push({
        id: `shell:close:${s.id}`,
        title: `关闭终端：${label}`,
        subtitle: s.envName,
        section: "终端操作",
        domain: "shell:actions",
        category: "action",
        icon: "✕",
        keywords: ["close", "关闭", "kill"],
        weight: -4,
        order: 20,
        run: () => removeSession(s.id),
      });
    }
    return out;
  });
}

/** 日志会话（#）。 */
export function createLogSessionProvider(bridge: WorkbenchBridge) {
  const { sessions, setCurrentSession, currentSessionId, closeSession } = useLogCenterStore();
  const logToken = useLogTokenValue();
  return computed<CommandItem[]>(() => {
    if (logToken.value) return [];
    const out: CommandItem[] = [];
    for (const s of sessions.value) {
      const label = s.kind === "pod" ? s.podName ?? "Pod" : `${s.workloadKind}/${s.workloadName}`;
      out.push({
        id: `log:focus:${s.id}`,
        title: label,
        subtitle: `${s.envName} · ${s.namespace}`,
        hint: currentSessionId.value === s.id ? "当前" : undefined,
        section: "日志会话",
        domain: s.kind === "pod" ? "log:pod" : "log:workload",
        category: "session",
        icon: "📜",
        keywords: [label, s.envName, s.namespace],
        run: () => {
          setCurrentSession(s.id);
          bridge.setTab("logCenter");
        },
      });
      out.push({
        id: `log:close:${s.id}`,
        title: `关闭日志：${label}`,
        subtitle: s.envName,
        section: "日志操作",
        domain: "log:actions",
        category: "action",
        icon: "✕",
        weight: -4,
        order: 20,
        keywords: ["close", "关闭"],
        run: () => closeSession(s.id),
      });
    }
    return out;
  });
}

/* ---------------- @env TokenSpec ---------------- */

/**
 * `@env <name>` 的 TokenSpec。全局生效，不限 tab。
 * value 候选按"已打开 / 可打开"两个 section 区分展示，当前环境会被提到已打开组首位。
 */
export function buildEnvTokenSpec(): TokenSpec {
  const { environments, openedIds, currentId } = useEnvStore();

  return {
    symbol: "@",
    key: "env",
    label: "环境",
    hint: "选中一个环境，面板候选将切换为该环境的动作",
    icon: "🌐",
    domain: "token-key:scope",
    weight: 12,
    values: (query) => {
      const opened = new Set(openedIds.value);
      const openedList: TokenValueCandidate[] = [];
      const closedList: TokenValueCandidate[] = [];

      for (const env of environments.value) {
        const isOpen = opened.has(env.id);
        const isCurrent = currentId.value === env.id;
        const subtitle =
          env.source === "ssh_tunnel"
            ? "SSH 环境"
            : env.current_context ?? env.contexts[0]?.context_name ?? "本地 kubeconfig";
        const v: TokenValueCandidate = {
          value: env.id,
          title: env.display_name,
          subtitle,
          hint: isCurrent ? "当前" : isOpen ? "已打开" : undefined,
          icon: "🌐",
          domain: isOpen ? "env:opened" : "env:closed",
          section: isOpen ? "已打开环境" : "可打开环境",
          pinned: isCurrent,
          order: isCurrent ? 0 : isOpen ? 10 : 20,
          keywords: [env.display_name, ...(env.tags ?? []), env.source, env.id],
        };
        if (isOpen) openedList.push(v);
        else closedList.push(v);
      }

      const curIdx = openedList.findIndex((v) => v.value === currentId.value);
      if (curIdx > 0) {
        const [cur] = openedList.splice(curIdx, 1);
        openedList.unshift(cur);
      }

      const opened2 = fuzzyFilter(
        query,
        openedList,
        (x) => `${x.title} ${(x.keywords ?? []).join(" ")}`,
      );
      const closed2 = fuzzyFilter(
        query,
        closedList,
        (x) => `${x.title} ${(x.keywords ?? []).join(" ")}`,
      );
      return [...opened2, ...closed2];
    },
    resolveValue: (raw) => {
      const env = environments.value.find((item) => item.id === raw);
      if (!env) return null;
      const isOpen = openedIds.value.includes(env.id);
      const isCurrent = currentId.value === env.id;
      return {
        value: env.id,
        title: env.display_name,
        subtitle:
          env.source === "ssh_tunnel"
            ? "SSH 环境"
            : env.current_context ?? env.contexts[0]?.context_name ?? "本地 kubeconfig",
        hint: isCurrent ? "当前" : isOpen ? "已打开" : undefined,
        icon: "🌐",
        domain: isOpen ? "env:opened" : "env:closed",
        section: isOpen ? "已打开环境" : "可打开环境",
        pinned: isCurrent,
        order: isCurrent ? 0 : isOpen ? 10 : 20,
      };
    },
  };
}

/* ---------------- @term TokenSpec + 动作 provider ---------------- */

function shellSessionLabel(s: ReturnType<typeof useShellStore>["sessions"]["value"][number]): string {
  return s.kind === "host"
    ? s.hostLabel || `${s.envName} 主机`
    : `${s.podName ?? "Pod"}${s.container ? ` / ${s.container}` : ""}`;
}

/**
 * `@term <name>` 的 TokenSpec：全局生效，列出所有终端会话。
 * value 候选按"主机终端 / Pod 终端"分组，当前会话置顶。
 */
export function buildTermTokenSpec(): TokenSpec {
  const { sessions, currentSessionId } = useShellStore();

  return {
    symbol: "@",
    key: "term",
    label: "终端会话",
    hint: "选中一个终端会话，面板候选将切换为该会话的动作",
    icon: "🖥️",
    domain: "token-key:scope",
    weight: 10,
    values: (query) => {
      const hostList: TokenValueCandidate[] = [];
      const podList: TokenValueCandidate[] = [];

      for (const s of sessions.value) {
        const label = shellSessionLabel(s);
        const isCurrent = currentSessionId.value === s.id;
        const ns = s.namespace ? s.namespace : "";
        const v: TokenValueCandidate = {
          value: s.id,
          title: label,
          subtitle: [s.envName, ns].filter(Boolean).join(" · "),
          hint: isCurrent ? "当前" : s.status,
          icon: s.kind === "host" ? "🖥️" : "⬢",
          domain: s.kind === "host" ? "shell:host" : "shell:pod",
          section: s.kind === "host" ? "主机终端" : "Pod 终端",
          pinned: isCurrent,
          order: isCurrent ? 0 : 10,
          keywords: [label, s.envName, s.kind, s.namespace ?? "", s.podName ?? "", s.container ?? ""],
        };
        if (s.kind === "host") hostList.push(v);
        else podList.push(v);
      }

      for (const list of [hostList, podList]) {
        const idx = list.findIndex((v) => v.value === currentSessionId.value);
        if (idx > 0) {
          const [cur] = list.splice(idx, 1);
          list.unshift(cur);
        }
      }

      const host2 = fuzzyFilter(query, hostList, (x) => `${x.title} ${x.subtitle ?? ""}`);
      const pod2 = fuzzyFilter(query, podList, (x) => `${x.title} ${x.subtitle ?? ""}`);
      return [...host2, ...pod2];
    },
    resolveValue: (raw) => {
      const s = sessions.value.find((item) => item.id === raw);
      if (!s) return null;
      const label = shellSessionLabel(s);
      const isCurrent = currentSessionId.value === s.id;
      return {
        value: s.id,
        title: label,
        subtitle: [s.envName, s.namespace ?? ""].filter(Boolean).join(" · "),
        hint: isCurrent ? "当前" : s.status,
        icon: s.kind === "host" ? "🖥️" : "⬢",
        domain: s.kind === "host" ? "shell:host" : "shell:pod",
        section: s.kind === "host" ? "主机终端" : "Pod 终端",
        pinned: isCurrent,
        order: isCurrent ? 0 : 10,
      };
    },
  };
}

/**
 * 基于 tokens 中 `@term=<id>` 生成"针对该终端会话的动作"候选。
 */
export function createTermTokenActionsProvider(bridge: WorkbenchBridge) {
  const { sessions, setCurrent, removeSession, currentSessionId } = useShellStore();
  const termToken = useTermTokenValue();
  return computed<CommandItem[]>(() => {
    const sid = termToken.value;
    if (!sid) return [];
    const s = sessions.value.find((x) => x.id === sid);
    if (!s) return [];
    const label = shellSessionLabel(s);
    const isCurrent = currentSessionId.value === s.id;
    const out: CommandItem[] = [];

    out.push({
      id: `term:act:focus:${s.id}`,
      title: isCurrent ? `前往终端：${label}` : `切换到终端：${label}`,
      subtitle: s.envName,
      section: "终端动作",
      domain: "shell:actions",
      category: "nav",
      icon: s.kind === "host" ? "🖥️" : "⬢",
      keywords: ["focus", "switch", "切换", "聚焦", "open", "前往"],
      weight: 50,
      order: 0,
      run: () => {
        setCurrent(s.id);
        bridge.setTab("shell");
      },
    });

    out.push({
      id: `term:act:close:${s.id}`,
      title: `关闭终端：${label}`,
      subtitle: s.envName,
      section: "终端动作",
      domain: "shell:actions",
      category: "action",
      icon: "✕",
      keywords: ["close", "关闭", "kill"],
      weight: 20,
      order: 10,
      run: () => removeSession(s.id),
    });

    return out;
  });
}

/* ---------------- @log TokenSpec + 动作 provider ---------------- */

function logSessionLabel(
  s: ReturnType<typeof useLogCenterStore>["sessions"]["value"][number],
): string {
  return s.kind === "pod" ? s.podName ?? "Pod" : `${s.workloadKind}/${s.workloadName}`;
}

/**
 * `@log <name>` 的 TokenSpec：全局生效，列出所有日志会话。
 * value 候选按"Pod 日志 / Workload 日志"分组，当前会话置顶。
 */
export function buildLogTokenSpec(): TokenSpec {
  const { sessions, currentSessionId } = useLogCenterStore();

  return {
    symbol: "@",
    key: "log",
    label: "日志会话",
    hint: "选中一个日志会话，面板候选将切换为该会话的动作",
    icon: "📜",
    domain: "token-key:scope",
    weight: 9,
    values: (query) => {
      const podList: TokenValueCandidate[] = [];
      const wlList: TokenValueCandidate[] = [];

      for (const s of sessions.value) {
        const label = logSessionLabel(s);
        const isCurrent = currentSessionId.value === s.id;
        const v: TokenValueCandidate = {
          value: s.id,
          title: label,
          subtitle: `${s.envName} · ${s.namespace}`,
          hint: isCurrent ? "当前" : undefined,
          icon: "📜",
          domain: s.kind === "pod" ? "log:pod" : "log:workload",
          section: s.kind === "pod" ? "Pod 日志" : "Workload 日志",
          pinned: isCurrent,
          order: isCurrent ? 0 : 10,
          keywords: [label, s.envName, s.namespace, s.podName ?? "", s.workloadKind ?? "", s.workloadName ?? ""],
        };
        if (s.kind === "pod") podList.push(v);
        else wlList.push(v);
      }

      for (const list of [podList, wlList]) {
        const idx = list.findIndex((v) => v.value === currentSessionId.value);
        if (idx > 0) {
          const [cur] = list.splice(idx, 1);
          list.unshift(cur);
        }
      }

      const pod2 = fuzzyFilter(query, podList, (x) => `${x.title} ${x.subtitle ?? ""}`);
      const wl2 = fuzzyFilter(query, wlList, (x) => `${x.title} ${x.subtitle ?? ""}`);
      return [...pod2, ...wl2];
    },
    resolveValue: (raw) => {
      const s = sessions.value.find((item) => item.id === raw);
      if (!s) return null;
      const label = logSessionLabel(s);
      const isCurrent = currentSessionId.value === s.id;
      return {
        value: s.id,
        title: label,
        subtitle: `${s.envName} · ${s.namespace}`,
        hint: isCurrent ? "当前" : undefined,
        icon: "📜",
        domain: s.kind === "pod" ? "log:pod" : "log:workload",
        section: s.kind === "pod" ? "Pod 日志" : "Workload 日志",
        pinned: isCurrent,
        order: isCurrent ? 0 : 10,
      };
    },
  };
}

/**
 * 基于 tokens 中 `@log=<id>` 生成"针对该日志会话的动作"候选。
 */
export function createLogTokenActionsProvider(bridge: WorkbenchBridge) {
  const { sessions, setCurrentSession, closeSession, currentSessionId } = useLogCenterStore();
  const logToken = useLogTokenValue();
  return computed<CommandItem[]>(() => {
    const sid = logToken.value;
    if (!sid) return [];
    const s = sessions.value.find((x) => x.id === sid);
    if (!s) return [];
    const label = logSessionLabel(s);
    const isCurrent = currentSessionId.value === s.id;
    const out: CommandItem[] = [];

    out.push({
      id: `log:act:focus:${s.id}`,
      title: isCurrent ? `前往日志：${label}` : `切换到日志：${label}`,
      subtitle: `${s.envName} · ${s.namespace}`,
      section: "日志动作",
      domain: "log:actions",
      category: "nav",
      icon: "📜",
      keywords: ["focus", "switch", "切换", "聚焦", "前往"],
      weight: 50,
      order: 0,
      run: () => {
        setCurrentSession(s.id);
        bridge.setTab("logCenter");
      },
    });

    out.push({
      id: `log:act:close:${s.id}`,
      title: `关闭日志：${label}`,
      subtitle: s.envName,
      section: "日志动作",
      domain: "log:actions",
      category: "action",
      icon: "✕",
      keywords: ["close", "关闭"],
      weight: 20,
      order: 10,
      run: () => closeSession(s.id),
    });

    return out;
  });
}
