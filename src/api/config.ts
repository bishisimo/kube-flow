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

export function appSettingsGetAutoSnapshotEnabled(): Promise<boolean> {
  return invoke("app_settings_get_auto_snapshot_enabled");
}

export function appSettingsSetAutoSnapshotEnabled(enabled: boolean): Promise<void> {
  return invoke("app_settings_set_auto_snapshot_enabled", { enabled });
}

export function appSettingsGetAutoSnapshotLimitPerResource(): Promise<number> {
  return invoke("app_settings_get_auto_snapshot_limit_per_resource");
}

export function appSettingsSetAutoSnapshotLimitPerResource(limit: number): Promise<void> {
  return invoke("app_settings_set_auto_snapshot_limit_per_resource", { limit });
}
