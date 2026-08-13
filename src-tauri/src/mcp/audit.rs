//! MCP 审计日志（JSONL）。

use crate::config::{app_data_dir, ensure_app_data_dir, mcp_audit_log_path};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub ts: String,
    pub env_id: Option<String>,
    pub tool: String,
    pub capability: String,
    pub ok: bool,
    pub error: Option<String>,
    pub approved: bool,
    pub via: String,
}

/// 追加一条审计记录。
pub fn append(entry: &AuditEntry) -> Result<(), String> {
    ensure_app_data_dir().ok_or_else(|| "app data dir unavailable".to_string())?;
    let path = resolve_audit_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let line = serde_json::to_string(entry).map_err(|e| e.to_string())?;
    writeln!(f, "{line}").map_err(|e| e.to_string())
}

pub fn record(
    env_id: Option<&str>,
    tool: &str,
    capability: &str,
    ok: bool,
    error: Option<&str>,
    approved: bool,
    via: &str,
) {
    let entry = AuditEntry {
        ts: Utc::now().to_rfc3339(),
        env_id: env_id.map(str::to_string),
        tool: tool.to_string(),
        capability: capability.to_string(),
        ok,
        error: error.map(str::to_string),
        approved,
        via: via.to_string(),
    };
    let _ = append(&entry);
}

fn resolve_audit_path() -> Result<PathBuf, String> {
    if let Ok(policy) = super::policy::McpPolicy::load_default() {
        let name = policy.mcp.audit_log;
        if let Some(dir) = app_data_dir() {
            return Ok(dir.join(name));
        }
    }
    mcp_audit_log_path().ok_or_else(|| "audit path unavailable".to_string())
}

/// 读取最近 N 条审计（从文件尾部扫描）。
pub fn read_recent(limit: usize) -> Result<Vec<AuditEntry>, String> {
    let path = resolve_audit_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut entries: Vec<AuditEntry> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    if entries.len() > limit {
        entries = entries.split_off(entries.len() - limit);
    }
    Ok(entries)
}
