/**
 * 应用设置：SSH 隧道默认映射方式等。
 */
import { invoke } from "@tauri-apps/api/core";

export type TunnelMappingMode = "ssh" | "builtin";
export type ResourceDeployStrategy = "create_replace" | "apply";
export interface GpuResourceRule {
  display_name: string;
  resource_name: string;
}

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

export function appSettingsGetTerminalInstanceCacheLimit(): Promise<number> {
  return invoke("app_settings_get_terminal_instance_cache_limit");
}

export function appSettingsSetTerminalInstanceCacheLimit(limit: number): Promise<void> {
  return invoke("app_settings_set_terminal_instance_cache_limit", { limit });
}

export function appSettingsGetLogActiveStreamLimit(): Promise<number> {
  return invoke("app_settings_get_log_active_stream_limit");
}

export function appSettingsSetLogActiveStreamLimit(limit: number): Promise<void> {
  return invoke("app_settings_set_log_active_stream_limit", { limit });
}

export function appSettingsGetResourceDeployStrategy(): Promise<ResourceDeployStrategy> {
  return invoke("app_settings_get_resource_deploy_strategy");
}

export function appSettingsSetResourceDeployStrategy(strategy: ResourceDeployStrategy): Promise<void> {
  return invoke("app_settings_set_resource_deploy_strategy", { strategy });
}

export function appSettingsGetNodeResourceUsageEnabled(): Promise<boolean> {
  return invoke("app_settings_get_node_resource_usage_enabled");
}

export function appSettingsSetNodeResourceUsageEnabled(enabled: boolean): Promise<void> {
  return invoke("app_settings_set_node_resource_usage_enabled", { enabled });
}

export function appSettingsGetBuiltinGpuResourceNames(): Promise<string[]> {
  return invoke("app_settings_get_builtin_gpu_resource_names");
}

export function appSettingsGetCustomGpuResourceRules(): Promise<GpuResourceRule[]> {
  return invoke("app_settings_get_custom_gpu_resource_rules");
}

export function appSettingsSetCustomGpuResourceRules(rules: GpuResourceRule[]): Promise<void> {
  return invoke("app_settings_set_custom_gpu_resource_rules", { rules });
}
