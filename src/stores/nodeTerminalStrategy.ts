import { ref } from "vue";
import type { HostShellBootstrap, NodeTerminalStep, NodeTerminalStepType, PodDebugTarget } from "../api/terminal";
export type { NodeTerminalStepType } from "../api/terminal";
import { uid } from "../utils/uid";
import { createStorage } from "../utils/storage";

const STORAGE_KEY = "kube-flow:node-terminal-strategies";

export interface NodeTerminalStepConfig {
  id: string;
  type: NodeTerminalStepType;
  user: string;
}

export interface NodeTerminalStrategy {
  envId: string;
  enabled: boolean;
  nodeAddressTemplate: string;
  steps: NodeTerminalStepConfig[];
  hasSavedPassword: boolean;
}

function createStep(type: NodeTerminalStepType, user = "root"): NodeTerminalStepConfig {
  return {
    id: uid("step"),
    type,
    user,
  };
}

function defaultSteps(): NodeTerminalStepConfig[] {
  return [createStep("ssh", "root")];
}

function defaultStrategy(envId: string): NodeTerminalStrategy {
  return {
    envId,
    enabled: false,
    nodeAddressTemplate: "{node}",
    steps: defaultSteps(),
    hasSavedPassword: false,
  };
}

function sanitizeStrategies(raw: unknown): Record<string, NodeTerminalStrategy> {
  if (!raw || typeof raw !== "object") return {};
  const parsed = raw as Record<string, Partial<NodeTerminalStrategy>>;
  return Object.fromEntries(
    Object.entries(parsed).map(([envId, strategy]) => [
      envId,
      {
        ...defaultStrategy(envId),
        envId,
        enabled: Boolean(strategy?.enabled),
        nodeAddressTemplate: strategy?.nodeAddressTemplate?.trim() || "{node}",
        steps: sanitizeSteps(strategy?.steps),
        hasSavedPassword: Boolean(strategy?.hasSavedPassword),
      },
    ])
  );
}

const strategiesStorage = createStorage<Record<string, NodeTerminalStrategy>>({
  key: STORAGE_KEY,
  version: 1,
  fallback: {},
  migrate: (old) => sanitizeStrategies(old),
});

const strategies = ref<Record<string, NodeTerminalStrategy>>(strategiesStorage.read());

function persist() {
  strategiesStorage.write(strategies.value);
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

export function nodeTerminalSwitchUserCredentialId(envId: string): string {
  return `node-terminal-strategy:switch-user:${envId}`;
}

export function createNodeTerminalStep(type: NodeTerminalStepType, user = "root"): NodeTerminalStepConfig {
  return createStep(type, user);
}

function renderTemplate(template: string, vars: Record<string, string>): string {
  return template.replace(/\{(\w+)\}/g, (_all, key: string) => vars[key] ?? "");
}

function sanitizeSteps(steps: NodeTerminalStepConfig[] | undefined): NodeTerminalStepConfig[] {
  const normalized = (steps ?? [])
    .map((step, index) => {
      if (step.type !== "ssh" && step.type !== "switch_user") return null;
      return {
        id: step.id || `step-${index}`,
        type: step.type,
        user: step.user?.trim() || "root",
      } satisfies NodeTerminalStepConfig;
    })
    .filter((item): item is NodeTerminalStepConfig => item !== null);
  return normalized.length ? normalized : defaultSteps();
}

function compilePreviewCommand(host: string, steps: NodeTerminalStepConfig[]): string {
  return sanitizeSteps(steps)
    .map((step) =>
      step.type === "ssh"
        ? `ssh ${step.user}@${host}`
        : `sudo su - ${step.user}`
    )
    .join("\n")
    .trim();
}

export function buildNodeTerminalCommand(
  strategy: NodeTerminalStrategy | null | undefined,
  nodeName: string
): { host: string; command: string } | null {
  if (!strategy || !strategy.enabled) return null;
  const host = renderTemplate(strategy.nodeAddressTemplate || "{node}", {
    node: nodeName,
    host: nodeName,
  }).trim();
  if (!host) return null;
  const command = compilePreviewCommand(host, strategy.steps);
  if (!command) return null;
  return { host, command };
}

function toLaunchSteps(steps: NodeTerminalStepConfig[]): NodeTerminalStep[] {
  return sanitizeSteps(steps).map((step) => ({
    type: step.type,
    user: step.user,
  }));
}

export function strategyNeedsSwitchUserPassword(
  strategy: NodeTerminalStrategy | null | undefined
): boolean {
  return Boolean(strategy?.steps.some((step) => step.type === "switch_user"));
}

export function buildNodeTerminalLaunch(
  strategy: NodeTerminalStrategy | null | undefined,
  nodeName: string,
  podDebug?: PodDebugTarget | null
): HostShellBootstrap | null {
  const target = buildNodeTerminalCommand(strategy, nodeName);
  if (!strategy || !target) return null;
  return {
    kind: "node_terminal",
    host: target.host,
    steps: toLaunchSteps(strategy.steps),
    credentialId:
      !strategyNeedsSwitchUserPassword(strategy) || !strategy.hasSavedPassword
        ? null
        : nodeTerminalSwitchUserCredentialId(strategy.envId),
    podDebug: podDebug ?? null,
  };
}

export function useNodeTerminalStrategyStore() {
  return {
    strategies,
    getNodeTerminalStrategy,
    setNodeTerminalStrategy,
    buildNodeTerminalCommand,
    buildNodeTerminalLaunch,
    nodeTerminalSwitchUserCredentialId,
    createNodeTerminalStep,
    strategyNeedsSwitchUserPassword,
  };
}
