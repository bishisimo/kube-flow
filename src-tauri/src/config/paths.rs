//! 应用数据目录与配置文件路径。
//! 与平台无关：使用 Tauri 或 dirs 解析 app_data_dir。

use std::path::PathBuf;

/// 返回应用数据根目录，如 `~/.local/share/kube-flow`（Linux）或等价路径。
pub fn app_data_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("kube-flow"))
}

/// 环境与 SSH 隧道配置：`{app_data_dir}/kube-flow.toml`
pub fn kube_flow_config_path() -> Option<PathBuf> {
    app_data_dir().map(|p| p.join("kube-flow.toml"))
}

/// CRD 展示独立配置：`{app_data_dir}/crd-display.toml`
#[allow(dead_code)]
pub fn crd_display_config_path() -> Option<PathBuf> {
    app_data_dir().map(|p| p.join("crd-display.toml"))
}

/// 调试日志目录：`{app_data_dir}/debug-logs`
pub fn debug_logs_dir() -> Option<PathBuf> {
    app_data_dir().map(|p| p.join("debug-logs"))
}

/// 默认调试日志文件：`{app_data_dir}/debug-logs/kube-flow-debug.log`
/// 仅用于兼容与兜底；实际运行时优先使用“本次启动日志文件”。
pub fn debug_log_path() -> Option<PathBuf> {
    debug_logs_dir().map(|p| p.join("kube-flow-debug.log"))
}

/// 应用设置（含日志级别）：`{app_data_dir}/app-settings.toml`
pub fn app_settings_config_path() -> Option<PathBuf> {
    app_data_dir().map(|p| p.join("app-settings.toml"))
}

/// MCP 权限策略：`{app_data_dir}/mcp-policy.toml`
pub fn mcp_policy_config_path() -> Option<PathBuf> {
    app_data_dir().map(|p| p.join("mcp-policy.toml"))
}

/// MCP 访问令牌：`{app_data_dir}/mcp.token`
pub fn mcp_token_path() -> Option<PathBuf> {
    app_data_dir().map(|p| p.join("mcp.token"))
}

/// MCP 审计日志：`{app_data_dir}/mcp-audit.jsonl`
pub fn mcp_audit_log_path() -> Option<PathBuf> {
    app_data_dir().map(|p| p.join("mcp-audit.jsonl"))
}

/// MCP App Gateway Unix socket：`{app_data_dir}/mcp.sock`（仅 Unix）。
#[cfg(unix)]
pub fn mcp_gateway_sock_path() -> Option<PathBuf> {
    app_data_dir().map(|p| p.join("mcp.sock"))
}

/// MCP App Gateway 监听端口文件：`{app_data_dir}/mcp.port`（Windows / 回退）。
pub fn mcp_gateway_port_path() -> Option<PathBuf> {
    app_data_dir().map(|p| p.join("mcp.port"))
}

/// MCP HTTP 服务监听端口文件：`{app_data_dir}/mcp-http.port`。
pub fn mcp_http_port_path() -> Option<PathBuf> {
    app_data_dir().map(|p| p.join("mcp-http.port"))
}

/// 确保应用数据目录存在；若路径不可用则返回 None。
pub fn ensure_app_data_dir() -> Option<PathBuf> {
    let dir = app_data_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}
