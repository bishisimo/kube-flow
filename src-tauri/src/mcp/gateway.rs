//! App 内 MCP Gateway：本地 IPC + token 校验 + 确认桥。

use super::audit;
use super::capabilities::Capability;
use super::gate::{self, GateDeny, GateRequest};
use super::policy::McpPolicy;
use super::protocol::{
    ApprovalRequestPayload, GatewayRequest, GatewayResponse, APPROVAL_EVENT,
};
use super::session_grant::SessionGrantStore;
use super::token;
use super::write_executor::{self, capability_for_method, meta_from_yaml};
use crate::config::{ensure_app_data_dir, mcp_gateway_port_path};
use crate::kube::KubeClientStore;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{oneshot, Mutex, RwLock};

#[cfg(unix)]
use crate::config::mcp_gateway_sock_path;
#[cfg(unix)]
use tokio::net::UnixListener;

#[cfg(not(unix))]
use tokio::net::TcpListener;

/// 待确认请求。
struct PendingApproval {
    tx: oneshot::Sender<ApprovalDecision>,
}

#[derive(Debug, Clone)]
pub struct ApprovalDecision {
    pub approved: bool,
    pub grant_session: bool,
}

pub struct McpGatewayState {
    pub grants: SessionGrantStore,
    pending: Mutex<HashMap<String, PendingApproval>>,
    running: RwLock<bool>,
    abort: Mutex<Option<tokio::task::AbortHandle>>,
}

impl McpGatewayState {
    pub fn new() -> Self {
        Self {
            grants: SessionGrantStore::new(),
            pending: Mutex::new(HashMap::new()),
            running: RwLock::new(false),
            abort: Mutex::new(None),
        }
    }

    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    pub async fn resolve_approval(&self, request_id: &str, decision: ApprovalDecision) {
        let mut guard = self.pending.lock().await;
        if let Some(p) = guard.remove(request_id) {
            let _ = p.tx.send(decision);
        }
    }
}

/// 根据策略启停 Gateway。
pub async fn sync_gateway(app: &AppHandle) -> Result<(), String> {
    let policy = McpPolicy::load_default().map_err(|e| e.to_string())?;
    let state = app.state::<Arc<McpGatewayState>>();
    if policy.mcp.enabled && policy.mcp.listen {
        start_gateway(app.clone(), state.inner().clone()).await
    } else {
        stop_gateway(state.inner().clone()).await;
        Ok(())
    }
}

/// 根据策略同步 Gateway 与本机 HTTP MCP。
pub async fn sync_mcp_services(app: &AppHandle) -> Result<(), String> {
    sync_gateway(app).await?;
    let policy = McpPolicy::load_default().map_err(|e| e.to_string())?;
    let http = app.state::<Arc<crate::mcp::http_server::McpHttpState>>();
    if policy.mcp.enabled && policy.mcp.listen {
        let _ = crate::mcp::http_server::start_http(http.inner().clone()).await?;
    } else {
        crate::mcp::http_server::stop_http(http.inner().clone()).await;
    }
    Ok(())
}

pub async fn start_gateway(app: AppHandle, state: Arc<McpGatewayState>) -> Result<(), String> {
    if *state.running.read().await {
        return Ok(());
    }
    ensure_app_data_dir().ok_or_else(|| "app data dir unavailable".to_string())?;
    let _ = token::ensure_token()?;

    let handle = {
        let app2 = app.clone();
        let state2 = state.clone();
        tokio::spawn(async move {
            if let Err(e) = run_listener(app2, state2.clone()).await {
                eprintln!("[mcp-gateway] stopped: {e}");
            }
            *state2.running.write().await = false;
        })
        .abort_handle()
    };

    {
        let mut abort = state.abort.lock().await;
        *abort = Some(handle);
    }
    *state.running.write().await = true;
    Ok(())
}

pub async fn stop_gateway(state: Arc<McpGatewayState>) {
    if let Some(h) = state.abort.lock().await.take() {
        h.abort();
    }
    *state.running.write().await = false;
    cleanup_listen_files();
}

fn cleanup_listen_files() {
    #[cfg(unix)]
    if let Some(p) = mcp_gateway_sock_path() {
        let _ = std::fs::remove_file(p);
    }
    if let Some(p) = mcp_gateway_port_path() {
        let _ = std::fs::remove_file(p);
    }
}

async fn run_listener(app: AppHandle, state: Arc<McpGatewayState>) -> Result<(), String> {
    #[cfg(unix)]
    {
        let sock = mcp_gateway_sock_path().ok_or_else(|| "sock path unavailable".to_string())?;
        let _ = std::fs::remove_file(&sock);
        let listener = UnixListener::bind(&sock).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&sock, std::fs::Permissions::from_mode(0o600));
        }
        loop {
            let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
            let app2 = app.clone();
            let state2 = state.clone();
            tokio::spawn(async move {
                let _ = handle_connection_unix(stream, app2, state2).await;
            });
        }
    }
    #[cfg(not(unix))]
    {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let port_path =
            mcp_gateway_port_path().ok_or_else(|| "port path unavailable".to_string())?;
        std::fs::write(&port_path, port.to_string()).map_err(|e| e.to_string())?;
        loop {
            let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
            let app2 = app.clone();
            let state2 = state.clone();
            tokio::spawn(async move {
                let _ = handle_connection_tcp(stream, app2, state2).await;
            });
        }
    }
}

#[cfg(unix)]
async fn handle_connection_unix(
    stream: tokio::net::UnixStream,
    app: AppHandle,
    state: Arc<McpGatewayState>,
) -> Result<(), String> {
    let (reader, writer) = stream.into_split();
    handle_rw(reader, writer, app, state).await
}

#[cfg(not(unix))]
async fn handle_connection_tcp(
    stream: tokio::net::TcpStream,
    app: AppHandle,
    state: Arc<McpGatewayState>,
) -> Result<(), String> {
    let (reader, writer) = stream.into_split();
    handle_rw(reader, writer, app, state).await
}

async fn handle_rw<R, W>(
    reader: R,
    mut writer: W,
    app: AppHandle,
    state: Arc<McpGatewayState>,
) -> Result<(), String>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
        if line.trim().is_empty() {
            continue;
        }
        let resp = match serde_json::from_str::<GatewayRequest>(&line) {
            Ok(req) => process_request(&app, &state, req).await,
            Err(e) => GatewayResponse::failure("?", "bad_request", e.to_string()),
        };
        let out = serde_json::to_string(&resp).map_err(|e| e.to_string())?;
        writer
            .write_all(out.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        writer.write_all(b"\n").await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn process_request(
    app: &AppHandle,
    state: &Arc<McpGatewayState>,
    req: GatewayRequest,
) -> GatewayResponse {
    if let Err(e) = token::validate_token(&req.token) {
        return GatewayResponse::failure(&req.id, "invalid_token", e);
    }

    let policy = match McpPolicy::load_default() {
        Ok(p) => p,
        Err(e) => return GatewayResponse::failure(&req.id, "policy_error", e.to_string()),
    };
    if !policy.mcp.enabled {
        return GatewayResponse::failure(&req.id, "mcp_disabled", GateDeny::McpDisabled.to_string());
    }

    // 健康探测：不要求 capability，仅校验 token + Gateway 可达。
    if req.method == "ping" {
        return GatewayResponse::success(
            &req.id,
            json!({
                "ok": true,
                "service": "kube-flow-mcp-gateway",
            }),
        );
    }

    let Some(cap) = capability_for_method(&req.method) else {
        return GatewayResponse::failure(&req.id, "unknown_method", format!("unknown: {}", req.method));
    };

    if req.method == "list_environments" {
        let ids: Vec<String> = policy
            .bindings
            .iter()
            .filter(|b| b.enabled)
            .map(|b| b.env_id.clone())
            .collect();
        match crate::mcp::read_executor::list_environment_summaries(&ids) {
            Ok(list) => {
                audit::record(None, &req.method, cap.as_str(), true, None, false, "gateway");
                return GatewayResponse::success(&req.id, json!({ "environments": list }));
            }
            Err(e) => {
                audit::record(None, &req.method, cap.as_str(), false, Some(&e), false, "gateway");
                return GatewayResponse::failure(&req.id, "exec_error", e);
            }
        }
    }

    let env_id = req
        .params
        .get("env_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let (kind, namespace) = extract_scope(&req.method, &req.params);

    let mut approved = req.approved;
    if let (Some(eid), true) = (env_id.as_deref(), !approved) {
        if state.grants.has_valid(eid, cap) {
            approved = true;
        }
    }

    let gate_req = GateRequest {
        env_id: env_id.as_deref(),
        capability: cap,
        namespace: namespace.as_deref(),
        kind: kind.as_deref(),
        via_stdio: false,
        approved,
    };

    match gate::evaluate(&policy, &gate_req) {
        Ok(_) => {}
        Err(GateDeny::ApprovalRequired { .. }) => {
            match request_user_approval(app, state, &req, &policy, env_id.as_deref().unwrap_or(""), cap)
                .await
            {
                Ok(decision) if decision.approved => {
                    if decision.grant_session {
                        if let Some(eid) = env_id.as_deref() {
                            let _ = state.grants.grant(
                                eid,
                                cap,
                                policy.mcp.session_grant_minutes,
                            );
                        }
                    }
                }
                Ok(_) => {
                    audit::record(
                        env_id.as_deref(),
                        &req.method,
                        cap.as_str(),
                        false,
                        Some("approval denied"),
                        false,
                        "gateway",
                    );
                    return GatewayResponse::failure(
                        &req.id,
                        "approval_denied",
                        GateDeny::ApprovalDenied.to_string(),
                    );
                }
                Err(e) => {
                    return GatewayResponse::failure(&req.id, "approval_error", e);
                }
            }
        }
        Err(e) => {
            audit::record(
                env_id.as_deref(),
                &req.method,
                cap.as_str(),
                false,
                Some(&e.to_string()),
                approved,
                "gateway",
            );
            return GatewayResponse::failure(&req.id, gate_code(&e), e.to_string());
        }
    }

    let store = app.state::<KubeClientStore>();
    match write_executor::handle_method(store.inner(), app, &req.method, &req.params).await {
        Ok(result) => {
            audit::record(
                env_id.as_deref(),
                &req.method,
                cap.as_str(),
                true,
                None,
                true,
                "gateway",
            );
            GatewayResponse::success(&req.id, result)
        }
        Err(e) => {
            audit::record(
                env_id.as_deref(),
                &req.method,
                cap.as_str(),
                false,
                Some(&e),
                true,
                "gateway",
            );
            GatewayResponse::failure(&req.id, "exec_error", e)
        }
    }
}

async fn request_user_approval(
    app: &AppHandle,
    state: &Arc<McpGatewayState>,
    req: &GatewayRequest,
    policy: &McpPolicy,
    env_id: &str,
    cap: Capability,
) -> Result<ApprovalDecision, String> {
    let force = cap.requires_forced_approval();
    let can_grant = !force;
    let (tx, rx) = oneshot::channel();
    let request_id = req.id.clone();
    {
        let mut pending = state.pending.lock().await;
        pending.insert(request_id.clone(), PendingApproval { tx });
    }
    let summary = summarize_request(req);
    let payload = ApprovalRequestPayload {
        request_id: request_id.clone(),
        env_id: env_id.to_string(),
        capability: cap.as_str().to_string(),
        method: req.method.clone(),
        summary,
        force,
        can_grant_session: can_grant,
    };
    app.emit(APPROVAL_EVENT, payload)
        .map_err(|e| e.to_string())?;

    let timeout_secs = if force { 300 } else { 120 };
    match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), rx).await {
        Ok(Ok(decision)) => Ok(decision),
        Ok(Err(_)) => Err("approval channel closed".into()),
        Err(_) => {
            state.pending.lock().await.remove(&request_id);
            let _ = policy; // silence
            Err("approval timed out".into())
        }
    }
}

fn summarize_request(req: &GatewayRequest) -> String {
    format!("{} {}", req.method, req.params)
}

fn extract_scope(method: &str, params: &Value) -> (Option<String>, Option<String>) {
    if matches!(method, "apply_resource_yaml" | "create_or_deploy") {
        if let Some(yaml) = params.get("yaml").and_then(|v| v.as_str()) {
            let (kind, ns, _) = meta_from_yaml(yaml);
            return (kind, ns);
        }
    }
    let kind = params
        .get("kind")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let ns = params
        .get("namespace")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    (kind, ns)
}

fn gate_code(d: &GateDeny) -> &'static str {
    match d {
        GateDeny::McpDisabled => "mcp_disabled",
        GateDeny::EnvNotVisible { .. } => "env_not_visible",
        GateDeny::CapabilityDenied { .. } => "capability_denied",
        GateDeny::NamespaceDenied { .. } => "namespace_denied",
        GateDeny::KindDenied { .. } => "kind_denied",
        GateDeny::SecretDenied => "secret_denied",
        GateDeny::AppChannelRequired { .. } => "app_required",
        GateDeny::ApprovalRequired { .. } => "approval_required",
        GateDeny::ApprovalDenied => "approval_denied",
        GateDeny::AppOffline => "app_offline",
        GateDeny::InvalidToken => "invalid_token",
    }
}
