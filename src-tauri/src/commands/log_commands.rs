//! 调试日志相关 Tauri 命令：级别读写、日志内容读取、清空、显示设置。

use crate::config::{
    app_settings_config_path, debug_log_path, AppSettingsConfig, LogDisplayFormat, LogDisplayOrder,
    LogLevel,
};

fn app_settings_path() -> Option<std::path::PathBuf> {
    app_settings_config_path()
}

fn log_path() -> Option<std::path::PathBuf> {
    debug_log_path()
}

#[tauri::command]
pub fn log_get_level() -> Result<String, String> {
    let path = app_settings_path().ok_or_else(|| "app data dir not available".to_string())?;
    let config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    Ok(config.log_level().as_str().to_string())
}

#[tauri::command]
pub fn log_set_level(level: String) -> Result<(), String> {
    let path = app_settings_path().ok_or_else(|| "app data dir not available".to_string())?;
    let mut config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    config.set_log_level(LogLevel::from_str(&level));
    config.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn log_read() -> Result<String, String> {
    let path = log_path().ok_or_else(|| "app data dir not available".to_string())?;
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    Ok(content)
}

#[tauri::command]
pub fn log_get_display_settings() -> Result<(String, String, u32), String> {
    let path = app_settings_path().ok_or_else(|| "app data dir not available".to_string())?;
    let config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    Ok((
        config.log_display_order().as_str().to_string(),
        config.log_display_format().as_str().to_string(),
        config.log_tail_lines(),
    ))
}

#[tauri::command]
pub fn log_set_display_settings(order: String, format: String, tail_lines: u32) -> Result<(), String> {
    let path = app_settings_path().ok_or_else(|| "app data dir not available".to_string())?;
    let mut config = AppSettingsConfig::load(&path).map_err(|e| e.to_string())?;
    config.set_log_display_order(LogDisplayOrder::from_str(&order));
    config.set_log_display_format(LogDisplayFormat::from_str(&format));
    config.set_log_tail_lines(tail_lines);
    config.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn log_clear() -> Result<(), String> {
    let path = log_path().ok_or_else(|| "app data dir not available".to_string())?;
    if path.exists() {
        std::fs::write(&path, "").map_err(|e| e.to_string())?;
    }
    Ok(())
}
