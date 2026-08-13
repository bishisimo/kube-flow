//! MCP stdio 进程调用 App Gateway 的客户端。

use super::protocol::{GatewayRequest, GatewayResponse};
use crate::config::mcp_gateway_port_path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

#[cfg(unix)]
use crate::config::mcp_gateway_sock_path;

pub async fn call_gateway(
    token: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let req = GatewayRequest {
        id: Uuid::new_v4().to_string(),
        token: token.to_string(),
        method: method.to_string(),
        params,
        approved: false,
        grant_session: false,
    };
    let line = serde_json::to_string(&req).map_err(|e| e.to_string())?;

    #[cfg(unix)]
    {
        if let Some(sock) = mcp_gateway_sock_path() {
            if sock.exists() {
                return call_unix(&sock, &line).await;
            }
        }
    }

    // TCP 回退（Windows 或 sock 不存在时尝试 port 文件）
    let port_path = mcp_gateway_port_path().ok_or_else(|| "gateway port path missing".to_string())?;
    if !port_path.exists() {
        return Err("AppOffline: kube-flow App Gateway 未在线".into());
    }
    let port: u16 = std::fs::read_to_string(&port_path)
        .map_err(|e| e.to_string())?
        .trim()
        .parse()
        .map_err(|_| "invalid gateway port".to_string())?;
    call_tcp(port, &line).await
}

#[cfg(unix)]
async fn call_unix(path: &std::path::Path, line: &str) -> Result<serde_json::Value, String> {
    let stream = tokio::net::UnixStream::connect(path)
        .await
        .map_err(|e| format!("AppOffline: {e}"))?;
    exchange(stream, line).await
}

async fn call_tcp(port: u16, line: &str) -> Result<serde_json::Value, String> {
    let stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .map_err(|e| format!("AppOffline: {e}"))?;
    exchange(stream, line).await
}

async fn exchange<S>(stream: S, line: &str) -> Result<serde_json::Value, String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (reader, mut writer) = tokio::io::split(stream);
    writer
        .write_all(line.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    writer.write_all(b"\n").await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;

    let mut lines = BufReader::new(reader).lines();
    let resp_line = lines
        .next_line()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "empty gateway response".to_string())?;
    let resp: GatewayResponse =
        serde_json::from_str(&resp_line).map_err(|e| e.to_string())?;
    if resp.ok {
        Ok(resp.result.unwrap_or(serde_json::Value::Null))
    } else {
        let err = resp
            .error
            .map(|e| format!("{}: {}", e.code, e.message))
            .unwrap_or_else(|| "unknown error".into());
        Err(err)
    }
}

pub fn gateway_seems_online() -> bool {
    #[cfg(unix)]
    {
        if let Some(sock) = mcp_gateway_sock_path() {
            if sock.exists() {
                return true;
            }
        }
    }
    mcp_gateway_port_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}
