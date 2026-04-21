//! 配置相关 Tauri 命令：路径、目录创建、应用设置。

use crate::commands::kube_command_context::{err_str, load_app_settings, CommandResult};
use crate::config::GpuResourceRule;

#[tauri::command]
pub fn app_data_dir() -> Option<String> {
    crate::config::app_data_dir().map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn ensure_app_data_dir() -> Option<String> {
    crate::config::ensure_app_data_dir().map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn app_settings_get_ssh_tunnel_mode() -> CommandResult<String> {
    let config = load_app_settings()?;
    Ok(config.default_ssh_tunnel_mode())
}

#[tauri::command]
pub fn app_settings_set_ssh_tunnel_mode(mode: String) -> CommandResult<()> {
    let mut config = load_app_settings()?;
    config.set_default_ssh_tunnel_mode(&mode);
    let path = crate::config::app_settings_config_path()
        .ok_or_else(|| "app data dir not available".to_string())?;
    config.save(&path).map_err(err_str)
}

#[tauri::command]
pub fn app_settings_get_auto_snapshot_enabled() -> CommandResult<bool> {
    let config = load_app_settings()?;
    Ok(config.auto_snapshot_enabled())
}

#[tauri::command]
pub fn app_settings_set_auto_snapshot_enabled(enabled: bool) -> CommandResult<()> {
    let mut config = load_app_settings()?;
    config.set_auto_snapshot_enabled(enabled);
    let path = crate::config::app_settings_config_path()
        .ok_or_else(|| "app data dir not available".to_string())?;
    config.save(&path).map_err(err_str)
}

#[tauri::command]
pub fn app_settings_get_auto_snapshot_limit_per_resource() -> CommandResult<u32> {
    let config = load_app_settings()?;
    Ok(config.auto_snapshot_limit_per_resource())
}

#[tauri::command]
pub fn app_settings_set_auto_snapshot_limit_per_resource(limit: u32) -> CommandResult<()> {
    let mut config = load_app_settings()?;
    config.set_auto_snapshot_limit_per_resource(limit);
    let path = crate::config::app_settings_config_path()
        .ok_or_else(|| "app data dir not available".to_string())?;
    config.save(&path).map_err(err_str)
}

#[tauri::command]
pub fn app_settings_get_terminal_instance_cache_limit() -> CommandResult<u32> {
    let config = load_app_settings()?;
    Ok(config.terminal_instance_cache_limit())
}

#[tauri::command]
pub fn app_settings_set_terminal_instance_cache_limit(limit: u32) -> CommandResult<()> {
    let mut config = load_app_settings()?;
    config.set_terminal_instance_cache_limit(limit);
    let path = crate::config::app_settings_config_path()
        .ok_or_else(|| "app data dir not available".to_string())?;
    config.save(&path).map_err(err_str)
}

#[tauri::command]
pub fn app_settings_get_log_active_stream_limit() -> CommandResult<u32> {
    let config = load_app_settings()?;
    Ok(config.log_active_stream_limit())
}

#[tauri::command]
pub fn app_settings_set_log_active_stream_limit(limit: u32) -> CommandResult<()> {
    let mut config = load_app_settings()?;
    config.set_log_active_stream_limit(limit);
    let path = crate::config::app_settings_config_path()
        .ok_or_else(|| "app data dir not available".to_string())?;
    config.save(&path).map_err(err_str)
}

#[tauri::command]
pub fn app_settings_get_resource_deploy_strategy() -> CommandResult<String> {
    let config = load_app_settings()?;
    Ok(config.resource_deploy_strategy().as_str().to_string())
}

#[tauri::command]
pub fn app_settings_set_resource_deploy_strategy(strategy: String) -> CommandResult<()> {
    let mut config = load_app_settings()?;
    config.set_resource_deploy_strategy(&strategy);
    let path = crate::config::app_settings_config_path()
        .ok_or_else(|| "app data dir not available".to_string())?;
    config.save(&path).map_err(err_str)
}

#[tauri::command]
pub fn app_settings_get_node_resource_usage_enabled() -> CommandResult<bool> {
    let config = load_app_settings()?;
    Ok(config.node_resource_usage_enabled())
}

#[tauri::command]
pub fn app_settings_set_node_resource_usage_enabled(enabled: bool) -> CommandResult<()> {
    let mut config = load_app_settings()?;
    config.set_node_resource_usage_enabled(enabled);
    let path = crate::config::app_settings_config_path()
        .ok_or_else(|| "app data dir not available".to_string())?;
    config.save(&path).map_err(err_str)
}

#[tauri::command]
pub fn app_settings_get_builtin_gpu_resource_names() -> CommandResult<Vec<String>> {
    let config = load_app_settings()?;
    Ok(config.builtin_gpu_resource_names())
}

#[tauri::command]
pub fn app_settings_get_custom_gpu_resource_rules() -> CommandResult<Vec<GpuResourceRule>> {
    let config = load_app_settings()?;
    Ok(config.custom_gpu_resource_rules())
}

#[tauri::command]
pub fn app_settings_set_custom_gpu_resource_rules(rules: Vec<GpuResourceRule>) -> CommandResult<()> {
    let mut config = load_app_settings()?;
    config.set_custom_gpu_resource_rules(rules);
    let path = crate::config::app_settings_config_path()
        .ok_or_else(|| "app data dir not available".to_string())?;
    config.save(&path).map_err(err_str)
}
