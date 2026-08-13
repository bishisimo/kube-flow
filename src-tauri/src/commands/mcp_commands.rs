//! MCP 策略管理、本机服务启停、确认应答与客户端配置导出。

use crate::mcp::audit::{self, AuditEntry};
use crate::mcp::gateway::{self, ApprovalDecision, McpGatewayState};
use crate::mcp::http_server::{self, McpHttpState, McpServiceStatus, DEFAULT_HTTP_PORT};
use crate::mcp::policy::McpPolicy;
use crate::mcp::token;
use crate::commands::kube_command_context::{err_str, CommandResult};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn mcp_get_policy() -> CommandResult<McpPolicy> {
    McpPolicy::load_default().map_err(err_str)
}

#[tauri::command]
pub async fn mcp_set_policy(
    app: AppHandle,
    state: State<'_, Arc<McpGatewayState>>,
    policy: McpPolicy,
) -> CommandResult<()> {
    policy.save_default().map_err(err_str)?;
    if policy.mcp.enabled {
        let _ = token::ensure_token()?;
    }
    gateway::sync_mcp_services(&app).await?;
    let _ = state;
    Ok(())
}

#[tauri::command]
pub async fn mcp_gateway_status(state: State<'_, Arc<McpGatewayState>>) -> CommandResult<bool> {
    Ok(state.is_running().await)
}

/// 返回 Gateway + HTTP MCP 的运行状态快照。
#[tauri::command]
pub async fn mcp_service_status(
    gateway: State<'_, Arc<McpGatewayState>>,
    http: State<'_, Arc<McpHttpState>>,
) -> CommandResult<McpServiceStatus> {
    let policy = McpPolicy::load_default().map_err(err_str)?;
    let http_running = http.is_running().await;
    let http_port = http.port().await;
    let http_url = http.endpoint_url().await;
    Ok(McpServiceStatus {
        gateway_running: gateway.is_running().await,
        http_running,
        http_port,
        http_url,
        token_ready: token::read_token().is_ok() || token::ensure_token().is_ok(),
        policy_enabled: policy.mcp.enabled,
    })
}

/// 一键启动：启用策略、启动 Gateway 与本机 HTTP MCP。
#[tauri::command]
pub async fn mcp_service_start(
    app: AppHandle,
    gateway: State<'_, Arc<McpGatewayState>>,
    http: State<'_, Arc<McpHttpState>>,
) -> CommandResult<McpServiceStatus> {
    let mut policy = McpPolicy::load_default().map_err(err_str)?;
    policy.mcp.enabled = true;
    policy.mcp.listen = true;
    policy.save_default().map_err(err_str)?;
    let _ = token::ensure_token()?;
    gateway::start_gateway(app.clone(), gateway.inner().clone()).await?;
    let _ = http_server::start_http(http.inner().clone()).await?;
    mcp_service_status(gateway, http).await
}

/// 停止本机 HTTP MCP 与 Gateway，并将 listen 设为 false（总开关保持，便于再次启动）。
#[tauri::command]
pub async fn mcp_service_stop(
    gateway: State<'_, Arc<McpGatewayState>>,
    http: State<'_, Arc<McpHttpState>>,
) -> CommandResult<McpServiceStatus> {
    if let Ok(mut policy) = McpPolicy::load_default() {
        policy.mcp.listen = false;
        let _ = policy.save_default();
    }
    http_server::stop_http(http.inner().clone()).await;
    gateway::stop_gateway(gateway.inner().clone()).await;
    mcp_service_status(gateway, http).await
}

#[derive(Debug, Serialize)]
pub struct McpCheckItem {
    pub id: String,
    pub label: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct McpDiagnoseResult {
    pub ready: bool,
    pub policy_enabled: bool,
    pub gateway_running: bool,
    pub gateway_reachable: bool,
    pub http_running: bool,
    pub http_url: Option<String>,
    pub token_ready: bool,
    pub binary_path: String,
    pub binary_exists: bool,
    pub enabled_env_count: u32,
    pub checks: Vec<McpCheckItem>,
    pub howto: Vec<String>,
}

fn resolve_mcp_binary(preferred: &str) -> (String, bool) {
    let preferred = preferred.trim();
    // 优先：主应用自身（通过 `mcp` 子命令提供 stdio）
    if let Ok(exe) = std::env::current_exe() {
        if exe.is_file() {
            if preferred.is_empty()
                || preferred == "kube-flow-mcp"
                || preferred == exe.to_string_lossy().as_ref()
            {
                return (exe.display().to_string(), true);
            }
        }
    }
    let candidates: Vec<std::path::PathBuf> = {
        let mut list = Vec::new();
        if !preferred.is_empty() {
            list.push(std::path::PathBuf::from(preferred));
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                list.push(dir.join("kube-flow-mcp"));
                #[cfg(windows)]
                list.push(dir.join("kube-flow-mcp.exe"));
            }
        }
        list.push(std::path::PathBuf::from("kube-flow-mcp"));
        list
    };
    for path in candidates {
        if path.is_file() {
            return (path.display().to_string(), true);
        }
        if path.components().count() == 1 {
            if let Some(found) = which_binary(path.to_string_lossy().as_ref()) {
                return (found, true);
            }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        return (exe.display().to_string(), exe.is_file());
    }
    let fallback = if preferred.is_empty() {
        "kube-flow".to_string()
    } else {
        preferred.to_string()
    };
    (fallback, false)
}

fn which_binary(name: &str) -> Option<String> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p.display().to_string());
        }
        #[cfg(windows)]
        {
            let p_exe = dir.join(format!("{name}.exe"));
            if p_exe.is_file() {
                return Some(p_exe.display().to_string());
            }
        }
    }
    None
}

/// 诊断 MCP 就绪状态：策略、Gateway、HTTP 服务、令牌。
#[tauri::command]
pub async fn mcp_diagnose(
    gateway: State<'_, Arc<McpGatewayState>>,
    http: State<'_, Arc<McpHttpState>>,
    binary_path: Option<String>,
) -> CommandResult<McpDiagnoseResult> {
    let policy = McpPolicy::load_default().map_err(err_str)?;
    let policy_enabled = policy.mcp.enabled;
    let gateway_running = gateway.is_running().await;
    let http_running = http.is_running().await;
    let http_url = http.endpoint_url().await;
    let enabled_env_count = policy.bindings.iter().filter(|b| b.enabled).count() as u32;
    let token_ready = token::read_token().is_ok() || token::ensure_token().is_ok();
    let (binary_path, binary_exists) =
        resolve_mcp_binary(binary_path.as_deref().unwrap_or(""));

    let mut gateway_reachable = false;
    let mut gateway_detail = if gateway_running {
        "进程内标记为运行中".to_string()
    } else {
        "未启动".to_string()
    };
    if gateway_running && token_ready {
        if let Ok(tok) = token::read_token() {
            match crate::mcp::ipc_client::call_gateway(&tok, "ping", serde_json::json!({})).await {
                Ok(_) => {
                    gateway_reachable = true;
                    gateway_detail = "本地 IPC 探测成功".into();
                }
                Err(e) => {
                    gateway_detail = format!("已标记运行中，但探测失败: {e}");
                }
            }
        }
    }

    let http_detail = match (&http_url, http_running) {
        (Some(url), true) => format!("监听中：{url}"),
        (_, false) => "未启动：在设置中点击「启动 MCP 服务」".into(),
        (None, true) => "已标记运行中，但端口未知".into(),
    };

    let checks = vec![
        McpCheckItem {
            id: "policy".into(),
            label: "MCP 总开关".into(),
            ok: policy_enabled,
            detail: if policy_enabled {
                "已启用".into()
            } else {
                "未启用：打开开关或点击「启动 MCP 服务」".into()
            },
        },
        McpCheckItem {
            id: "http".into(),
            label: "HTTP MCP 服务".into(),
            ok: http_running,
            detail: http_detail,
        },
        McpCheckItem {
            id: "gateway_running".into(),
            label: "App Gateway".into(),
            ok: gateway_running,
            detail: gateway_detail.clone(),
        },
        McpCheckItem {
            id: "gateway_reachable".into(),
            label: "Gateway 可达性".into(),
            ok: gateway_reachable,
            detail: if gateway_reachable {
                "ping 成功".into()
            } else if gateway_running {
                gateway_detail
            } else {
                "Gateway 未运行，写操作与 SSH 只读会失败".into()
            },
        },
        McpCheckItem {
            id: "token".into(),
            label: "访问令牌".into(),
            ok: token_ready,
            detail: if token_ready {
                "mcp.token 已就绪".into()
            } else {
                "令牌缺失".into()
            },
        },
        McpCheckItem {
            id: "envs".into(),
            label: "已开放环境".into(),
            ok: enabled_env_count > 0,
            detail: if enabled_env_count > 0 {
                format!("{enabled_env_count} 个环境对 MCP 可见")
            } else {
                "尚未开放任何环境：在「按环境开放」中启用".into()
            },
        },
    ];

    // HTTP 路径不再要求 stdio 二进制；binary 仅作备选信息。
    let _ = binary_exists;
    let ready = checks.iter().all(|c| c.ok);
    let howto = vec![
        "1. 点击「启动 MCP 服务」（自动开启总开关、Gateway 与本机 HTTP）".into(),
        "2. 在「按环境开放」中至少开放一个环境".into(),
        "3. 在「客户端接入」复制 HTTP 配置到 Cursor / Claude Code / Codex".into(),
        "4. 保持 kube-flow App 运行；写操作仍会在本机弹出确认".into(),
        format!(
            "说明：推荐使用 HTTP（设置内启停）。stdio 备选：`{} mcp`。",
            binary_path
        ),
    ];

    Ok(McpDiagnoseResult {
        ready,
        policy_enabled,
        gateway_running,
        gateway_reachable,
        http_running,
        http_url,
        token_ready,
        binary_path,
        binary_exists,
        enabled_env_count,
        checks,
        howto,
    })
}

#[tauri::command]
pub fn mcp_resolve_binary(binary_path: Option<String>) -> CommandResult<(String, bool)> {
    Ok(resolve_mcp_binary(binary_path.as_deref().unwrap_or("")))
}

#[tauri::command]
pub async fn mcp_gateway_start(
    app: AppHandle,
    state: State<'_, Arc<McpGatewayState>>,
) -> CommandResult<()> {
    let mut policy = McpPolicy::load_default().map_err(err_str)?;
    policy.mcp.enabled = true;
    policy.mcp.listen = true;
    policy.save_default().map_err(err_str)?;
    let _ = token::ensure_token()?;
    gateway::start_gateway(app, state.inner().clone()).await
}

#[tauri::command]
pub async fn mcp_gateway_stop(state: State<'_, Arc<McpGatewayState>>) -> CommandResult<()> {
    gateway::stop_gateway(state.inner().clone()).await;
    Ok(())
}

#[tauri::command]
pub fn mcp_ensure_token() -> CommandResult<String> {
    token::ensure_token()
}

#[tauri::command]
pub fn mcp_regenerate_token() -> CommandResult<String> {
    token::regenerate_token()
}

#[tauri::command]
pub async fn mcp_approve(
    state: State<'_, Arc<McpGatewayState>>,
    request_id: String,
    grant_session: bool,
) -> CommandResult<()> {
    state
        .resolve_approval(
            &request_id,
            ApprovalDecision {
                approved: true,
                grant_session,
            },
        )
        .await;
    Ok(())
}

#[tauri::command]
pub async fn mcp_deny(
    state: State<'_, Arc<McpGatewayState>>,
    request_id: String,
) -> CommandResult<()> {
    state
        .resolve_approval(
            &request_id,
            ApprovalDecision {
                approved: false,
                grant_session: false,
            },
        )
        .await;
    Ok(())
}

#[tauri::command]
pub fn mcp_audit_recent(limit: Option<u32>) -> CommandResult<Vec<AuditEntry>> {
    audit::read_recent(limit.unwrap_or(50) as usize)
}

#[derive(Debug, Serialize)]
pub struct McpClientExport {
    pub id: String,
    pub label: String,
    pub config_path_hint: String,
    pub format: String,
    pub snippet: String,
    pub cli_hint: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct McpClientsExport {
    pub command: String,
    pub args: Vec<String>,
    pub token: String,
    pub transport: String,
    pub http_url: String,
    pub clients: Vec<McpClientExport>,
}

/// 导出 Cursor / Claude Code / Codex 的 MCP 接入片段（优先本机 HTTP）。
#[tauri::command]
pub async fn mcp_export_client_configs(
    http: State<'_, Arc<McpHttpState>>,
    binary_path: String,
) -> CommandResult<McpClientsExport> {
    let token = token::ensure_token()?;
    let (command, _) = resolve_mcp_binary(binary_path.trim());
    let args = vec!["mcp".to_string()];
    let url = http
        .endpoint_url()
        .await
        .unwrap_or_else(|| format!("http://127.0.0.1:{DEFAULT_HTTP_PORT}/mcp"));
    let auth_header = format!("Bearer {token}");

    let cursor_json = serde_json::json!({
        "mcpServers": {
            "kube-flow": {
                "url": url,
                "headers": {
                    "Authorization": auth_header.clone()
                }
            }
        }
    });

    let claude_json = serde_json::json!({
        "mcpServers": {
            "kube-flow": {
                "type": "http",
                "url": url,
                "headers": {
                    "Authorization": auth_header
                }
            }
        }
    });

    let url_toml = toml_escape(&url);
    let token_toml = toml_escape(&token);
    let codex_toml = format!(
        r#"[mcp_servers.kube-flow]
url = "{url_toml}"

[mcp_servers.kube-flow.http_headers]
Authorization = "Bearer {token_toml}"
"#
    );

    let claude_cli = format!(
        "claude mcp add --transport http kube-flow {} --header {}",
        shell_single_quote(&url),
        shell_single_quote(&format!("Authorization: Bearer {token}"))
    );
    let codex_cli = format!(
        "codex mcp add kube-flow --url {}",
        shell_single_quote(&url)
    );

    Ok(McpClientsExport {
        command: command.clone(),
        args: args.clone(),
        token: token.clone(),
        transport: "http".into(),
        http_url: url,
        clients: vec![
            McpClientExport {
                id: "cursor".into(),
                label: "Cursor".into(),
                config_path_hint: "~/.cursor/mcp.json 或项目 .cursor/mcp.json".into(),
                format: "json".into(),
                snippet: serde_json::to_string_pretty(&cursor_json).map_err(err_str)?,
                cli_hint: None,
            },
            McpClientExport {
                id: "claude_code".into(),
                label: "Claude Code".into(),
                config_path_hint: "项目 .mcp.json 或用户 ~/.claude.json".into(),
                format: "json".into(),
                snippet: serde_json::to_string_pretty(&claude_json).map_err(err_str)?,
                cli_hint: Some(claude_cli),
            },
            McpClientExport {
                id: "codex".into(),
                label: "Codex".into(),
                config_path_hint: "~/.codex/config.toml 或项目 .codex/config.toml".into(),
                format: "toml".into(),
                snippet: codex_toml,
                cli_hint: Some(codex_cli),
            },
        ],
    })
}

/// 兼容旧调用：仅返回 Cursor JSON 片段。
#[tauri::command]
pub async fn mcp_export_cursor_config(
    http: State<'_, Arc<McpHttpState>>,
    binary_path: String,
) -> CommandResult<McpClientsExport> {
    mcp_export_client_configs(http, binary_path).await
}

fn toml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}
