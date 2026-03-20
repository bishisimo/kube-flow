import { invoke } from "@tauri-apps/api/core";

export function hostShellStart(envId: string): Promise<string> {
  return invoke("host_shell_start", { envId });
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
