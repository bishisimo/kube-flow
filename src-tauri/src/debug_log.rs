//! 调试日志：按配置级别写入 kube-flow-debug.log，每行一条 JSON。

use crate::config::{app_settings_config_path, debug_log_path, AppSettingsConfig, LogLevel};
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;

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

/// 写入一条调试记录；若当前配置级别不允许则跳过。
pub fn log_debug_entry(entry: DebugEntry) {
    let configured = current_log_level();
    let entry_level = level_from_str(&entry.level);
    if !level_allows(configured, entry_level) {
        return;
    }
    let path = match debug_log_path() {
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
