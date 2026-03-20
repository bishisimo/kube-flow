import { ref } from "vue";

const STORAGE_KEY = "kube-flow:node-terminal-strategies";

export interface NodeTerminalStrategy {
  envId: string;
  enabled: boolean;
  nodeAddressTemplate: string;
  switchUser: string;
  switchPassword: string;
  commandTemplate: string;
}

const strategies = ref<Record<string, NodeTerminalStrategy>>(loadStrategies());

function defaultStrategy(envId: string): NodeTerminalStrategy {
  return {
    envId,
    enabled: false,
    nodeAddressTemplate: "{node}",
    switchUser: "root",
    switchPassword: "",
    commandTemplate: "ssh {user}@{host}",
  };
}

function loadStrategies(): Record<string, NodeTerminalStrategy> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, Partial<NodeTerminalStrategy>>;
    if (!parsed || typeof parsed !== "object") return {};
    return Object.fromEntries(
      Object.entries(parsed).map(([envId, strategy]) => [
        envId,
        {
          ...defaultStrategy(envId),
          ...strategy,
          envId,
          enabled: Boolean(strategy?.enabled),
          nodeAddressTemplate: strategy?.nodeAddressTemplate?.trim() || "{node}",
          switchUser: strategy?.switchUser?.trim() || "root",
          switchPassword: typeof strategy?.switchPassword === "string" ? strategy.switchPassword : "",
          commandTemplate: strategy?.commandTemplate?.trim() || "ssh {user}@{host}",
        },
      ])
    );
  } catch {
    return {};
  }
}

function persist() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(strategies.value));
  } catch {}
}

export function getNodeTerminalStrategy(envId: string | null | undefined): NodeTerminalStrategy | null {
  if (!envId) return null;
  return strategies.value[envId] ?? defaultStrategy(envId);
}

export function setNodeTerminalStrategy(envId: string, patch: Partial<NodeTerminalStrategy>) {
  strategies.value = {
    ...strategies.value,
    [envId]: {
      ...(strategies.value[envId] ?? defaultStrategy(envId)),
      ...patch,
      envId,
    },
  };
  persist();
}

function renderTemplate(template: string, vars: Record<string, string>): string {
  return template.replace(/\{(\w+)\}/g, (_all, key: string) => vars[key] ?? "");
}

export function buildNodeTerminalCommand(
  strategy: NodeTerminalStrategy | null | undefined,
  nodeName: string
): { host: string; command: string } | null {
  if (!strategy || !strategy.enabled) return null;
  const host = renderTemplate(strategy.nodeAddressTemplate || "{node}", {
    node: nodeName,
    host: nodeName,
    user: strategy.switchUser,
    password: strategy.switchPassword,
  }).trim();
  if (!host) return null;
  const command = renderTemplate(strategy.commandTemplate || "ssh {user}@{host}", {
    node: nodeName,
    host,
    user: strategy.switchUser,
    password: strategy.switchPassword,
  }).trim();
  if (!command) return null;
  return { host, command };
}

export function useNodeTerminalStrategyStore() {
  return {
    strategies,
    getNodeTerminalStrategy,
    setNodeTerminalStrategy,
    buildNodeTerminalCommand,
  };
}
