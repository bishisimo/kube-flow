/**
 * 应用设置：SSH 隧道默认映射方式等。
 */
import { invoke } from "@tauri-apps/api/core";

export type TunnelMappingMode = "ssh" | "builtin";

export function appSettingsGetSshTunnelMode(): Promise<TunnelMappingMode> {
  return invoke("app_settings_get_ssh_tunnel_mode");
}

export function appSettingsSetSshTunnelMode(mode: TunnelMappingMode): Promise<void> {
  return invoke("app_settings_set_ssh_tunnel_mode", { mode });
}

