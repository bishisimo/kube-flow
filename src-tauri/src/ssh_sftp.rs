//! 主机文件传输：基于 libssh2 SFTP 的流式上传/下载，支持进度与取消。

use crate::config::{ssh_config_get_host_config, ssh_config_resolve_proxy_command, SshHostConfig};
use crate::credentials::AuthMethod;
use crate::kube::file_transfer::{emit_progress, ProgressEmitter};
use ssh2::Session;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::AppHandle;

const CHUNK_SIZE: usize = 64 * 1024;

fn err_ssh(e: impl std::fmt::Display) -> String {
    format!("SFTP: {}", e)
}

/// 通过 ProxyCommand 建立连接；仅 Unix。
#[cfg(unix)]
fn connect_via_proxy(
    proxy_args: Vec<String>,
) -> Result<std::os::unix::net::UnixStream, String> {
    use std::io::{Read as _, Write as _};
    use std::os::unix::net::UnixStream;
    use std::process::{Command, Stdio};
    use std::thread;

    let (mut proxy_end, libssh2_end) =
        UnixStream::pair().map_err(|e| format!("创建 ProxyCommand 通道失败: {}", e))?;

    let (exe, args) = proxy_args
        .split_first()
        .ok_or_else(|| "ProxyCommand 为空".to_string())?;

    let mut child = Command::new(exe)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("启动 ProxyCommand 失败: {}", e))?;

    let mut child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| "无法获取 ProxyCommand stdin".to_string())?;
    let mut child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法获取 ProxyCommand stdout".to_string())?;

    let mut proxy_end_clone = proxy_end
        .try_clone()
        .map_err(|e| format!("克隆 ProxyCommand 通道失败: {}", e))?;

    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match child_stdout.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if proxy_end.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match proxy_end_clone.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if child_stdin.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    libssh2_end
        .set_read_timeout(Some(Duration::from_secs(30)))
        .ok();
    libssh2_end
        .set_write_timeout(Some(Duration::from_secs(30)))
        .ok();
    Ok(libssh2_end)
}

fn connect_session(host_config: &SshHostConfig) -> Result<Session, String> {
    let mut sess = Session::new().map_err(err_ssh)?;
    sess.set_timeout(30_000);

    #[cfg(unix)]
    {
        if let Some(proxy_args) = ssh_config_resolve_proxy_command(host_config) {
            let stream = connect_via_proxy(proxy_args)?;
            sess.set_tcp_stream(stream);
        } else {
            let tcp = TcpStream::connect((host_config.hostname.as_str(), host_config.port))
                .map_err(|e| {
                    format!(
                        "TCP 连接 {}:{} 失败: {}",
                        host_config.hostname, host_config.port, e
                    )
                })?;
            tcp.set_read_timeout(Some(Duration::from_secs(30))).ok();
            tcp.set_write_timeout(Some(Duration::from_secs(30))).ok();
            sess.set_tcp_stream(tcp);
        }
    }

    #[cfg(not(unix))]
    {
        if ssh_config_resolve_proxy_command(host_config).is_some() {
            return Err("ProxyCommand/ProxyJump 仅支持 Unix 平台".to_string());
        }
        let tcp = TcpStream::connect((host_config.hostname.as_str(), host_config.port)).map_err(
            |e| {
                format!(
                    "TCP 连接 {}:{} 失败: {}",
                    host_config.hostname, host_config.port, e
                )
            },
        )?;
        tcp.set_read_timeout(Some(Duration::from_secs(30))).ok();
        tcp.set_write_timeout(Some(Duration::from_secs(30))).ok();
        sess.set_tcp_stream(tcp);
    }

    sess.handshake().map_err(err_ssh)?;
    Ok(sess)
}

fn authenticate_session(
    sess: &Session,
    host_config: &SshHostConfig,
    auth_method: AuthMethod,
    password: Option<&str>,
) -> Result<(), String> {
    let user = &host_config.user;

    let try_pubkey = || -> bool {
        if let Some(ref key_path) = host_config.identity_file {
            if key_path.exists()
                && sess
                    .userauth_pubkey_file(user, None, key_path, None)
                    .is_ok()
                && sess.authenticated()
            {
                return true;
            }
        }
        let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        for name in ["id_ed25519", "id_rsa", "id_ecdsa"] {
            let key = home.join(".ssh").join(name);
            if key.exists()
                && sess.userauth_pubkey_file(user, None, &key, None).is_ok()
                && sess.authenticated()
            {
                return true;
            }
        }
        false
    };

    let try_agent = || -> bool {
        sess.userauth_agent(user).is_ok() && sess.authenticated()
    };

    let try_password = || -> bool {
        let Some(pwd) = password.filter(|p| !p.is_empty()) else {
            return false;
        };
        sess.userauth_password(user, pwd).is_ok() && sess.authenticated()
    };

    match auth_method {
        AuthMethod::PublicKey => {
            if try_pubkey() || try_agent() {
                return Ok(());
            }
        }
        AuthMethod::Password | AuthMethod::KeyboardInteractive => {
            if try_password() {
                return Ok(());
            }
            // 回退到密钥，兼容误配 auth_method 的环境
            if try_pubkey() || try_agent() {
                return Ok(());
            }
        }
        AuthMethod::Auto => {
            if try_agent() || try_pubkey() || try_password() {
                return Ok(());
            }
        }
    }

    Err(
        "SSH 认证失败。请确认 ~/.ssh/config IdentityFile / ssh-agent，或在环境中保存密码凭证。"
            .to_string(),
    )
}

fn resolve_remote_path(sftp: &ssh2::Sftp, remote_path: &str) -> Result<String, String> {
    let path = remote_path.trim();
    if path.is_empty() {
        return Err("远端路径不能为空".to_string());
    }
    if path == "~" || path.starts_with("~/") || path.starts_with('~') {
        match sftp.realpath(Path::new(path)) {
            Ok(resolved) => Ok(resolved.to_string_lossy().into_owned()),
            Err(_) if path.starts_with("~/") => {
                // 部分服务端不展开 ~，退化为相对登录目录
                let home = sftp
                    .realpath(Path::new("."))
                    .map_err(|e| format!("解析远端 HOME 失败: {}", e))?;
                Ok(format!(
                    "{}/{}",
                    home.to_string_lossy().trim_end_matches('/'),
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
fn open_sftp(
    ssh_host: &str,
    auth_method: AuthMethod,
    password: Option<&str>,
) -> Result<(Session, ssh2::Sftp), String> {
    let host_config = ssh_config_get_host_config(ssh_host)
        .ok_or_else(|| format!("~/.ssh/config 中未找到 Host: {}", ssh_host))?;
    let sess = connect_session(&host_config)?;
    authenticate_session(&sess, &host_config, auth_method, password)?;
    let sftp = sess.sftp().map_err(err_ssh)?;
    Ok((sess, sftp))
}

/// 通过 SFTP 上传本地文件到远端，并报告字节进度。
pub fn sftp_upload(
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
        return Err(format!("本地文件不存在或不是文件: {}", local_path.display()));
    }
    let total_bytes = std::fs::metadata(local_path)
        .map_err(|e| format!("读取本地文件失败: {}", e))?
        .len();

    let (_sess, sftp) = open_sftp(ssh_host, auth_method, password)?;
    check_cancelled(cancel)?;
    let remote = resolve_remote_path(&sftp, remote_path)?;

    if !overwrite {
        if sftp.stat(Path::new(&remote)).is_ok() {
            return Err(format!("远端文件已存在: {}", remote));
        }
    }

    let mut local = std::fs::File::open(local_path)
        .map_err(|e| format!("打开本地文件失败: {}", e))?;
    let mut remote_file = sftp
        .create(Path::new(&remote))
        .map_err(|e| format!("创建远端文件失败: {}", e))?;

    emit_progress(app, transfer_id, 0, Some(total_bytes));
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut transferred = 0u64;
    let mut ticker = ProgressEmitter::new();
    loop {
        check_cancelled(cancel)?;
        let n = local
            .read(&mut buf)
            .map_err(|e| format!("读取本地文件失败: {}", e))?;
        if n == 0 {
            break;
        }
        remote_file
            .write_all(&buf[..n])
            .map_err(|e| format!("写入远端失败: {}", e))?;
        transferred += n as u64;
        ticker.emit(app, transfer_id, transferred, Some(total_bytes));
    }
    remote_file
        .flush()
        .map_err(|e| format!("刷新远端文件失败: {}", e))?;
    emit_progress(app, transfer_id, transferred, Some(total_bytes));
    Ok(())
}

/// 通过 SFTP 从远端下载文件到本地，并报告字节进度。
pub fn sftp_download(
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
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let (_sess, sftp) = open_sftp(ssh_host, auth_method, password)?;
    check_cancelled(cancel)?;
    let remote = resolve_remote_path(&sftp, remote_path)?;

    let mut remote_file = sftp
        .open(Path::new(&remote))
        .map_err(|e| format!("打开远端文件失败: {}", e))?;
    let total_bytes = remote_file
        .stat()
        .ok()
        .and_then(|s| s.size);

    let mut local = std::fs::File::create(local_path)
        .map_err(|e| format!("创建本地文件失败: {}", e))?;

    emit_progress(app, transfer_id, 0, total_bytes);
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut transferred = 0u64;
    let mut ticker = ProgressEmitter::new();
    loop {
        check_cancelled(cancel)?;
        let n = remote_file
            .read(&mut buf)
            .map_err(|e| format!("读取远端文件失败: {}", e))?;
        if n == 0 {
            break;
        }
        local
            .write_all(&buf[..n])
            .map_err(|e| format!("写入本地文件失败: {}", e))?;
        transferred += n as u64;
        ticker.emit(app, transfer_id, transferred, total_bytes);
    }
    local
        .flush()
        .map_err(|e| format!("刷新本地文件失败: {}", e))?;
    let final_total = total_bytes.or(Some(transferred));
    emit_progress(app, transfer_id, transferred, final_total);
    Ok(())
}
