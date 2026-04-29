//! 调试日志：按配置级别写入“本次启动日志文件”，每行一条 JSON。

use crate::config::{app_settings_config_path, debug_log_path, debug_logs_dir, AppSettingsConfig, LogLevel};
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static CURRENT_DEBUG_LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

/// 单条调试记录，对应 DESIGN 中的 DebugEntry。
#[derive(Debug, Clone, Serialize)]
pub struct DebugEntry {
    pub ts: String,
    pub level: String,
    pub resource: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_sample: Option<String>,
}

fn current_log_level() -> LogLevel {
    let path = match app_settings_config_path() {
        Some(p) => p,
        None => return LogLevel::Off,
    };
    AppSettingsConfig::load(&path)
        .map(|c| c.log_level())
        .unwrap_or(LogLevel::Off)
}

fn level_allows(configured: LogLevel, entry_level: LogLevel) -> bool {
    configured.allows(entry_level)
}

fn level_from_str(s: &str) -> LogLevel {
    LogLevel::from_str(s)
}

fn build_startup_log_filename() -> String {
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    format!("kube-flow-debug-{}.log", ts)
}

fn ensure_valid_log_filename(name: &str) -> bool {
    let candidate = Path::new(name);
    if candidate.components().count() != 1 {
        return false;
    }
    matches!(candidate.extension().and_then(|e| e.to_str()), Some("log"))
}

/// 初始化本次应用启动的调试日志文件路径。
pub fn init_current_debug_log_path() -> Option<PathBuf> {
    if let Some(existing) = CURRENT_DEBUG_LOG_PATH.get() {
        return Some(existing.clone());
    }
    let dir = debug_logs_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(build_startup_log_filename());
    let _ = OpenOptions::new().create(true).append(true).open(&path);
    if CURRENT_DEBUG_LOG_PATH.set(path.clone()).is_ok() {
        Some(path)
    } else {
        CURRENT_DEBUG_LOG_PATH.get().cloned()
    }
}

/// 返回本次应用启动对应的日志文件路径。
pub fn current_debug_log_path() -> Option<PathBuf> {
    if let Some(existing) = CURRENT_DEBUG_LOG_PATH.get() {
        return Some(existing.clone());
    }
    init_current_debug_log_path().or_else(debug_log_path)
}

/// 列出所有调试日志文件名（按文件名倒序）。
pub fn list_debug_log_files() -> Vec<String> {
    let Some(dir) = debug_logs_dir() else {
        return Vec::new();
    };
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if ensure_valid_log_filename(name) {
                names.push(name.to_string());
            }
        }
    }
    names.sort_by(|a, b| b.cmp(a));
    names
}

/// 根据文件名解析日志文件绝对路径。
pub fn resolve_debug_log_file_path(file_name: &str) -> Option<PathBuf> {
    if !ensure_valid_log_filename(file_name) {
        return None;
    }
    debug_logs_dir().map(|dir| dir.join(file_name))
}

/// 写入一条调试记录；若当前配置级别不允许则跳过。
pub fn log_debug_entry(entry: DebugEntry) {
    let configured = current_log_level();
    let entry_level = level_from_str(&entry.level);
    if !level_allows(configured, entry_level) {
        return;
    }
    let path = match current_debug_log_path() {
        Some(p) => p,
        None => return,
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let line = match serde_json::to_string(&entry) {
        Ok(s) => s,
        Err(_) => return,
    };
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{}", line);
        let _ = file.flush();
    }
}

/// 便捷：记录 list 成功。
pub fn log_list_ok(resource: &str, env_id: Option<&str>, item_count: u32, level: LogLevel) {
    log_debug_entry(DebugEntry {
        ts: chrono::Utc::now().to_rfc3339(),
        level: level.as_str().to_string(),
        resource: resource.to_string(),
        env_id: env_id.map(String::from),
        result: Some("ok".to_string()),
        item_count: Some(item_count),
        error: None,
        raw_sample: None,
    });
}

/// 便捷：记录 list 失败。
pub fn log_list_err(resource: &str, env_id: Option<&str>, err: &str, level: LogLevel) {
    log_debug_entry(DebugEntry {
        ts: chrono::Utc::now().to_rfc3339(),
        level: level.as_str().to_string(),
        resource: resource.to_string(),
        env_id: env_id.map(String::from),
        result: Some("error".to_string()),
        item_count: None,
        error: Some(err.to_string()),
        raw_sample: None,
    });
}

/// 便捷：记录 SSH 隧道事件（建立成功、失败、转发错误等）。
pub fn log_tunnel(env_id: Option<&str>, result: &str, detail: Option<&str>, level: LogLevel) {
    log_debug_entry(DebugEntry {
        ts: chrono::Utc::now().to_rfc3339(),
        level: level.as_str().to_string(),
        resource: "ssh_tunnel".to_string(),
        env_id: env_id.map(String::from),
        result: Some(result.to_string()),
        item_count: None,
        error: None,
        raw_sample: detail.map(String::from),
    });
}

/// 便捷：记录使用虚拟 kubeconfig 构建 Client（仅记录 server、context 等元信息，不记录证书等敏感内容）。
pub fn log_virtual_kubeconfig(
    env_id: &str,
    context: &str,
    server: &str,
    default_ns: Option<&str>,
    level: LogLevel,
) {
    let detail = match default_ns {
        Some(ns) => format!("context={}, server={}, default_namespace={}", context, server, ns),
        None => format!("context={}, server={}", context, server),
    };
    log_debug_entry(DebugEntry {
        ts: chrono::Utc::now().to_rfc3339(),
        level: level.as_str().to_string(),
        resource: "virtual_kubeconfig".to_string(),
        env_id: Some(env_id.to_string()),
        result: Some("client_build".to_string()),
        item_count: None,
        error: None,
        raw_sample: Some(detail),
    });
}

/// 便捷：记录 SSH 隧道错误。
pub fn log_tunnel_err(env_id: Option<&str>, err: &str, level: LogLevel) {
    log_debug_entry(DebugEntry {
        ts: chrono::Utc::now().to_rfc3339(),
        level: level.as_str().to_string(),
        resource: "ssh_tunnel".to_string(),
        env_id: env_id.map(String::from),
        result: Some("error".to_string()),
        item_count: None,
        error: Some(err.to_string()),
        raw_sample: None,
    });
}
