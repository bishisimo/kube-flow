//! 调试日志相关 Tauri 命令：级别读写、日志内容读取、清空、显示设置。

use crate::commands::kube_command_context::{err_str, load_app_settings, CommandResult};
use crate::config::{debug_log_path, LogDisplayFormat, LogDisplayOrder, LogLevel};

#[tauri::command]
pub fn log_get_level() -> CommandResult<String> {
    let config = load_app_settings()?;
    Ok(config.log_level().as_str().to_string())
}

#[tauri::command]
pub fn log_set_level(level: String) -> CommandResult<()> {
    let mut config = load_app_settings()?;
    config.set_log_level(LogLevel::from_str(&level));
    let path = crate::config::app_settings_config_path()
        .ok_or_else(|| "app data dir not available".to_string())?;
    config.save(&path).map_err(err_str)
}

#[tauri::command]
pub fn log_read() -> CommandResult<String> {
    let path = debug_log_path().ok_or_else(|| "app data dir not available".to_string())?;
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    Ok(content)
}

#[tauri::command]
pub fn log_get_display_settings() -> CommandResult<(String, String, u32)> {
    let config = load_app_settings()?;
    Ok((
        config.log_display_order().as_str().to_string(),
        config.log_display_format().as_str().to_string(),
        config.log_tail_lines(),
    ))
}

#[tauri::command]
pub fn log_set_display_settings(order: String, format: String, tail_lines: u32) -> CommandResult<()> {
    let mut config = load_app_settings()?;
    config.set_log_display_order(LogDisplayOrder::from_str(&order));
    config.set_log_display_format(LogDisplayFormat::from_str(&format));
    config.set_log_tail_lines(tail_lines);
    let path = crate::config::app_settings_config_path()
        .ok_or_else(|| "app data dir not available".to_string())?;
    config.save(&path).map_err(err_str)
}

#[tauri::command]
pub fn log_clear() -> CommandResult<()> {
    let path = debug_log_path().ok_or_else(|| "app data dir not available".to_string())?;
    if path.exists() {
        std::fs::write(&path, "").map_err(err_str)?;
    }
    Ok(())
}
