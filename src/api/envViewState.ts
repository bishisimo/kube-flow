import { invoke } from "@tauri-apps/api/core";
import type { ResolvedAliasTarget } from "./types/kube";

export interface EnvViewState {
  namespace: string | null;
  kind: string;
  nameFilter: string;
  nodeFilter: string;
  podIpFilter: string;
  labelSelector: string;
  customTarget?: ResolvedAliasTarget | null;
}

export function envViewStateList(): Promise<Record<string, EnvViewState>> {
  return invoke<Record<string, EnvViewState>>("env_view_state_list");
}

export function envViewStateSet(envId: string, state: EnvViewState): Promise<void> {
  return invoke("env_view_state_set", { args: { envId, state } });
}

export function envViewStateDelete(envId: string): Promise<void> {
  return invoke("env_view_state_delete", { envId });
}
