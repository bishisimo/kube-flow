//! 配置相关 Tauri 命令：路径、目录创建、应用设置。

use crate::config::{app_settings_config_path, AppSettingsConfig};

#[tauri::command]
pub fn app_data_dir() -> Option<String> {
    crate::config::app_data_dir().map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn ensure_app_data_dir() -> Option<String> {
    crate::config::ensure_app_data_dir().map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub fn app_settings_get_ssh_tunnel_mode() -> Result<String, String> {
    let path = app_settings_config_path().ok_or_else(|| "app data dir not available".to_string())?;
    let config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    Ok(config.default_ssh_tunnel_mode())
}

#[tauri::command]
pub fn app_settings_set_ssh_tunnel_mode(mode: String) -> Result<(), String> {
    let path = app_settings_config_path().ok_or_else(|| "app data dir not available".to_string())?;
    let mut config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    config.set_default_ssh_tunnel_mode(&mode);
    config.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn app_settings_get_auto_snapshot_enabled() -> Result<bool, String> {
    let path = app_settings_config_path().ok_or_else(|| "app data dir not available".to_string())?;
    let config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    Ok(config.auto_snapshot_enabled())
}

#[tauri::command]
pub fn app_settings_set_auto_snapshot_enabled(enabled: bool) -> Result<(), String> {
    let path = app_settings_config_path().ok_or_else(|| "app data dir not available".to_string())?;
    let mut config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    config.set_auto_snapshot_enabled(enabled);
    config.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn app_settings_get_auto_snapshot_limit_per_resource() -> Result<u32, String> {
    let path = app_settings_config_path().ok_or_else(|| "app data dir not available".to_string())?;
    let config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    Ok(config.auto_snapshot_limit_per_resource())
}

#[tauri::command]
pub fn app_settings_set_auto_snapshot_limit_per_resource(limit: u32) -> Result<(), String> {
    let path = app_settings_config_path().ok_or_else(|| "app data dir not available".to_string())?;
    let mut config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    config.set_auto_snapshot_limit_per_resource(limit);
    config.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn app_settings_get_terminal_instance_cache_limit() -> Result<u32, String> {
    let path = app_settings_config_path().ok_or_else(|| "app data dir not available".to_string())?;
    let config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    Ok(config.terminal_instance_cache_limit())
}

#[tauri::command]
pub fn app_settings_set_terminal_instance_cache_limit(limit: u32) -> Result<(), String> {
    let path = app_settings_config_path().ok_or_else(|| "app data dir not available".to_string())?;
    let mut config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    config.set_terminal_instance_cache_limit(limit);
    config.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn app_settings_get_log_active_stream_limit() -> Result<u32, String> {
    let path = app_settings_config_path().ok_or_else(|| "app data dir not available".to_string())?;
    let config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    Ok(config.log_active_stream_limit())
}

#[tauri::command]
pub fn app_settings_set_log_active_stream_limit(limit: u32) -> Result<(), String> {
    let path = app_settings_config_path().ok_or_else(|| "app data dir not available".to_string())?;
    let mut config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    config.set_log_active_stream_limit(limit);
    config.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn app_settings_get_resource_deploy_strategy() -> Result<String, String> {
    let path = app_settings_config_path().ok_or_else(|| "app data dir not available".to_string())?;
    let config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    Ok(config.resource_deploy_strategy().as_str().to_string())
}

#[tauri::command]
pub fn app_settings_set_resource_deploy_strategy(strategy: String) -> Result<(), String> {
    let path = app_settings_config_path().ok_or_else(|| "app data dir not available".to_string())?;
    let mut config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    config.set_resource_deploy_strategy(&strategy);
    config.save(&path).map_err(|e| e.to_string())
}
