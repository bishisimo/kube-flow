//! 主机文件传输：基于 russh + russh-sftp 的异步流式上传/下载，支持进度与取消。

use crate::config::{ssh_config_get_host_config, ssh_config_resolve_proxy_command, SshHostConfig};
use crate::credentials::AuthMethod;
use crate::kube::file_transfer::{emit_progress, ProgressEmitter};
use russh::client::{self, AuthResult, Handle};
use russh::keys::{self, load_secret_key, PrivateKeyWithHashAlg, PublicKey};
use russh_sftp::client::SftpSession;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tauri::AppHandle;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

const CHUNK_SIZE: usize = 64 * 1024;

fn err_ssh(e: impl std::fmt::Display) -> String {
    format!("SFTP: {}", e)
}

struct ClientHandler;

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        // 与原先 libssh2 文件传输一致：不校验 known_hosts。
        Ok(true)
    }
}

/// 将 ProxyCommand 子进程的 stdin/stdout 合成 russh 可用的双工流。
struct ProxyStream {
    stdout: ChildStdout,
    stdin: ChildStdin,
    _child: Child,
}

impl AsyncRead for ProxyStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

impl AsyncWrite for ProxyStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.stdin).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdin).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdin).poll_shutdown(cx)
    }
}

#[cfg(unix)]
async fn connect_via_proxy(proxy_args: Vec<String>) -> Result<ProxyStream, String> {
    let (exe, args) = proxy_args
        .split_first()
        .ok_or_else(|| "ProxyCommand 为空".to_string())?;

    let mut child = Command::new(exe)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("启动 ProxyCommand 失败: {}", e))?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "无法获取 ProxyCommand stdin".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法获取 ProxyCommand stdout".to_string())?;

    Ok(ProxyStream {
        stdout,
        stdin,
        _child: child,
    })
}

async fn connect_session(host_config: &SshHostConfig) -> Result<Handle<ClientHandler>, String> {
    let config = Arc::new(client::Config::default());
    let handler = ClientHandler;

    #[cfg(unix)]
    {
        if let Some(proxy_args) = ssh_config_resolve_proxy_command(host_config) {
            let stream = connect_via_proxy(proxy_args).await?;
            return client::connect_stream(config, stream, handler)
                .await
                .map_err(err_ssh);
        }
    }

    #[cfg(not(unix))]
    {
        if ssh_config_resolve_proxy_command(host_config).is_some() {
            return Err("ProxyCommand/ProxyJump 仅支持 Unix 平台".to_string());
        }
    }

    client::connect(
        config,
        (host_config.hostname.as_str(), host_config.port),
        handler,
    )
    .await
    .map_err(|e| {
        format!(
            "TCP 连接 {}:{} 失败: {}",
            host_config.hostname, host_config.port, e
        )
    })
}

fn identity_candidates(host_config: &SshHostConfig) -> Vec<PathBuf> {
    let mut keys = Vec::new();
    if let Some(ref key_path) = host_config.identity_file {
        if key_path.exists() {
            keys.push(key_path.clone());
        }
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    for name in ["id_ed25519", "id_rsa", "id_ecdsa"] {
        let key = home.join(".ssh").join(name);
        if key.exists() && !keys.iter().any(|k| k == &key) {
            keys.push(key);
        }
    }
    keys
}

async fn try_pubkey(
    session: &mut Handle<ClientHandler>,
    user: &str,
    host_config: &SshHostConfig,
) -> bool {
    let hash_alg = session
        .best_supported_rsa_hash()
        .await
        .ok()
        .and_then(|v| v.flatten());

    for path in identity_candidates(host_config) {
        let Ok(key) = load_secret_key(&path, None) else {
            continue;
        };
        let key = PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg);
        match session.authenticate_publickey(user, key).await {
            Ok(AuthResult::Success) => return true,
            _ => continue,
        }
    }
    false
}

async fn try_agent(session: &mut Handle<ClientHandler>, user: &str) -> bool {
    #[cfg(unix)]
    {
        let Ok(mut agent) = keys::agent::client::AgentClient::connect_env().await else {
            return false;
        };
        let Ok(identities) = agent.request_identities().await else {
            return false;
        };
        let hash_alg = session
            .best_supported_rsa_hash()
            .await
            .ok()
            .and_then(|v| v.flatten());
        for identity in identities {
            match session
                .authenticate_publickey_with(user, identity, hash_alg, &mut agent)
                .await
            {
                Ok(AuthResult::Success) => return true,
                _ => continue,
            }
        }
        false
    }
    #[cfg(not(unix))]
    {
        let _ = (session, user);
        false
    }
}

async fn try_password(
    session: &mut Handle<ClientHandler>,
    user: &str,
    password: Option<&str>,
) -> bool {
    let Some(pwd) = password.filter(|p| !p.is_empty()) else {
        return false;
    };
    matches!(
        session.authenticate_password(user, pwd).await,
        Ok(AuthResult::Success)
    )
}

async fn authenticate_session(
    session: &mut Handle<ClientHandler>,
    host_config: &SshHostConfig,
    auth_method: AuthMethod,
    password: Option<&str>,
) -> Result<(), String> {
    let user = host_config.user.as_str();

    match auth_method {
        AuthMethod::PublicKey => {
            if try_pubkey(session, user, host_config).await || try_agent(session, user).await {
                return Ok(());
            }
        }
        AuthMethod::Password | AuthMethod::KeyboardInteractive => {
            if try_password(session, user, password).await {
                return Ok(());
            }
            if try_pubkey(session, user, host_config).await || try_agent(session, user).await {
                return Ok(());
            }
        }
        AuthMethod::Auto => {
            if try_agent(session, user).await
                || try_pubkey(session, user, host_config).await
                || try_password(session, user, password).await
            {
                return Ok(());
            }
        }
    }

    Err(
        "SSH 认证失败。请确认 ~/.ssh/config IdentityFile / ssh-agent，或在环境中保存密码凭证。"
            .to_string(),
    )
}

async fn resolve_remote_path(sftp: &SftpSession, remote_path: &str) -> Result<String, String> {
    let path = remote_path.trim();
    if path.is_empty() {
        return Err("远端路径不能为空".to_string());
    }
    if path == "~" || path.starts_with("~/") || path.starts_with('~') {
        match sftp.canonicalize(path).await {
            Ok(resolved) => Ok(resolved),
            Err(_) if path.starts_with("~/") => {
                let home = sftp
                    .canonicalize(".")
                    .await
                    .map_err(|e| format!("解析远端 HOME 失败: {}", e))?;
                Ok(format!(
                    "{}/{}",
                    home.trim_end_matches('/'),
                    &path[2..]
                ))
            }
            Err(e) => Err(format!("解析远端路径失败: {}", e)),
        }
    } else {
        Ok(path.to_string())
    }
}

fn check_cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::SeqCst) {
        Err("已取消".to_string())
    } else {
        Ok(())
    }
}

/// 建立已认证的 SSH 会话并打开 SFTP。
async fn open_sftp(
    ssh_host: &str,
    auth_method: AuthMethod,
    password: Option<&str>,
) -> Result<(Handle<ClientHandler>, SftpSession), String> {
    let host_config = ssh_config_get_host_config(ssh_host)
        .ok_or_else(|| format!("~/.ssh/config 中未找到 Host: {}", ssh_host))?;
    let mut session = connect_session(&host_config).await?;
    authenticate_session(&mut session, &host_config, auth_method, password).await?;
    let channel = session
        .channel_open_session()
        .await
        .map_err(err_ssh)?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(err_ssh)?;
    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(err_ssh)?;
    Ok((session, sftp))
}

/// 通过 SFTP 上传本地文件到远端，并报告字节进度。
pub async fn sftp_upload(
    app: &AppHandle,
    transfer_id: &str,
    cancel: &AtomicBool,
    ssh_host: &str,
    auth_method: AuthMethod,
    password: Option<&str>,
    local_path: &Path,
    remote_path: &str,
    overwrite: bool,
) -> Result<(), String> {
    check_cancelled(cancel)?;
    if !local_path.is_file() {
        return Err(format!(
            "本地文件不存在或不是文件: {}",
            local_path.display()
        ));
    }
    let total_bytes = tokio::fs::metadata(local_path)
        .await
        .map_err(|e| format!("读取本地文件失败: {}", e))?
        .len();

    let (_session, sftp) = open_sftp(ssh_host, auth_method, password).await?;
    check_cancelled(cancel)?;
    let remote = resolve_remote_path(&sftp, remote_path).await?;

    if !overwrite {
        match sftp.try_exists(&remote).await {
            Ok(true) => return Err(format!("远端文件已存在: {}", remote)),
            Ok(false) => {}
            Err(e) => return Err(format!("检查远端文件失败: {}", e)),
        }
    }

    let mut local = tokio::fs::File::open(local_path)
        .await
        .map_err(|e| format!("打开本地文件失败: {}", e))?;
    let mut remote_file = sftp
        .create(&remote)
        .await
        .map_err(|e| format!("创建远端文件失败: {}", e))?;

    emit_progress(app, transfer_id, 0, Some(total_bytes));
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut transferred = 0u64;
    let mut ticker = ProgressEmitter::new();
    loop {
        check_cancelled(cancel)?;
        let n = local
            .read(&mut buf)
            .await
            .map_err(|e| format!("读取本地文件失败: {}", e))?;
        if n == 0 {
            break;
        }
        remote_file
            .write_all(&buf[..n])
            .await
            .map_err(|e| format!("写入远端失败: {}", e))?;
        transferred += n as u64;
        ticker.emit(app, transfer_id, transferred, Some(total_bytes));
    }
    remote_file
        .flush()
        .await
        .map_err(|e| format!("刷新远端文件失败: {}", e))?;
    emit_progress(app, transfer_id, transferred, Some(total_bytes));
    Ok(())
}

/// 通过 SFTP 从远端下载文件到本地，并报告字节进度。
pub async fn sftp_download(
    app: &AppHandle,
    transfer_id: &str,
    cancel: &AtomicBool,
    ssh_host: &str,
    auth_method: AuthMethod,
    password: Option<&str>,
    remote_path: &str,
    local_path: &Path,
    overwrite: bool,
) -> Result<(), String> {
    check_cancelled(cancel)?;
    if local_path.exists() && !overwrite {
        return Err(format!("本地文件已存在: {}", local_path.display()));
    }
    if let Some(parent) = local_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }

    let (_session, sftp) = open_sftp(ssh_host, auth_method, password).await?;
    check_cancelled(cancel)?;
    let remote = resolve_remote_path(&sftp, remote_path).await?;

    let mut remote_file = sftp
        .open(&remote)
        .await
        .map_err(|e| format!("打开远端文件失败: {}", e))?;
    let total_bytes = remote_file
        .metadata()
        .await
        .ok()
        .and_then(|m| m.size);

    let mut local = tokio::fs::File::create(local_path)
        .await
        .map_err(|e| format!("创建本地文件失败: {}", e))?;

    emit_progress(app, transfer_id, 0, total_bytes);
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut transferred = 0u64;
    let mut ticker = ProgressEmitter::new();
    loop {
        check_cancelled(cancel)?;
        let n = remote_file
            .read(&mut buf)
            .await
            .map_err(|e| format!("读取远端文件失败: {}", e))?;
        if n == 0 {
            break;
        }
        local
            .write_all(&buf[..n])
            .await
            .map_err(|e| format!("写入本地文件失败: {}", e))?;
        transferred += n as u64;
        ticker.emit(app, transfer_id, transferred, total_bytes);
    }
    local
        .flush()
        .await
        .map_err(|e| format!("刷新本地文件失败: {}", e))?;
    let final_total = total_bytes.or(Some(transferred));
    emit_progress(app, transfer_id, transferred, final_total);
    Ok(())
}
