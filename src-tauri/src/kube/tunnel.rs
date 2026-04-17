//! SSH 隧道：连接跳板机、远程读取 kubeconfig，支持 ssh（子进程）或 builtin（libssh2）两种映射方式。
//! 支持 ~/.ssh/config 中的 ProxyCommand 与 ProxyJump。

use crate::config::{ssh_config_get_host_config, ssh_config_resolve_proxy_command, LogLevel};
use crate::debug_log;
use crate::env::TunnelMappingMode;
use serde::Serialize;
use serde::Deserialize;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(windows)]
fn apply_no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn apply_no_window(_: &mut Command) {}

#[derive(Debug, thiserror::Error)]
pub enum TunnelError {
    #[error("ssh config: {0}")]
    SshConfig(String),
    #[error("ssh connect: {0}")]
    Ssh(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("kubeconfig parse: {0}")]
    Kubeconfig(String),
    /// SSH 认证需要密码，前端收到此错误后应弹出密码输入框并重试。
    /// 格式固定为 "SSH_AUTH_REQUIRED:{tunnel_id}" 供前端解析。
    #[error("SSH_AUTH_REQUIRED:{0}")]
    AuthRequired(String),
}

use crate::ssh_askpass::SshAskpassGuard;

/// 去掉 SSH 输出开头的 shell 污染（如 bash locale 警告），保留从 apiVersion 行开始的 kubeconfig YAML。
fn trim_kubeconfig_pollution(content: &str) -> &str {
    if let Some(pos) = content.find("apiVersion:") {
        let line_start = content[..pos].rfind('\n').map(|n| n + 1).unwrap_or(0);
        &content[line_start..]
    } else {
        content
    }
}

/// 过滤掉 stderr 中所有 WARNING 行，返回剩余有效内容。
/// 兼容 `** WARNING:`、`warning:` 等前缀（大小写不敏感）。
fn strip_ssh_warnings(stderr: &str) -> String {
    stderr
        .lines()
        .filter(|l| {
            let lower = l.to_lowercase();
            !lower.trim_start().starts_with("** warning")
                && !lower.trim_start().starts_with("warning:")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 从 kubeconfig YAML 中解析出指定 context 对应 cluster 的 server 的 host 与 port（用于端口转发）。
/// context_override: 若提供则使用，否则用 current-context。
fn server_host_port_from_kubeconfig(yaml: &str, context_override: Option<&str>) -> Result<(String, u16, String, String), TunnelError> {
    let yaml = trim_kubeconfig_pollution(yaml);
    let root: KubeconfigRoot = serde_yaml::from_str(yaml).map_err(|e| {
        let preview: String = yaml.chars().take(150).collect();
        let preview = if preview.is_empty() {
            "<空>".into()
        } else {
            format!("{:?}", preview)
        };
        TunnelError::Kubeconfig(format!("{} 内容预览: {}", e, preview))
    })?;
    let current = context_override
        .or(root.current_context.as_deref())
        .ok_or(TunnelError::Kubeconfig("no current-context".into()))?;
    let cluster_name = root
        .contexts
        .as_deref()
        .and_then(|c| c.iter().find(|e| e.name.as_deref() == Some(current)))
        .and_then(|e| e.context.as_ref())
        .and_then(|c| c.cluster.as_ref())
        .ok_or_else(|| TunnelError::Kubeconfig("context or cluster not found".into()))?;
    let server_url = root
        .clusters
        .as_deref()
        .and_then(|c| c.iter().find(|e| e.name.as_deref() == Some(cluster_name)))
        .and_then(|e| e.cluster.as_ref())
        .and_then(|c| c.server.as_ref())
        .ok_or_else(|| TunnelError::Kubeconfig("cluster server not found".into()))?;
    // server 形如 https://host:port 或 https://host
    let rest = server_url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let (host, port) = if let Some(p) = rest.find(":") {
        let host = rest[..p].to_string();
        let port: u16 = rest[p + 1..].trim_end_matches('/').parse().unwrap_or(443);
        (host, port)
    } else {
        (rest.trim_end_matches('/').to_string(), 443)
    };
    Ok((host, port, current.to_string(), cluster_name.to_string()))
}

#[derive(Deserialize)]
struct KubeconfigRoot {
    #[serde(rename = "current-context")]
    current_context: Option<String>,
    contexts: Option<Vec<ContextEntry>>,
    clusters: Option<Vec<ClusterEntry>>,
}
#[derive(Deserialize)]
struct ContextEntry {
    name: Option<String>,
    context: Option<ContextInner>,
}
#[derive(Deserialize)]
struct ContextInner {
    cluster: Option<String>,
}
#[derive(Deserialize)]
struct ClusterEntry {
    name: Option<String>,
    cluster: Option<ClusterInner>,
}
#[derive(Deserialize)]
struct ClusterInner {
    server: Option<String>,
}

/// 将 kubeconfig YAML 中指定 context 对应 cluster 的 server 替换为 https://127.0.0.1:local_port，并设置 insecure-skip-tls-verify。
/// context_override: 若提供则使用，否则用 current-context。
fn replace_server_in_kubeconfig(yaml: &str, local_port: u16, context_override: Option<&str>) -> Result<String, TunnelError> {
    let yaml = trim_kubeconfig_pollution(yaml);
    let mut root: serde_yaml::Value = serde_yaml::from_str(yaml).map_err(|e| TunnelError::Kubeconfig(e.to_string()))?;
    let current = context_override
        .or_else(|| root.get("current-context").and_then(|v| v.as_str()))
        .ok_or_else(|| TunnelError::Kubeconfig("no current-context".into()))?;
    let cluster_name: String = root
        .get("contexts")
        .and_then(|c| c.as_sequence())
        .and_then(|s| s.iter().find(|e| e.get("name").and_then(|n| n.as_str()) == Some(current)))
        .and_then(|e| e.get("context").and_then(|c| c.get("cluster")).and_then(|c| c.as_str()))
        .map(String::from)
        .ok_or_else(|| TunnelError::Kubeconfig("context or cluster not found".into()))?;
    let clusters = root
        .get_mut("clusters")
        .and_then(|c| c.as_sequence_mut())
        .ok_or_else(|| TunnelError::Kubeconfig("clusters not found".into()))?;
    for cluster in clusters.iter_mut() {
        if cluster.get("name").and_then(|n| n.as_str()) == Some(cluster_name.as_str()) {
            if let Some(inner) = cluster.get_mut("cluster").and_then(|c| c.as_mapping_mut()) {
                inner.insert(
                    serde_yaml::Value::String("server".into()),
                    serde_yaml::Value::String(format!("https://127.0.0.1:{}", local_port)),
                );
                inner.insert(
                    serde_yaml::Value::String("insecure-skip-tls-verify".into()),
                    serde_yaml::Value::Bool(true),
                );
            }
            break;
        }
    }
    serde_yaml::to_string(&root).map_err(|e| TunnelError::Kubeconfig(e.to_string()))
}

/// 连接进度事件，供前端展示步骤条。
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionProgressPayload {
    pub env_id: String,
    pub source: String,
    pub stage_id: String,
    pub stage_label: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub overall_status: String,
}

fn emit_progress(
    tx: &Option<mpsc::Sender<ConnectionProgressPayload>>,
    env_id: &str,
    stage_id: &str,
    stage_label: &str,
    status: &str,
    detail: Option<&str>,
    overall_status: &str,
) {
    if let Some(tx) = tx {
        let _ = tx.send(ConnectionProgressPayload {
            env_id: env_id.to_string(),
            source: "ssh_tunnel".to_string(),
            stage_id: stage_id.to_string(),
            stage_label: stage_label.to_string(),
            status: status.to_string(),
            detail: detail.map(String::from),
            error: None,
            overall_status: overall_status.to_string(),
        });
    }
}

fn emit_error_progress(
    tx: &Option<mpsc::Sender<ConnectionProgressPayload>>,
    env_id: &str,
    stage_id: &str,
    stage_label: &str,
    error: &str,
) {
    if let Some(tx) = tx {
        let _ = tx.send(ConnectionProgressPayload {
            env_id: env_id.to_string(),
            source: "ssh_tunnel".to_string(),
            stage_id: stage_id.to_string(),
            stage_label: stage_label.to_string(),
            status: "error".to_string(),
            detail: None,
            error: Some(error.to_string()),
            overall_status: "error".to_string(),
        });
    }
}

fn shell_escape(s: &str) -> String {
    if s.contains(' ') || s.contains('"') || s.contains('$') || s.contains('`') {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

fn append_idle_protection_ssh_args(cmd: &mut Command, enabled: bool) {
    if !enabled {
        return;
    }
    cmd.args([
        "-o",
        "ServerAliveInterval=30",
        "-o",
        "ServerAliveCountMax=3",
        "-o",
        "TCPKeepAlive=yes",
    ]);
}

/// 通过 ProxyCommand 建立连接；仅 Unix。返回 UnixStream 供 libssh2 使用。
#[cfg(unix)]
fn connect_via_proxy(proxy_args: Vec<String>) -> Result<std::os::unix::net::UnixStream, TunnelError> {
    use std::os::unix::net::UnixStream;

    let (mut proxy_end, libssh2_end) = UnixStream::pair().map_err(TunnelError::Io)?;

    let (exe, args) = proxy_args
        .split_first()
        .ok_or_else(|| TunnelError::Ssh("ProxyCommand 为空".into()))?;

    let mut child = Command::new(exe)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| TunnelError::Ssh(format!("启动 ProxyCommand 失败: {}", e)))?;

    let mut child_stdin = child.stdin.take().ok_or_else(|| TunnelError::Ssh("无法获取 ProxyCommand stdin".into()))?;
    let mut child_stdout = child.stdout.take().ok_or_else(|| TunnelError::Ssh("无法获取 ProxyCommand stdout".into()))?;

    let mut proxy_end_clone = proxy_end.try_clone().map_err(TunnelError::Io)?;

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

    libssh2_end.set_read_timeout(Some(Duration::from_secs(30))).ok();
    libssh2_end.set_write_timeout(Some(Duration::from_secs(30))).ok();
    Ok(libssh2_end)
}

/// 阻塞运行隧道：根据 tunnel_mode 选择 ssh 或 builtin 实现。
#[allow(clippy::too_many_arguments)]
fn run_tunnel_blocking(
    tx: mpsc::Sender<Result<(u16, String), String>>,
    shutdown: Arc<AtomicBool>,
    env_id: String,
    tunnel_id: String,
    ssh_host: String,
    remote_kubeconfig_path: String,
    local_port: Option<u16>,
    preferred_context: Option<String>,
    tunnel_mode: TunnelMappingMode,
    password: Option<String>,
    idle_protection_enabled: bool,
    progress_tx: Option<mpsc::Sender<ConnectionProgressPayload>>,
) -> Result<(), TunnelError> {
    match tunnel_mode {
        TunnelMappingMode::Ssh => run_tunnel_ssh(
            tx,
            shutdown,
            env_id,
            tunnel_id,
            ssh_host,
            remote_kubeconfig_path,
            local_port,
            preferred_context,
            password,
            idle_protection_enabled,
            progress_tx,
        ),
        TunnelMappingMode::Builtin => run_tunnel_builtin(
            tx,
            shutdown,
            env_id,
            ssh_host,
            remote_kubeconfig_path,
            local_port,
            preferred_context,
            progress_tx,
        ),
    }
}

/// ssh 模式：纯调用系统 ssh 命令，获取 kubeconfig 与端口转发均走 ssh，自动支持 ProxyCommand/ProxyJump，跨平台。
///
/// 密码处理流程：
///   - password.is_some()：创建 SSH_ASKPASS 临时脚本，注入两步 SSH 命令（cat + -L）
///   - password.is_none()：加 BatchMode=yes 快速探测；若认证失败返回 AuthRequired
#[allow(clippy::too_many_arguments)]
fn run_tunnel_ssh(
    tx: mpsc::Sender<Result<(u16, String), String>>,
    shutdown: Arc<AtomicBool>,
    env_id: String,
    tunnel_id: String,
    ssh_host: String,
    remote_kubeconfig_path: String,
    local_port: Option<u16>,
    preferred_context: Option<String>,
    password: Option<String>,
    idle_protection_enabled: bool,
    progress_tx: Option<mpsc::Sender<ConnectionProgressPayload>>,
) -> Result<(), TunnelError> {
    let _ = ssh_config_get_host_config(&ssh_host).ok_or_else(|| {
        let e = TunnelError::SshConfig(format!("~/.ssh/config 中未找到 Host: {}", ssh_host));
        emit_error_progress(&progress_tx, &env_id, "ssh_config", "解析 SSH 配置", &e.to_string());
        e
    })?;

    emit_progress(&progress_tx, &env_id, "ssh_config", "解析 SSH 配置", "success", None, "connecting");
    emit_progress(&progress_tx, &env_id, "fetch_kubeconfig", "通过 SSH 获取 kubeconfig", "running", Some(&format!("Host {}", ssh_host)), "connecting");

    let cat_cmd = format!("cat {}", shell_escape(&remote_kubeconfig_path));
    debug_log::log_tunnel(
        Some(&env_id),
        "connect",
        Some(&format!("Host {} (ssh cat, has_password={})", ssh_host, password.is_some())),
        LogLevel::Info,
    );

    // 构建用于第一步（获取 kubeconfig）的 ssh 命令
    // 有密码：注入 SSH_ASKPASS；无密码：加 BatchMode=yes 快速失败
    #[cfg(unix)]
    let _askpass_guard: Option<SshAskpassGuard>;
    #[cfg(unix)]
    let askpass_path: Option<String>;

    #[cfg(unix)]
    {
        if let Some(ref pwd) = password {
            match SshAskpassGuard::new(pwd) {
                Ok(guard) => {
                    askpass_path = Some(guard.path_str().to_string());
                    _askpass_guard = Some(guard);
                }
                Err(e) => {
                    let msg = format!("创建 SSH_ASKPASS 脚本失败: {}", e);
                    emit_error_progress(&progress_tx, &env_id, "fetch_kubeconfig", "通过 SSH 获取 kubeconfig", &msg);
                    return Err(TunnelError::Ssh(msg));
                }
            }
        } else {
            _askpass_guard = None;
            askpass_path = None;
        }
    }

    #[cfg(not(unix))]
    let askpass_path: Option<String> = None;

    let mut cat_cmd_builder = Command::new("ssh");
    cat_cmd_builder.stdin(Stdio::null());
    apply_no_window(&mut cat_cmd_builder);
    append_idle_protection_ssh_args(&mut cat_cmd_builder, idle_protection_enabled);

    if let Some(ref ap) = askpass_path {
        // 有密码：通过 SSH_ASKPASS 注入，SSH_ASKPASS_REQUIRE=force 不依赖 DISPLAY
        // 同时设置 DISPLAY=:0 兼容旧版 OpenSSH（<8.4）
        cat_cmd_builder
            .env("SSH_ASKPASS", ap)
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env("DISPLAY", ":0");
    } else {
        // 无密码：批处理模式快速失败，避免卡在 TTY 等待
        cat_cmd_builder
            .args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10"]);
    }
    cat_cmd_builder.args([&ssh_host, &cat_cmd]);

    let output = cat_cmd_builder
        .output()
        .map_err(|e| {
            let msg = format!("ssh cat 失败: {}", e);
            emit_error_progress(&progress_tx, &env_id, "fetch_kubeconfig", "通过 SSH 获取 kubeconfig", &msg);
            TunnelError::Ssh(msg)
        })?;

    // 以 stdout 中是否存在 apiVersion: 作为成功判据：
    // 部分 SSH 客户端因安全 WARNING 退出非零，但实际已成功输出 kubeconfig，
    // 与 trim_kubeconfig_pollution 处理 stdout 污染保持一致的策略。
    let stdout_raw = String::from_utf8_lossy(&output.stdout);
    if !stdout_raw.contains("apiVersion:") {
        // stdout 无有效 kubeconfig，进入错误判断
        let stderr = String::from_utf8_lossy(&output.stderr);
        // 完整 stderr 写入调试日志，便于全景排查
        debug_log::log_tunnel(
            Some(&env_id),
            "ssh_cat_stderr",
            Some(&format!("exit={:?} stderr={}", output.status.code(), stderr.trim())),
            LogLevel::Error,
        );
        let real_stderr = strip_ssh_warnings(&stderr);
        let real_lower = real_stderr.to_lowercase();
        // 判断是否为认证失败：需要密码但未提供
        let is_auth_failure = password.is_none()
            && (real_lower.contains("permission denied")
                || real_lower.contains("publickey")
                || real_lower.contains("authentication failed")
                || real_lower.contains("no supported authentication"));
        if is_auth_failure {
            let msg = "SSH 认证需要密码，请输入密码后重试".to_string();
            emit_error_progress(&progress_tx, &env_id, "fetch_kubeconfig", "通过 SSH 获取 kubeconfig", &msg);
            return Err(TunnelError::AuthRequired(tunnel_id));
        }
        let first_line = real_stderr.trim().lines().next().unwrap_or("无输出").to_string();
        let msg = format!("ssh cat 失败: {}", first_line);
        emit_error_progress(&progress_tx, &env_id, "fetch_kubeconfig", "通过 SSH 获取 kubeconfig", &msg);
        return Err(TunnelError::Ssh(msg));
    }

    let content = String::from_utf8(output.stdout).map_err(|_| {
        let msg = "ssh cat 输出非 UTF-8".to_string();
        emit_error_progress(&progress_tx, &env_id, "fetch_kubeconfig", "通过 SSH 获取 kubeconfig", &msg);
        TunnelError::Ssh(msg)
    })?;

    debug_log::log_tunnel(
        Some(&env_id),
        "kubeconfig_fetched",
        Some(&format!(
            "path={} len={}",
            remote_kubeconfig_path,
            content.len()
        )),
        LogLevel::Info,
    );
    emit_progress(&progress_tx, &env_id, "fetch_kubeconfig", "通过 SSH 获取 kubeconfig", "success", None, "connecting");
    emit_progress(&progress_tx, &env_id, "parse_kubeconfig", "解析 kubeconfig", "running", None, "connecting");

    let (remote_host, remote_port, ctx_used, cluster_name) =
        server_host_port_from_kubeconfig(&content, preferred_context.as_deref()).map_err(|e| {
            emit_error_progress(&progress_tx, &env_id, "parse_kubeconfig", "解析 kubeconfig", &e.to_string());
            e
        })?;

    debug_log::log_tunnel(
        Some(&env_id),
        "remote_server_parsed",
        Some(&format!(
            "context={} cluster={} remote={}:{}",
            ctx_used, cluster_name, remote_host, remote_port
        )),
        LogLevel::Info,
    );
    emit_progress(&progress_tx, &env_id, "parse_kubeconfig", "解析 kubeconfig", "success", Some(&format!("{}:{}", remote_host, remote_port)), "connecting");
    emit_progress(&progress_tx, &env_id, "create_tunnel", "创建 SSH 隧道", "running", None, "connecting");

    let local_port = match local_port {
        Some(p) => p,
        None => {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(TunnelError::Io)?;
            let port = listener.local_addr().map_err(TunnelError::Io)?.port();
            drop(listener);
            port
        }
    };

    let virtual_yaml = replace_server_in_kubeconfig(&content, local_port, preferred_context.as_deref()).map_err(|e| {
        emit_error_progress(&progress_tx, &env_id, "parse_kubeconfig", "解析 kubeconfig", &e.to_string());
        e
    })?;

    let ssh_tunnel_cmd = format!(
        "ssh -L {}:{}:{} {} -N",
        local_port, remote_host, remote_port, ssh_host
    );
    debug_log::log_tunnel(Some(&env_id), "ssh_cmd", Some(&ssh_tunnel_cmd), LogLevel::Info);
    debug_log::log_tunnel(
        Some(&env_id),
        "ok",
        Some(&format!(
            "127.0.0.1:{} -> {}:{} (ssh -L)",
            local_port, remote_host, remote_port
        )),
        LogLevel::Info,
    );
    emit_progress(&progress_tx, &env_id, "create_tunnel", "创建 SSH 隧道", "success", Some(&format!("127.0.0.1:{}", local_port)), "connecting");
    emit_progress(&progress_tx, &env_id, "create_client", "创建 K8s 客户端", "running", None, "connecting");

    // 启动 ssh -L -N 端口转发子进程，同样注入 SSH_ASKPASS（如有）
    let mut tunnel_cmd = std::process::Command::new("ssh");
    apply_no_window(&mut tunnel_cmd);
    append_idle_protection_ssh_args(&mut tunnel_cmd, idle_protection_enabled);
    tunnel_cmd
        .args([
            "-L",
            &format!("{}:{}:{}", local_port, remote_host, remote_port),
            &ssh_host,
            "-N",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    if let Some(ref ap) = askpass_path {
        tunnel_cmd
            .env("SSH_ASKPASS", ap)
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env("DISPLAY", ":0");
    }

    let mut child = tunnel_cmd.spawn().map_err(|e| {
        let msg = format!("启动 ssh -L 子进程失败: {}", e);
        emit_error_progress(&progress_tx, &env_id, "create_tunnel", "创建 SSH 隧道", &msg);
        TunnelError::Ssh(msg)
    })?;

    // 等待 ssh -L 在本地端口就绪，检查子进程是否提前退出（认证失败等）。
    // 最长等待 30 秒（密码认证 + 网络延迟），逐 200ms 轮询一次。
    let mut port_ready = false;
    for _ in 0..150 {
        // 子进程提前退出说明 ssh -L 连接失败
        match child.try_wait() {
            Ok(Some(exit_status)) => {
                let msg = format!(
                    "ssh -L 子进程提前退出 (exit={}), 隧道建立失败",
                    exit_status
                );
                debug_log::log_tunnel_err(Some(&env_id), &msg, LogLevel::Error);
                emit_error_progress(&progress_tx, &env_id, "create_tunnel", "创建 SSH 隧道", &msg);
                let _ = tx.send(Err(msg.clone()));
                return Ok(());
            }
            _ => {}
        }
        if std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], local_port)),
            Duration::from_millis(100),
        )
        .is_ok()
        {
            port_ready = true;
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }

    if !port_ready {
        let _ = child.kill();
        let _ = child.wait();
        let msg = format!(
            "等待 SSH 端口转发超时（30 秒），127.0.0.1:{} 未就绪，请检查 SSH 连接与网络",
            local_port
        );
        debug_log::log_tunnel_err(Some(&env_id), &msg, LogLevel::Error);
        emit_error_progress(&progress_tx, &env_id, "create_tunnel", "创建 SSH 隧道", &msg);
        let _ = tx.send(Err(msg));
        return Ok(());
    }

    emit_progress(&progress_tx, &env_id, "create_client", "创建 K8s 客户端", "success", None, "connected");

    tx.send(Ok((local_port, virtual_yaml)))
        .map_err(|_| TunnelError::Io(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "channel closed")))?;

    while !shutdown.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(200));
    }

    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

/// builtin 模式：libssh2 channel_direct_tcpip 内置转发，无子进程。
fn run_tunnel_builtin(
    tx: mpsc::Sender<Result<(u16, String), String>>,
    shutdown: Arc<AtomicBool>,
    env_id: String,
    ssh_host: String,
    remote_kubeconfig_path: String,
    local_port: Option<u16>,
    preferred_context: Option<String>,
    progress_tx: Option<mpsc::Sender<ConnectionProgressPayload>>,
) -> Result<(), TunnelError> {
    let host_config = ssh_config_get_host_config(&ssh_host).ok_or_else(|| {
        let e = TunnelError::SshConfig(format!("~/.ssh/config 中未找到 Host: {}", ssh_host));
        emit_error_progress(&progress_tx, &env_id, "ssh_config", "解析 SSH 配置", &e.to_string());
        e
    })?;

    emit_progress(&progress_tx, &env_id, "ssh_config", "解析 SSH 配置", "success", None, "connecting");
    emit_progress(&progress_tx, &env_id, "fetch_kubeconfig", "通过 SSH 获取 kubeconfig", "running", None, "connecting");

    let connect_target = format!("{}:{}", host_config.hostname, host_config.port);
    debug_log::log_tunnel(
        Some(&env_id),
        "connect",
        Some(&format!("Host {} -> {} (builtin)", ssh_host, connect_target)),
        LogLevel::Info,
    );

    let mut sess = ssh2::Session::new().map_err(|e| TunnelError::Ssh(e.to_string()))?;

    #[cfg(unix)]
    {
        if let Some(proxy_args) = ssh_config_resolve_proxy_command(&host_config) {
            debug_log::log_tunnel(
                Some(&env_id),
                "proxy",
                Some(&proxy_args.join(" ")),
                LogLevel::Info,
            );
            let stream = connect_via_proxy(proxy_args)?;
            sess.set_tcp_stream(stream);
        } else {
            let tcp = std::net::TcpStream::connect((host_config.hostname.as_str(), host_config.port))
                .map_err(|e| {
                    let msg = format!("TCP connect to {}: {}", connect_target, e);
                    emit_error_progress(&progress_tx, &env_id, "fetch_kubeconfig", "通过 SSH 获取 kubeconfig", &msg);
                    TunnelError::Ssh(msg)
                })?;
            tcp.set_read_timeout(Some(Duration::from_secs(30))).ok();
            tcp.set_write_timeout(Some(Duration::from_secs(30))).ok();
            sess.set_tcp_stream(tcp);
        }
    }

    #[cfg(not(unix))]
    {
        if ssh_config_resolve_proxy_command(&host_config).is_some() {
            let msg = "ProxyCommand/ProxyJump 仅支持 Unix 平台".to_string();
            emit_error_progress(&progress_tx, &env_id, "fetch_kubeconfig", "通过 SSH 获取 kubeconfig", &msg);
            return Err(TunnelError::Ssh(msg));
        }
        let tcp = std::net::TcpStream::connect((host_config.hostname.as_str(), host_config.port))
            .map_err(|e| {
                let msg = format!("TCP connect to {}: {}", connect_target, e);
                emit_error_progress(&progress_tx, &env_id, "fetch_kubeconfig", "通过 SSH 获取 kubeconfig", &msg);
                TunnelError::Ssh(msg)
            })?;
        tcp.set_read_timeout(Some(Duration::from_secs(30))).ok();
        tcp.set_write_timeout(Some(Duration::from_secs(30))).ok();
        sess.set_tcp_stream(tcp);
    }

    sess.handshake().map_err(|e| {
        let msg = e.to_string();
        emit_error_progress(&progress_tx, &env_id, "fetch_kubeconfig", "通过 SSH 获取 kubeconfig", &msg);
        TunnelError::Ssh(msg)
    })?;

    if let Some(ref key_path) = host_config.identity_file {
        if key_path.exists() {
            let _ = sess.userauth_pubkey_file(&host_config.user, None, key_path, None);
        }
    }
    if !sess.authenticated() {
        let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        for name in ["id_ed25519", "id_rsa", "id_ecdsa"] {
            let key = home.join(".ssh").join(name);
            if key.exists() {
                let _ = sess.userauth_pubkey_file(&host_config.user, None, &key, None);
                if sess.authenticated() {
                    break;
                }
            }
        }
    }
    if !sess.authenticated() {
        let _ = sess.userauth_agent(&host_config.user);
    }
    if !sess.authenticated() {
        let msg = "SSH 认证失败。请在 ~/.ssh/config 中为该 Host 设置 IdentityFile，或确保本机存在 ~/.ssh/id_ed25519 / id_rsa，或运行 ssh-add 将密钥加入 agent。".to_string();
        emit_error_progress(&progress_tx, &env_id, "fetch_kubeconfig", "通过 SSH 获取 kubeconfig", &msg);
        return Err(TunnelError::Ssh(msg));
    }

    let mut channel = sess.channel_session().map_err(|e| TunnelError::Ssh(e.to_string()))?;
    let cmd = format!("cat {}", shell_escape(&remote_kubeconfig_path));
    channel.exec(&cmd).map_err(|e| TunnelError::Ssh(e.to_string()))?;
    let mut content = String::new();
    channel.read_to_string(&mut content).map_err(TunnelError::Io)?;
    channel.wait_close().map_err(|e| TunnelError::Ssh(e.to_string()))?;

    debug_log::log_tunnel(
        Some(&env_id),
        "kubeconfig_fetched",
        Some(&format!("path={} len={}", remote_kubeconfig_path, content.len())),
        LogLevel::Info,
    );
    emit_progress(&progress_tx, &env_id, "fetch_kubeconfig", "通过 SSH 获取 kubeconfig", "success", None, "connecting");
    emit_progress(&progress_tx, &env_id, "parse_kubeconfig", "解析 kubeconfig", "running", None, "connecting");

    let (remote_host, remote_port, ctx_used, cluster_name) =
        server_host_port_from_kubeconfig(&content, preferred_context.as_deref()).map_err(|e| {
            emit_error_progress(&progress_tx, &env_id, "parse_kubeconfig", "解析 kubeconfig", &e.to_string());
            e
        })?;

    debug_log::log_tunnel(
        Some(&env_id),
        "remote_server_parsed",
        Some(&format!(
            "context={} cluster={} remote={}:{}",
            ctx_used, cluster_name, remote_host, remote_port
        )),
        LogLevel::Info,
    );
    emit_progress(&progress_tx, &env_id, "parse_kubeconfig", "解析 kubeconfig", "success", Some(&format!("{}:{}", remote_host, remote_port)), "connecting");
    emit_progress(&progress_tx, &env_id, "create_tunnel", "创建 SSH 隧道", "running", None, "connecting");

    let bind_port = local_port.unwrap_or(0);
    let listener = std::net::TcpListener::bind(format!("127.0.0.1:{}", bind_port)).map_err(TunnelError::Io)?;
    let local_port = listener.local_addr().map_err(TunnelError::Io)?.port();
    listener.set_nonblocking(true).map_err(TunnelError::Io)?;

    let virtual_yaml = replace_server_in_kubeconfig(&content, local_port, preferred_context.as_deref())?;
    debug_log::log_tunnel(
        Some(&env_id),
        "ok",
        Some(&format!(
            "127.0.0.1:{} -> {}:{} (builtin)",
            local_port, remote_host, remote_port
        )),
        LogLevel::Info,
    );
    emit_progress(&progress_tx, &env_id, "create_tunnel", "创建 SSH 隧道", "success", Some(&format!("127.0.0.1:{}", local_port)), "connecting");
    emit_progress(&progress_tx, &env_id, "create_client", "创建 K8s 客户端", "running", None, "connecting");
    emit_progress(&progress_tx, &env_id, "create_client", "创建 K8s 客户端", "success", None, "connected");

    tx.send(Ok((local_port, virtual_yaml)))
        .map_err(|_| TunnelError::Io(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "channel closed")))?;

    while !shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, addr)) => {
                let _ = stream.set_nodelay(true);
                let src_ip = addr.ip().to_string();
                match sess.channel_direct_tcpip(&remote_host, remote_port, Some((src_ip.as_str(), addr.port()))) {
                    Ok(channel) => {
                        thread::spawn(move || tunnel_bidirectional_spawn(stream, channel));
                    }
                    Err(e) => {
                        let err_msg = format!(
                            "direct-tcpip 到 {}:{} 失败: {}",
                            remote_host, remote_port, e
                        );
                        debug_log::log_tunnel_err(Some(&env_id), &err_msg, LogLevel::Error);
                        drop(stream);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(_) => break,
        }
    }
    Ok(())
}

/// 单连接双向转发：stream ↔ channel 对拷（builtin 模式用）。
fn tunnel_bidirectional_spawn(stream: std::net::TcpStream, channel: ssh2::Channel) {
    let mut s_read = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut s_write = stream;
    let channel = Arc::new(Mutex::new(channel));
    let c_w = channel.clone();
    let c_r = channel.clone();
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            let n = match s_read.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            let mut ch = match c_w.lock() {
                Ok(g) => g,
                Err(_) => break,
            };
            if ch.write_all(&buf[..n]).is_err() {
                break;
            }
            let _ = ch.flush();
        }
        if let Ok(mut ch) = c_w.lock() {
            let _ = ch.close();
        }
    });
    let mut buf = [0u8; 8192];
    loop {
        let n = match c_r.lock() {
            Ok(mut ch) => match ch.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            },
            Err(_) => break,
        };
        if s_write.write_all(&buf[..n]).is_err() {
            break;
        }
    }
}

/// 每个隧道的状态：后台线程 handle、shutdown 标志、本地端口、虚拟 kubeconfig YAML。
struct TunnelState {
    _handle: thread::JoinHandle<()>,
    shutdown: Arc<AtomicBool>,
    local_port: u16,
    virtual_kubeconfig_yaml: String,
}

/// SSH 隧道管理器：按 env_id 维护隧道，支持 ensure 与 close。
pub struct SshTunnelRunner {
    tunnels: Mutex<std::collections::HashMap<String, TunnelState>>,
}

impl SshTunnelRunner {
    pub fn new() -> Self {
        Self {
            tunnels: Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// 确保该 env 的隧道已启动，返回 (local_port, virtual_kubeconfig_yaml)。若已存在则直接返回缓存。
    pub fn ensure_tunnel(
        &self,
        env_id: String,
        tunnel_id: String,
        ssh_host: String,
        remote_kubeconfig_path: String,
        local_port: Option<u16>,
        preferred_context: Option<String>,
        tunnel_mode: TunnelMappingMode,
        password: Option<String>,
        idle_protection_enabled: bool,
        progress_tx: Option<mpsc::Sender<ConnectionProgressPayload>>,
    ) -> Result<(u16, String), TunnelError> {
        {
            let g = self.tunnels.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(s) = g.get(&env_id) {
                debug_log::log_tunnel(
                    Some(&env_id),
                    "cached",
                    Some(&format!("127.0.0.1:{}", s.local_port)),
                    LogLevel::Info,
                );
                return Ok((s.local_port, s.virtual_kubeconfig_yaml.clone()));
            }
        }
        let (tx, rx) = mpsc::channel::<Result<(u16, String), String>>();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let ssh_host_c = ssh_host.clone();
        let path_c = remote_kubeconfig_path.clone();
        let local_port_c = local_port;
        let preferred_ctx_c = preferred_context.clone();
        let env_id_c = env_id.clone();
        let tunnel_id_c = tunnel_id.clone();
        let handle = thread::spawn(move || {
            if let Err(e) = run_tunnel_blocking(
                tx.clone(),
                shutdown_clone,
                env_id_c.clone(),
                tunnel_id_c,
                ssh_host_c,
                path_c,
                local_port_c,
                preferred_ctx_c,
                tunnel_mode,
                password,
                idle_protection_enabled,
                progress_tx,
            ) {
                debug_log::log_tunnel_err(Some(&env_id_c), &e.to_string(), LogLevel::Error);
                // AuthRequired 需要保留结构化错误码，不能 to_string() 丢失信息
                let err_str = e.to_string();
                let _ = tx.send(Err(err_str));
            }
        });
        let (local_port, virtual_yaml) = match rx.recv_timeout(Duration::from_secs(90)) {
            Ok(Ok(t)) => t,
            Ok(Err(s)) => {
                // 还原结构化错误：AuthRequired 经 to_string() 传递后需在此重建
                if let Some(tid) = s.strip_prefix("SSH_AUTH_REQUIRED:") {
                    return Err(TunnelError::AuthRequired(tid.to_string()));
                }
                return Err(TunnelError::Ssh(s));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(TunnelError::Ssh("SSH 隧道建立超时（90 秒），请检查网络与 SSH 配置".into()));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(TunnelError::Ssh("tunnel thread exited without sending".into()));
            }
        };
        let state = TunnelState {
            _handle: handle,
            shutdown,
            local_port,
            virtual_kubeconfig_yaml: virtual_yaml.clone(),
        };
        {
            let mut g = self.tunnels.lock().unwrap_or_else(|p| p.into_inner());
            g.insert(env_id, state);
        }
        Ok((local_port, virtual_yaml))
    }

    pub fn get_local_port(&self, env_id: &str) -> Option<u16> {
        let g = self.tunnels.lock().unwrap_or_else(|p| p.into_inner());
        g.get(env_id).map(|s| s.local_port)
    }

    pub fn close_tunnel(&self, env_id: &str) {
        let mut g = self.tunnels.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(s) = g.remove(env_id) {
            s.shutdown.store(true, Ordering::SeqCst);
            let _ = s._handle.join();
        }
    }

    /// 关闭所有隧道并 kill 对应 SSH 子进程；应用退出前调用，避免孤儿进程。
    pub fn close_all_tunnels(&self) {
        let to_close: Vec<_> = {
            let mut g = self.tunnels.lock().unwrap_or_else(|p| p.into_inner());
            g.drain().collect()
        };
        for (_, s) in to_close {
            s.shutdown.store(true, Ordering::SeqCst);
            let _ = s._handle.join();
        }
    }
}

impl Default for SshTunnelRunner {
    fn default() -> Self {
        Self::new()
    }
}
