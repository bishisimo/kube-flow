use crate::commands::kube_command_context::{err_str, load_app_settings, CommandResult};
use crate::kube::session_store::{SessionHandle, SessionStore};
use crate::config::ssh_config_get_host_config;
use crate::credentials::{AuthMethod, CredentialKey, CredentialManager};
use crate::env::{EnvService, EnvironmentSource};
use crate::kube::{resource_get, KubeClientStore};
use serde::Deserialize;
#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::os::fd::FromRawFd;
use std::process::Stdio;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::mpsc;
use uuid::Uuid;

const HOST_SHELL_CHUNK_EVENT: &str = "host-shell-chunk";
const HOST_SHELL_END_EVENT: &str = "host-shell-end";
const NODE_TERMINAL_SUDO_PROMPT: &str = "__KUBE_FLOW_SUDO_PROMPT__";

#[cfg(windows)]
fn apply_no_window(cmd: &mut Command) {
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn apply_no_window(_: &mut Command) {}

use crate::ssh_askpass::SshAskpassGuard;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeTerminalStepType {
    Ssh,
    SwitchUser,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeTerminalStepRequest {
    pub r#type: NodeTerminalStepType,
    pub user: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostShellBootstrapRequest {
    pub kind: String,
    pub host: String,
    pub steps: Vec<NodeTerminalStepRequest>,
    pub credential_id: Option<String>,
    pub pod_debug: Option<PodDebugTargetRequest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PodDebugNamespace {
    Net,
    Pid,
    Mnt,
    Uts,
    Ipc,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodDebugTargetRequest {
    pub namespace: String,
    pub pod_name: String,
    pub container: String,
    pub namespaces: Vec<PodDebugNamespace>,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone)]
struct HostShellAutomation {
    command: String,
    sudo_password: Option<String>,
    sudo_prompt: Option<String>,
}

#[derive(Debug, Clone)]
struct HostShellAutomationState {
    sudo_password: Option<String>,
    sudo_prompt: Option<String>,
    password_sent: bool,
}

impl HostShellAutomationState {
    fn new(plan: HostShellAutomation) -> Self {
        Self {
            sudo_password: plan.sudo_password,
            sudo_prompt: plan.sudo_prompt,
            password_sent: false,
        }
    }

    fn process_output(&mut self, chunk: &[u8]) -> (Vec<u8>, Option<Vec<u8>>) {
        let mut output = String::from_utf8_lossy(chunk).into_owned();
        let mut password_input = None;
        if let (Some(prompt), Some(password)) = (&self.sudo_prompt, &self.sudo_password) {
            if output.contains(prompt) {
                output = output.replace(prompt, "");
                if !self.password_sent {
                    self.password_sent = true;
                    password_input = Some(format!("{}\n", password).into_bytes());
                }
            }
        }
        (output.into_bytes(), password_input)
    }
}

fn shell_single_quote(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\"'\"'"))
}

fn build_pod_nsenter_command(
    container_id: Option<&str>,
    pid: Option<u32>,
    namespaces: &[PodDebugNamespace],
) -> String {
    let interactive_shell = "export TERM=\"xterm-256color\"; \
export LANG=\"${LANG:-C.UTF-8}\"; \
export LC_CTYPE=\"${LC_CTYPE:-$LANG}\"; \
if command -v bash >/dev/null 2>&1; then exec bash -il; else exec sh -i; fi";
    let mut flags = Vec::new();
    for ns in namespaces {
        let flag = match ns {
            PodDebugNamespace::Net => "-n",
            PodDebugNamespace::Pid => "-p",
            PodDebugNamespace::Mnt => "-m",
            PodDebugNamespace::Uts => "-u",
            PodDebugNamespace::Ipc => "-i",
        };
        if !flags.contains(&flag) {
            flags.push(flag);
        }
    }
    let ns_flags = if flags.is_empty() {
        "-n".to_string()
    } else {
        flags.join(" ")
    };
    if let Some(target_pid) = pid {
        return format!(
            "exec nsenter -t {} {} /bin/sh -lc {}",
            target_pid,
            ns_flags,
            shell_single_quote(interactive_shell)
        );
    }
    let container_id = container_id.unwrap_or_default();
    format!(
        "CONTAINER_ID={cid}; \
PID=\"\"; \
if command -v crictl >/dev/null 2>&1; then \
  PID=$(crictl inspect \"$CONTAINER_ID\" 2>/dev/null | sed -n 's/.*\"pid\":[[:space:]]*\\([0-9][0-9]*\\).*/\\1/p' | head -n 1); \
fi; \
if [ -z \"$PID\" ] && command -v docker >/dev/null 2>&1; then \
  PID=$(docker inspect -f '{{{{.State.Pid}}}}' \"$CONTAINER_ID\" 2>/dev/null); \
fi; \
if [ -z \"$PID\" ] && command -v nerdctl >/dev/null 2>&1; then \
  PID=$(nerdctl inspect --format '{{{{.State.Pid}}}}' \"$CONTAINER_ID\" 2>/dev/null); \
fi; \
if [ -z \"$PID\" ]; then \
  echo '未能解析容器 PID，请确认节点已安装 crictl/docker/nerdctl 且当前用户有权限。' >&2; \
  exit 1; \
fi; \
exec nsenter -t \"$PID\" {flags} /bin/sh -lc {shell_cmd}",
        cid = shell_single_quote(container_id),
        flags = ns_flags,
        shell_cmd = shell_single_quote(interactive_shell),
    )
}

fn resolve_pod_container_id(
    pod_value: &serde_json::Value,
    container_name: &str,
) -> Result<String, String> {
    let status = pod_value
        .get("status")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "Pod status 不存在，无法解析容器运行时信息".to_string())?;

    for field in ["containerStatuses", "initContainerStatuses"] {
        if let Some(items) = status.get(field).and_then(|v| v.as_array()) {
            for item in items {
                let name = item.get("name").and_then(|v| v.as_str()).unwrap_or_default();
                if name != container_name {
                    continue;
                }
                let raw_id = item
                    .get("containerID")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .trim();
                if raw_id.is_empty() {
                    return Err(format!("容器 {} 尚未运行，无法进入调试环境", container_name));
                }
                let normalized = raw_id
                    .split_once("://")
                    .map(|(_, id)| id)
                    .unwrap_or(raw_id)
                    .trim()
                    .to_string();
                if normalized.is_empty() {
                    return Err(format!("容器 {} 缺少有效 containerID", container_name));
                }
                return Ok(normalized);
            }
        }
    }

    Err(format!("未找到容器 {} 的运行时状态", container_name))
}

async fn resolve_pod_debug_command(
    kube_store: &KubeClientStore,
    env: &crate::env::Environment,
    target: &PodDebugTargetRequest,
) -> Result<String, String> {
    let client = kube_store.get_or_build(env).await.map_err(err_str)?;
    let pod_value = resource_get::get_resource_value(
        &client,
        "Pod",
        &target.pod_name,
        Some(&target.namespace),
    )
    .await
    .map_err(err_str)?;
    let container_id = if target.pid.is_some() {
        None
    } else {
        Some(resolve_pod_container_id(&pod_value, target.container.trim())?)
    };
    Ok(build_pod_nsenter_command(
        container_id.as_deref(),
        target.pid,
        &target.namespaces,
    ))
}

fn compile_node_terminal_steps(
    host: &str,
    steps: &[NodeTerminalStepRequest],
    final_command: Option<&str>,
) -> Result<(String, bool), String> {
    if steps.is_empty() {
        return Err("节点终端策略至少需要一个步骤".to_string());
    }

    let compiled = steps
        .iter()
        .rev()
        .try_fold(final_command.map(|item| item.to_string()), |next, step| {
        let user = step.user.trim();
        if user.is_empty() {
            return Err("节点终端步骤缺少 user".to_string());
        }
        let command = match step.r#type {
            NodeTerminalStepType::Ssh => {
                let ssh_cmd = format!("exec ssh {}@{}", user, host);
                if let Some(next_cmd) = next {
                    format!("ssh {}@{} -t {}", user, host, shell_single_quote(&next_cmd))
                } else {
                    ssh_cmd
                }
            }
            NodeTerminalStepType::SwitchUser => {
                let sudo_prompt = shell_single_quote(NODE_TERMINAL_SUDO_PROMPT);
                if let Some(next_cmd) = next {
                    format!(
                        "sudo -S -p {} su - {} -c {}",
                        sudo_prompt,
                        user,
                        shell_single_quote(&next_cmd)
                    )
                } else {
                    format!("sudo -S -p {} su - {}", sudo_prompt, user)
                }
            }
        };
            Ok::<Option<String>, String>(Some(command))
        })?;

    let needs_password = steps
        .iter()
        .any(|step| matches!(step.r#type, NodeTerminalStepType::SwitchUser));

    compiled
        .map(|command| (format!("{}\n", command), needs_password))
        .ok_or_else(|| "节点终端策略无法生成可执行命令".to_string())
}

pub struct HostShellSession {
    pub stdin_tx: mpsc::Sender<Vec<u8>>,
    pub resize_tx: Option<mpsc::Sender<(u16, u16)>>,
    pub abort_handle: tokio::task::AbortHandle,
}

impl SessionHandle for HostShellSession {
    fn abort_handle(&self) -> &tokio::task::AbortHandle { &self.abort_handle }
    fn stdin_tx(&self) -> &mpsc::Sender<Vec<u8>> { &self.stdin_tx }
    fn resize_tx(&self) -> Option<&mpsc::Sender<(u16, u16)>> { self.resize_tx.as_ref() }
}

pub type HostShellStore = SessionStore<HostShellSession>;

fn build_local_shell_command() -> Command {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd.exe");
        cmd.arg("/K");
        apply_no_window(&mut cmd);
        cmd
    }

    #[cfg(not(windows))]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let mut cmd = Command::new(shell);
        cmd.args(["-il"]);
        cmd.env("TERM", "xterm-256color")
            .env("LANG", "C.UTF-8")
            .env("LC_CTYPE", "C.UTF-8");
        apply_no_window(&mut cmd);
        cmd
    }
}

fn build_local_command(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd.exe");
        cmd.args(["/C", command]);
        apply_no_window(&mut cmd);
        cmd
    }

    #[cfg(not(windows))]
    {
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-lc", command]);
        cmd.env("TERM", "xterm-256color")
            .env("LANG", "C.UTF-8")
            .env("LC_CTYPE", "C.UTF-8");
        apply_no_window(&mut cmd);
        cmd
    }
}

#[cfg(unix)]
fn create_pty(cols: u16, rows: u16) -> Result<(std::fs::File, std::fs::File), String> {
    let mut master_fd = -1;
    let mut slave_fd = -1;
    let winsize = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    let rc = unsafe {
        libc::openpty(
            &mut master_fd,
            &mut slave_fd,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &winsize as *const libc::winsize as *mut libc::winsize,
        )
    };
    if rc != 0 {
        return Err(format!("openpty failed: {}", std::io::Error::last_os_error()));
    }

    let master = unsafe { std::fs::File::from_raw_fd(master_fd) };
    let slave = unsafe { std::fs::File::from_raw_fd(slave_fd) };
    Ok((master, slave))
}

#[cfg(unix)]
fn configure_child_pty(cmd: &mut Command, slave: std::fs::File) -> Result<(), String> {
    let stdin = slave
        .try_clone()
        .map_err(|e| format!("clone slave stdin failed: {}", e))?;
    let stdout = slave
        .try_clone()
        .map_err(|e| format!("clone slave stdout failed: {}", e))?;

    cmd.stdin(Stdio::from(stdin))
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(slave));

    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            let tty_path = CString::new("/dev/tty")
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid tty path"))?;
            let tty_fd = libc::open(tty_path.as_ptr(), libc::O_RDWR);
            if tty_fd >= 0 {
                libc::close(tty_fd);
            }
            Ok(())
        });
    }

    Ok(())
}

#[cfg(unix)]
fn resize_pty(file: &std::fs::File, cols: u16, rows: u16) -> Result<(), String> {
    use std::os::fd::AsRawFd;

    let winsize = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    let rc = unsafe { libc::ioctl(file.as_raw_fd(), libc::TIOCSWINSZ, &winsize) };
    if rc == -1 {
        return Err(format!("resize pty failed: {}", std::io::Error::last_os_error()));
    }
    Ok(())
}

fn build_remote_shell_command(
    tunnel: &crate::env::SshTunnel,
    manager: &CredentialManager,
    remote_cmd_override: Option<&str>,
) -> Result<(Command, Option<SshAskpassGuard>), String> {
    let ssh_host = &tunnel.ssh_host;
    let _host_config = ssh_config_get_host_config(ssh_host)
        .ok_or_else(|| format!("~/.ssh/config 中未找到 Host: {}", ssh_host))?;

    let mut cmd = Command::new("ssh");
    apply_no_window(&mut cmd);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.env("TERM", "xterm-256color")
        .env("LANG", "C.UTF-8")
        .env("LC_CTYPE", "C.UTF-8");

    let default_remote_cmd = "export TERM=\"xterm-256color\"; \
export LANG=\"${LANG:-C.UTF-8}\"; \
export LC_CTYPE=\"${LC_CTYPE:-$LANG}\"; \
if command -v bash >/dev/null 2>&1; then exec bash -il; else exec sh -i; fi";
    let remote_cmd = remote_cmd_override.unwrap_or(default_remote_cmd);

    #[cfg(unix)]
    {
        if tunnel.has_saved_credential {
            let settings = load_app_settings()?;
            let password = manager
                .get(&CredentialKey::new(&tunnel.id), &settings.security)
                .map_err(err_str)?;
            if let Some(ref pwd) = password {
                let askpass = SshAskpassGuard::new(pwd)
                    .map_err(|e| format!("创建 SSH_ASKPASS 失败: {}", e))?;
                let ap = askpass.path_str().to_string();
                cmd.env("SSH_ASKPASS", ap)
                    .env("SSH_ASKPASS_REQUIRE", "force")
                    .env("DISPLAY", ":0");
                cmd.args(["-tt", ssh_host, remote_cmd]);
                return Ok((cmd, Some(askpass)));
            }
        }
    }

    match tunnel.auth_method {
        AuthMethod::Password | AuthMethod::KeyboardInteractive => {
            cmd.args(["-o", "ConnectTimeout=10", "-tt", ssh_host, remote_cmd]);
        }
        AuthMethod::Auto | AuthMethod::PublicKey => {
            cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-tt", ssh_host, remote_cmd]);
        }
    }
    Ok((cmd, None))
}

async fn resolve_host_shell_automation(
    bootstrap: Option<HostShellBootstrapRequest>,
    manager: &CredentialManager,
    kube_store: &KubeClientStore,
    env: &crate::env::Environment,
) -> Result<Option<HostShellAutomation>, String> {
    let Some(bootstrap) = bootstrap else {
        return Ok(None);
    };
    if bootstrap.kind != "node_terminal" {
        return Err("unsupported host shell bootstrap kind".to_string());
    }

    let host = bootstrap.host.trim();
    if host.is_empty() {
        return Err("节点终端策略缺少 host".to_string());
    }

    let debug_command = if let Some(target) = bootstrap.pod_debug.as_ref() {
        Some(resolve_pod_debug_command(kube_store, env, target).await?)
    } else {
        None
    };

    let (command, needs_password) =
        compile_node_terminal_steps(host, &bootstrap.steps, debug_command.as_deref())?;

    let sudo_password = if needs_password {
        let credential_id = bootstrap.credential_id.unwrap_or_default();
        if credential_id.trim().is_empty() {
            None
        } else {
            let settings = load_app_settings()?;
            manager.get(&CredentialKey::new(credential_id), &settings.security)?
        }
    } else {
        None
    };

    Ok(Some(HostShellAutomation {
        command,
        sudo_password,
        sudo_prompt: if needs_password {
            Some(NODE_TERMINAL_SUDO_PROMPT.to_string())
        } else {
            None
        },
    }))
}

async fn run_host_shell_process(
    app: AppHandle,
    stream_id: String,
    mut cmd: Command,
    _askpass_guard: Option<SshAskpassGuard>,
    automation: Option<HostShellAutomation>,
    store: Arc<HostShellStore>,
) {
    cmd.kill_on_drop(true);

    #[cfg(unix)]
    let pty = match create_pty(80, 24) {
        Ok(pair) => Some(pair),
        Err(e) => {
            let _ = app.emit(HOST_SHELL_END_EVENT, serde_json::json!({
                "stream_id": stream_id,
                "error": format!("无法创建终端 PTY: {}", e)
            }));
            return;
        }
    };

    #[cfg(unix)]
    let (master_file, slave_file) = pty.unwrap();

    #[cfg(unix)]
    if let Err(e) = configure_child_pty(&mut cmd, slave_file) {
        let _ = app.emit(HOST_SHELL_END_EVENT, serde_json::json!({
            "stream_id": stream_id,
            "error": format!("无法配置终端 PTY: {}", e)
        }));
        return;
    }

    #[cfg(not(unix))]
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let end_error = match cmd.spawn() {
        Ok(mut child) => {
            let (stdin_tx, mut stdin_rx) = mpsc::channel::<Vec<u8>>(64);
            let (resize_tx, mut resize_rx) = mpsc::channel::<(u16, u16)>(8);
            let stream_id_clone = stream_id.clone();
            let app_clone = app.clone();

            #[cfg(unix)]
            let resize_file = match master_file.try_clone() {
                Ok(file) => file,
                Err(e) => {
                    let _ = app.emit(HOST_SHELL_END_EVENT, serde_json::json!({
                        "stream_id": stream_id,
                        "error": format!("无法克隆 PTY resize 端: {}", e)
                    }));
                    return;
                }
            };

            #[cfg(unix)]
            let mut writer = match master_file.try_clone() {
                Ok(file) => tokio::fs::File::from_std(file),
                Err(e) => {
                    let _ = app.emit(HOST_SHELL_END_EVENT, serde_json::json!({
                        "stream_id": stream_id,
                        "error": format!("无法克隆 PTY 写入端: {}", e)
                    }));
                    return;
                }
            };

            #[cfg(unix)]
            let mut reader = tokio::fs::File::from_std(master_file);

            #[cfg(not(unix))]
            let mut stdin = match child.stdin.take() {
                Some(v) => v,
                None => {
                    let _ = app.emit(HOST_SHELL_END_EVENT, serde_json::json!({
                        "stream_id": stream_id,
                        "error": "无法获取主机 Shell stdin"
                    }));
                    return;
                }
            };
            #[cfg(not(unix))]
            let mut stdout = child.stdout.take();
            #[cfg(not(unix))]
            let mut stderr = child.stderr.take();

            #[cfg(unix)]
            let task: tokio::task::JoinHandle<Option<String>> = tokio::spawn(async move {
                let mut read_buf = [0u8; 4096];
                let mut automation = automation.map(HostShellAutomationState::new);

                loop {
                    tokio::select! {
                        Some(data) = stdin_rx.recv() => {
                            if let Err(e) = writer.write_all(&data).await {
                                break Some(format!("stdin write failed: {}", e));
                            }
                            let _ = writer.flush().await;
                        }
                        Some((cols, rows)) = resize_rx.recv() => {
                            if let Err(e) = resize_pty(&resize_file, cols, rows) {
                                break Some(e);
                            }
                        }
                        result = reader.read(&mut read_buf) => {
                            match result {
                                Ok(0) => break None,
                                Ok(n) => {
                                    let (output_chunk, password_input) = if let Some(state) = automation.as_mut() {
                                        state.process_output(&read_buf[..n])
                                    } else {
                                        (read_buf[..n].to_vec(), None)
                                    };
                                    if !output_chunk.is_empty() && app_clone.emit(HOST_SHELL_CHUNK_EVENT, serde_json::json!({
                                        "stream_id": stream_id_clone,
                                        "chunk_bytes": output_chunk
                                    })).is_err() {
                                        break Some("emit pty chunk failed".to_string());
                                    }
                                    if let Some(password_input) = password_input {
                                        if let Err(e) = writer.write_all(&password_input).await {
                                            break Some(format!("automation password write failed: {}", e));
                                        }
                                        let _ = writer.flush().await;
                                    }
                                }
                                Err(e) => break Some(format!("pty read failed: {}", e)),
                            }
                        }
                        status = child.wait() => {
                            match status {
                                Ok(exit) if exit.success() => break None,
                                Ok(exit) => break Some(format!("shell exited with status {}", exit)),
                                Err(e) => break Some(format!("shell wait failed: {}", e)),
                            }
                        }
                    }
                }
            });

            #[cfg(not(unix))]
            let task: tokio::task::JoinHandle<Option<String>> = tokio::spawn(async move {
                let mut stdout_buf = [0u8; 4096];
                let mut stderr_buf = [0u8; 4096];
                let mut automation = automation.map(HostShellAutomationState::new);

                loop {
                    tokio::select! {
                        Some(data) = stdin_rx.recv() => {
                            if let Err(e) = stdin.write_all(&data).await {
                                break Some(format!("stdin write failed: {}", e));
                            }
                            let _ = stdin.flush().await;
                        }
                        Some((cols, rows)) = resize_rx.recv() => {
                            let _ = (cols, rows);
                        }
                        result = async {
                            if let Some(ref mut out) = stdout {
                                out.read(&mut stdout_buf).await
                            } else {
                                Ok(0)
                            }
                        } => {
                            match result {
                                Ok(0) => {
                                    stdout = None;
                                    if stderr.is_none() {
                                        break None;
                                    }
                                }
                                Ok(n) => {
                                    let (output_chunk, password_input) = if let Some(state) = automation.as_mut() {
                                        state.process_output(&stdout_buf[..n])
                                    } else {
                                        (stdout_buf[..n].to_vec(), None)
                                    };
                                    if !output_chunk.is_empty() && app_clone.emit(HOST_SHELL_CHUNK_EVENT, serde_json::json!({
                                        "stream_id": stream_id_clone,
                                        "chunk_bytes": output_chunk
                                    })).is_err() {
                                        break Some("emit stdout chunk failed".to_string());
                                    }
                                    if let Some(password_input) = password_input {
                                        if let Err(e) = stdin.write_all(&password_input).await {
                                            break Some(format!("automation password write failed: {}", e));
                                        }
                                        let _ = stdin.flush().await;
                                    }
                                }
                                Err(e) => break Some(format!("stdout read failed: {}", e)),
                            }
                        }
                        result = async {
                            if let Some(ref mut err) = stderr {
                                err.read(&mut stderr_buf).await
                            } else {
                                Ok(0)
                            }
                        } => {
                            match result {
                                Ok(0) => {
                                    stderr = None;
                                    if stdout.is_none() {
                                        break None;
                                    }
                                }
                                Ok(n) => {
                                    let (output_chunk, password_input) = if let Some(state) = automation.as_mut() {
                                        state.process_output(&stderr_buf[..n])
                                    } else {
                                        (stderr_buf[..n].to_vec(), None)
                                    };
                                    if !output_chunk.is_empty() && app_clone.emit(HOST_SHELL_CHUNK_EVENT, serde_json::json!({
                                        "stream_id": stream_id_clone,
                                        "chunk_bytes": output_chunk
                                    })).is_err() {
                                        break Some("emit stderr chunk failed".to_string());
                                    }
                                    if let Some(password_input) = password_input {
                                        if let Err(e) = stdin.write_all(&password_input).await {
                                            break Some(format!("automation password write failed: {}", e));
                                        }
                                        let _ = stdin.flush().await;
                                    }
                                }
                                Err(e) => break Some(format!("stderr read failed: {}", e)),
                            }
                        }
                        status = child.wait() => {
                            match status {
                                Ok(exit) if exit.success() => break None,
                                Ok(exit) => break Some(format!("shell exited with status {}", exit)),
                                Err(e) => break Some(format!("shell wait failed: {}", e)),
                            }
                        }
                    }
                }
            });

            let abort_handle = task.abort_handle();
            store
                .insert(
                    stream_id.clone(),
                    HostShellSession {
                        stdin_tx,
                        #[cfg(unix)]
                        resize_tx: Some(resize_tx),
                        #[cfg(not(unix))]
                        resize_tx: None,
                        abort_handle,
                    },
                )
                .await;

            match task.await {
                Ok(err) => err,
                Err(e) if e.is_cancelled() => None,
                Err(e) => Some(format!("host shell task join failed: {}", e)),
            }
        }
        Err(e) => Some(format!("启动主机 Shell 失败: {}", e)),
    };

    store.remove(&stream_id).await;
    let _ = app.emit(
        HOST_SHELL_END_EVENT,
        serde_json::json!({
            "stream_id": stream_id,
            "error": end_error
        }),
    );
}

#[tauri::command]
pub async fn host_shell_start(
    app: AppHandle,
    manager: State<'_, CredentialManager>,
    store: State<'_, Arc<HostShellStore>>,
    kube_store: State<'_, KubeClientStore>,
    env_id: String,
    bootstrap: Option<HostShellBootstrapRequest>,
) -> CommandResult<String> {
    let env = EnvService::list()
        .map_err(err_str)?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;

    let stream_id = Uuid::new_v4().to_string();
    let stream_id_clone = stream_id.clone();
    let app_handle = app.clone();
    let store_clone = store.inner().clone();
    let automation = resolve_host_shell_automation(bootstrap, &manager, &kube_store, &env).await?;
    let bootstrap_command = automation.as_ref().map(|item| item.command.as_str());

    match env.source {
        EnvironmentSource::LocalKubeconfig => {
            #[cfg(windows)]
            {
                return Err("Windows 仅支持通过 SSH 打开主机 Shell，不支持本地环境 Shell。".to_string());
            }

            let cmd = if let Some(command) = bootstrap_command {
                build_local_command(command)
            } else {
                build_local_shell_command()
            };
            let automation_plan = automation.clone();
            tokio::spawn(async move {
                run_host_shell_process(app_handle, stream_id_clone, cmd, None, automation_plan, store_clone).await;
            });
        }
        EnvironmentSource::SshTunnel => {
            let tunnel_id = env
                .ssh_tunnel_id
                .clone()
                .ok_or_else(|| "环境缺少 ssh_tunnel_id".to_string())?;
            let tunnel = EnvService::get_ssh_tunnel(&tunnel_id)
                .map_err(err_str)?
                .ok_or_else(|| format!("未找到隧道配置: {}", tunnel_id))?;
            let (cmd, askpass_guard) =
                build_remote_shell_command(&tunnel, &manager, bootstrap_command)?;
            tokio::spawn(async move {
                run_host_shell_process(
                    app_handle,
                    stream_id_clone,
                    cmd,
                    askpass_guard,
                    automation,
                    store_clone,
                )
                .await;
            });
        }
    }

    Ok(stream_id)
}

#[tauri::command]
pub async fn host_shell_stdin(
    store: State<'_, Arc<HostShellStore>>,
    stream_id: String,
    data: Vec<u8>,
) -> CommandResult<()> {
    store.send_stdin(&stream_id, data).await
}

#[tauri::command]
pub async fn host_shell_resize(
    store: State<'_, Arc<HostShellStore>>,
    stream_id: String,
    cols: u16,
    rows: u16,
) -> CommandResult<()> {
    store.send_resize(&stream_id, cols, rows).await
}

#[tauri::command]
pub async fn host_shell_stop(
    store: State<'_, Arc<HostShellStore>>,
    stream_id: String,
) -> CommandResult<()> {
    store.stop(&stream_id).await;
    Ok(())
}
