//! 应用磁盘存储用量统计与可选清理（调试日志目录、SSH 配置备份等）。

use crate::commands::kube_command_context::{err_str, CommandResult};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileItem {
    pub name: String,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageDirectoryUsage {
    pub path: String,
    pub total_bytes: u64,
    pub file_count: u32,
    pub files: Vec<StorageFileItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageDiskUsage {
    pub app_data_dir: String,
    pub debug_logs: StorageDirectoryUsage,
    pub app_files: Vec<StorageFileItem>,
    pub ssh_backups: StorageDirectoryUsage,
}

fn file_bytes(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn collect_directory_usage(dir: &Path, name_filter: impl Fn(&str) -> bool) -> StorageDirectoryUsage {
    let mut files = Vec::new();
    let mut total_bytes = 0u64;
    if dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                    continue;
                };
                if !name_filter(name) {
                    continue;
                }
                let bytes = file_bytes(&path);
                total_bytes += bytes;
                files.push(StorageFileItem {
                    name: name.to_string(),
                    bytes,
                });
            }
        }
    }
    files.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    let file_count = files.len() as u32;
    StorageDirectoryUsage {
        path: dir.to_string_lossy().to_string(),
        total_bytes,
        file_count,
        files,
    }
}

fn collect_known_app_files(app_dir: &Path) -> Vec<StorageFileItem> {
    const KNOWN: &[&str] = &[
        "app-settings.toml",
        "kube-flow.toml",
        "credentials.hold",
        "env-view-states.json",
        "kube-flow-debug.log",
        "crd-display.toml",
    ];
    let mut files = Vec::new();
    for name in KNOWN {
        let path = app_dir.join(name);
        if path.is_file() {
            files.push(StorageFileItem {
                name: (*name).to_string(),
                bytes: file_bytes(&path),
            });
        }
    }
    files.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));
    files
}

fn ssh_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ssh"))
}

fn is_ssh_backup_name(name: &str) -> bool {
    name.starts_with("config.kube-flow.") && name.ends_with(".bak")
}

/// 返回应用数据目录、调试日志、配置文件与 SSH 备份的磁盘占用统计。
#[tauri::command]
pub fn storage_get_disk_usage() -> CommandResult<StorageDiskUsage> {
    let app_dir = crate::config::app_data_dir()
        .ok_or_else(|| "app data dir not available".to_string())?;
    let debug_dir = crate::config::debug_logs_dir().unwrap_or_else(|| app_dir.join("debug-logs"));
    let debug_logs = collect_directory_usage(&debug_dir, |name| name.ends_with(".log"));
    let app_files = collect_known_app_files(&app_dir);
    let ssh_backups = ssh_dir()
        .map(|dir| collect_directory_usage(&dir, is_ssh_backup_name))
        .unwrap_or_else(|| StorageDirectoryUsage {
            path: String::new(),
            total_bytes: 0,
            file_count: 0,
            files: Vec::new(),
        });
    Ok(StorageDiskUsage {
        app_data_dir: app_dir.to_string_lossy().to_string(),
        debug_logs,
        app_files,
        ssh_backups,
    })
}

/// 将文本写入用户选定的绝对路径（用于终端交互历史等导出）。
#[tauri::command]
pub fn storage_write_text_file(path: String, content: String) -> CommandResult<()> {
    let target = PathBuf::from(&path);
    if !target.is_absolute() {
        return Err("path must be absolute".to_string());
    }
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(err_str)?;
        }
    }
    std::fs::write(&target, content).map_err(err_str)
}

/// 删除 ~/.ssh 下由 kube-flow 写入的配置备份文件，返回删除数量。
#[tauri::command]
pub fn storage_delete_ssh_backups() -> CommandResult<u32> {
    let Some(dir) = ssh_dir() else {
        return Ok(0);
    };
    let mut deleted = 0u32;
    let entries = std::fs::read_dir(&dir).map_err(err_str)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !is_ssh_backup_name(name) {
            continue;
        }
        if std::fs::remove_file(&path).is_ok() {
            deleted += 1;
        }
    }
    Ok(deleted)
}
