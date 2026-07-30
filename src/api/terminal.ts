import { invoke } from "@tauri-apps/api/core";

export type NodeTerminalStepType = "ssh" | "switch_user" | "kind_node_exec";
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

export function hostShellStart(
  envId: string,
  bootstrap?: HostShellBootstrap | null,
  cols?: number | null,
  rows?: number | null
): Promise<string> {
  return invoke("host_shell_start", {
    envId,
    bootstrap: bootstrap ?? null,
    cols: cols ?? null,
    rows: rows ?? null,
  });
}

export function hostShellStdin(streamId: string, dataB64: string): Promise<void> {
  return invoke("host_shell_stdin", { streamId, dataB64 });
}

export function hostShellAck(streamId: string, bytes: number): Promise<void> {
  return invoke("host_shell_ack", { streamId, bytes });
}

export function hostShellResize(streamId: string, cols: number, rows: number): Promise<void> {
  return invoke("host_shell_resize", { streamId, cols, rows });
}

export function hostShellStop(streamId: string): Promise<void> {
  return invoke("host_shell_stop", { streamId });
}

export function hostFileUpload(
  envId: string,
  localPath: string,
  remotePath: string,
  overwrite = false
): Promise<string> {
  return invoke("host_file_upload", { envId, localPath, remotePath, overwrite });
}

export function hostFileDownload(
  envId: string,
  remotePath: string,
  localPath: string,
  overwrite = false
): Promise<string> {
  return invoke("host_file_download", { envId, remotePath, localPath, overwrite });
}
