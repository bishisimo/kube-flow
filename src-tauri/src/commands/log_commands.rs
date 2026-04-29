//! 调试日志相关 Tauri 命令：级别读写、日志内容读取、清空、显示设置。

use crate::commands::kube_command_context::{err_str, load_app_settings, CommandResult};
use crate::config::LogLevel;
use crate::config::{LogDisplayFormat, LogDisplayOrder};
use crate::debug_log::{current_debug_log_path, list_debug_log_files, resolve_debug_log_file_path};
use serde::Serialize;

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
pub fn log_read(file_name: Option<String>) -> CommandResult<String> {
    let path = match file_name.as_deref() {
        Some(name) => resolve_debug_log_file_path(name).ok_or_else(|| "invalid log file name".to_string())?,
        None => current_debug_log_path().ok_or_else(|| "app data dir not available".to_string())?,
    };
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    Ok(content)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugLogFileItem {
    pub file_name: String,
    pub is_current: bool,
}

#[tauri::command]
pub fn log_list_files() -> CommandResult<Vec<DebugLogFileItem>> {
    let current_name = current_debug_log_path()
        .and_then(|p| p.file_name().and_then(|s| s.to_str()).map(str::to_string));
    let files = list_debug_log_files()
        .into_iter()
        .map(|file_name| DebugLogFileItem {
            is_current: current_name.as_deref() == Some(file_name.as_str()),
            file_name,
        })
        .collect();
    Ok(files)
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
    let path = current_debug_log_path().ok_or_else(|| "app data dir not available".to_string())?;
    if path.exists() {
        std::fs::write(&path, "").map_err(err_str)?;
    }
    Ok(())
}

#[tauri::command]
pub fn log_delete(file_name: String) -> CommandResult<()> {
    let path = resolve_debug_log_file_path(&file_name).ok_or_else(|| "invalid log file name".to_string())?;
    let current_path = current_debug_log_path();
    if current_path.as_ref() == Some(&path) {
        return Err("cannot delete current startup log file".to_string());
    }
    if path.exists() {
        std::fs::remove_file(path).map_err(err_str)?;
    }
    Ok(())
}
