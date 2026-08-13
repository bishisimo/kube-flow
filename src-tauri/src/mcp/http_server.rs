//! App 内本机 HTTP MCP 服务：设置页启停，客户端通过 URL 接入。

use super::server;
use super::token;
use crate::config::{ensure_app_data_dir, mcp_http_port_path};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};

/// 默认本机 HTTP MCP 端口（固定以便客户端配置可复用）。
pub const DEFAULT_HTTP_PORT: u16 = 19527;

/// HTTP MCP 运行时状态。
pub struct McpHttpState {
    running: RwLock<bool>,
    port: RwLock<Option<u16>>,
    abort: Mutex<Option<tokio::task::AbortHandle>>,
}

impl McpHttpState {
    pub fn new() -> Self {
        Self {
            running: RwLock::new(false),
            port: RwLock::new(None),
            abort: Mutex::new(None),
        }
    }

    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    pub async fn port(&self) -> Option<u16> {
        *self.port.read().await
    }

    pub async fn endpoint_url(&self) -> Option<String> {
        self.port()
            .await
            .map(|p| format!("http://127.0.0.1:{p}/mcp"))
    }
}

impl Default for McpHttpState {
    fn default() -> Self {
        Self::new()
    }
}

/// 供设置页与诊断使用的服务快照。
#[derive(Debug, Clone, Serialize)]
pub struct McpServiceStatus {
    pub gateway_running: bool,
    pub http_running: bool,
    pub http_port: Option<u16>,
    pub http_url: Option<String>,
    pub token_ready: bool,
    pub policy_enabled: bool,
}

/// 启动本机 HTTP MCP（127.0.0.1）。若已在运行则直接返回。
pub async fn start_http(state: Arc<McpHttpState>) -> Result<u16, String> {
    if *state.running.read().await {
        return state
            .port()
            .await
            .ok_or_else(|| "http mcp marked running but port missing".to_string());
    }
    ensure_app_data_dir().ok_or_else(|| "app data dir unavailable".to_string())?;
    let _ = token::ensure_token()?;

    let listener = bind_listener().await?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    write_port_file(port)?;

    let handle = {
        let state2 = state.clone();
        tokio::spawn(async move {
            if let Err(e) = run_accept_loop(listener).await {
                eprintln!("[mcp-http] stopped: {e}");
            }
            *state2.running.write().await = false;
            *state2.port.write().await = None;
            cleanup_port_file();
        })
        .abort_handle()
    };

    {
        let mut abort = state.abort.lock().await;
        *abort = Some(handle);
    }
    *state.port.write().await = Some(port);
    *state.running.write().await = true;
    Ok(port)
}

/// 停止本机 HTTP MCP。
pub async fn stop_http(state: Arc<McpHttpState>) {
    if let Some(h) = state.abort.lock().await.take() {
        h.abort();
    }
    *state.running.write().await = false;
    *state.port.write().await = None;
    cleanup_port_file();
}

async fn bind_listener() -> Result<TcpListener, String> {
    match TcpListener::bind(("127.0.0.1", DEFAULT_HTTP_PORT)).await {
        Ok(l) => Ok(l),
        Err(_) => TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("bind mcp http failed: {e}")),
    }
}

fn write_port_file(port: u16) -> Result<(), String> {
    let path = mcp_http_port_path().ok_or_else(|| "http port path unavailable".to_string())?;
    std::fs::write(&path, port.to_string()).map_err(|e| e.to_string())
}

fn cleanup_port_file() {
    if let Some(p) = mcp_http_port_path() {
        let _ = std::fs::remove_file(p);
    }
}

async fn run_accept_loop(listener: TcpListener) -> Result<(), String> {
    loop {
        let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
        tokio::spawn(async move {
            let _ = handle_connection(stream).await;
        });
    }
}

async fn handle_connection(mut stream: tokio::net::TcpStream) -> Result<(), String> {
    let mut buf = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut tmp).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find_header_end(&buf) {
            break pos;
        }
        if buf.len() > 64 * 1024 {
            return write_response(&mut stream, 413, "text/plain", b"headers too large").await;
        }
    };

    let header_bytes = &buf[..header_end];
    let header_str = String::from_utf8_lossy(header_bytes);
    let mut lines = header_str.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_uppercase();
    let path = parts.next().unwrap_or("/");

    let mut headers = HashMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }

    let content_length = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > 8 * 1024 * 1024 {
        return write_response(&mut stream, 413, "text/plain", b"body too large").await;
    }

    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut tmp).await.map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(content_length);

    match (method.as_str(), path) {
        ("GET", "/health") | ("GET", "/mcp/health") => {
            let body = br#"{"ok":true,"service":"kube-flow-mcp-http"}"#;
            write_response(&mut stream, 200, "application/json", body).await
        }
        ("GET", "/mcp") => {
            // Streamable HTTP：无会话时返回服务信息；客户端以 POST 为主。
            let body = br#"{"ok":true,"transport":"streamable-http","endpoint":"/mcp"}"#;
            write_response(&mut stream, 200, "application/json", body).await
        }
        ("DELETE", "/mcp") => write_response(&mut stream, 200, "application/json", b"{}").await,
        ("POST", "/mcp") => handle_mcp_post(&mut stream, &headers, &body).await,
        ("OPTIONS", _) => {
            write_raw(
                &mut stream,
                "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Authorization, Content-Type, Mcp-Session-Id, Accept\r\nAccess-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\nContent-Length: 0\r\n\r\n",
            )
            .await
        }
        _ => write_response(&mut stream, 404, "text/plain", b"not found").await,
    }
}

async fn handle_mcp_post(
    stream: &mut tokio::net::TcpStream,
    headers: &HashMap<String, String>,
    body: &[u8],
) -> Result<(), String> {
    if let Err(e) = authorize(headers) {
        return write_response(stream, 401, "application/json", format_err_body(&e).as_bytes())
            .await;
    }

    let body_str = std::str::from_utf8(body).map_err(|e| e.to_string())?;
    if body_str.trim().is_empty() {
        return write_response(stream, 400, "application/json", br#"{"error":"empty body"}"#).await;
    }

    match server::handle_jsonrpc_body(body_str).await {
        Ok(None) => write_response(stream, 202, "application/json", b"").await,
        Ok(Some(resp)) => {
            let payload = serde_json::to_vec(&resp).map_err(|e| e.to_string())?;
            let session = headers
                .get("mcp-session-id")
                .cloned()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            write_mcp_json(stream, 200, &payload, &session).await
        }
        Err(e) => {
            let payload = serde_json::json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32700, "message": e }
            });
            let bytes = serde_json::to_vec(&payload).map_err(|err| err.to_string())?;
            write_response(stream, 400, "application/json", &bytes).await
        }
    }
}

fn authorize(headers: &HashMap<String, String>) -> Result<(), String> {
    if let Some(auth) = headers.get("authorization") {
        let token = auth
            .strip_prefix("Bearer ")
            .or_else(|| auth.strip_prefix("bearer "))
            .unwrap_or(auth.as_str())
            .trim();
        return token::validate_token(token);
    }
    if let Some(token) = headers.get("x-kube-flow-mcp-token") {
        return token::validate_token(token.trim());
    }
    Err("missing Authorization bearer token".into())
}

fn format_err_body(msg: &str) -> String {
    serde_json::json!({ "error": msg }).to_string()
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

async fn write_mcp_json(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    body: &[u8],
    session_id: &str,
) -> Result<(), String> {
    let reason = status_reason(status);
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nMcp-Session-Id: {session_id}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    stream.write_all(body).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn write_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    let reason = status_reason(status);
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    if !body.is_empty() {
        stream.write_all(body).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn write_raw(stream: &mut tokio::net::TcpStream, raw: &str) -> Result<(), String> {
    stream
        .write_all(raw.as_bytes())
        .await
        .map_err(|e| e.to_string())
}

fn status_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Error",
    }
}
