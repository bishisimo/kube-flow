import { invoke } from "@tauri-apps/api/core";

export type NodeTerminalStepType = "ssh" | "switch_user";
export type PodDebugNamespace = "net" | "pid" | "mnt" | "uts" | "ipc";

export interface NodeTerminalStep {
  type: NodeTerminalStepType;
  user: string;
}

export interface PodDebugTarget {
  namespace: string;
  podName: string;
  container: string;
  namespaces: PodDebugNamespace[];
  pid?: number | null;
}

export interface HostShellBootstrap {
  kind: "node_terminal";
  host: string;
  steps: NodeTerminalStep[];
  credentialId?: string | null;
  podDebug?: PodDebugTarget | null;
}

export function hostShellStart(envId: string, bootstrap?: HostShellBootstrap | null): Promise<string> {
  return invoke("host_shell_start", { envId, bootstrap: bootstrap ?? null });
}

export function hostShellStdin(streamId: string, data: number[]): Promise<void> {
  return invoke("host_shell_stdin", { streamId, data });
}

export function hostShellResize(streamId: string, cols: number, rows: number): Promise<void> {
  return invoke("host_shell_resize", { streamId, cols, rows });
}

export function hostShellStop(streamId: string): Promise<void> {
  return invoke("host_shell_stop", { streamId });
}
