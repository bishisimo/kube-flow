use crate::config::{app_settings_config_path, ssh_config_get_host_config, AppSettingsConfig};
use crate::credentials::{CredentialKey, CredentialManager};
use crate::env::{EnvService, EnvironmentSource};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

const HOST_SHELL_CHUNK_EVENT: &str = "host-shell-chunk";
const HOST_SHELL_END_EVENT: &str = "host-shell-end";

#[cfg(windows)]
fn apply_no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn apply_no_window(_: &mut Command) {}

#[cfg(unix)]
struct SshAskpassGuard {
    path: std::path::PathBuf,
}

#[cfg(unix)]
impl SshAskpassGuard {
    fn new(password: &str) -> Result<Self, std::io::Error> {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("kf-host-askpass-{}.sh", Uuid::new_v4()));
        let escaped = password.replace('\'', "'\\''");
        let content = format!("#!/bin/sh\nprintf '%s' '{}'\n", escaped);
        std::fs::write(&path, &content)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
        Ok(Self { path })
    }

    fn path_str(&self) -> &str {
        self.path.to_str().unwrap_or("")
    }
}

#[cfg(unix)]
impl Drop for SshAskpassGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub struct HostShellSession {
    pub stdin_tx: mpsc::Sender<Vec<u8>>,
    pub abort_handle: tokio::task::AbortHandle,
}

pub struct HostShellStore {
    sessions: Arc<RwLock<HashMap<String, HostShellSession>>>,
}

impl HostShellStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn insert(&self, stream_id: String, session: HostShellSession) {
        let mut guard = self.sessions.write().await;
        if let Some(old) = guard.insert(stream_id, session) {
            old.abort_handle.abort();
        }
    }

    pub async fn remove(&self, stream_id: &str) -> Option<HostShellSession> {
        let mut guard = self.sessions.write().await;
        guard.remove(stream_id)
    }

    pub async fn send_stdin(&self, stream_id: &str, data: Vec<u8>) -> Result<(), String> {
        let guard = self.sessions.read().await;
        let session = guard.get(stream_id).ok_or_else(|| "session not found".to_string())?;
        session.stdin_tx.send(data).await.map_err(|e| e.to_string())
    }

    pub async fn stop(&self, stream_id: &str) {
        if let Some(session) = self.remove(stream_id).await {
            session.abort_handle.abort();
        }
    }
}

impl Default for HostShellStore {
    fn default() -> Self {
        Self::new()
    }
}

fn load_settings() -> Result<AppSettingsConfig, String> {
    let path = app_settings_config_path().ok_or("app data dir 不可用".to_string())?;
    AppSettingsConfig::load(&path).map_err(|e| e.to_string())
}

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

fn build_remote_shell_command(
    ssh_host: &str,
    tunnel_id: &str,
    manager: &CredentialManager,
) -> Result<(Command, Option<SshAskpassGuard>), String> {
    let _host_config = ssh_config_get_host_config(ssh_host)
        .ok_or_else(|| format!("~/.ssh/config 中未找到 Host: {}", ssh_host))?;

    let mut cmd = Command::new("ssh");
    apply_no_window(&mut cmd);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let remote_cmd = "export TERM=\"${TERM:-xterm-256color}\"; \
export LANG=\"${LANG:-C.UTF-8}\"; \
export LC_CTYPE=\"${LC_CTYPE:-$LANG}\"; \
if command -v bash >/dev/null 2>&1; then exec bash -il; else exec sh -i; fi";

    #[cfg(unix)]
    {
        let settings = load_settings()?;
        let password = manager
            .get(&CredentialKey::new(tunnel_id), &settings.security)
            .map_err(|e| e.to_string())?;
        if let Some(ref pwd) = password {
            let askpass = SshAskpassGuard::new(pwd).map_err(|e| format!("创建 SSH_ASKPASS 失败: {}", e))?;
            let ap = askpass.path_str().to_string();
            cmd.env("SSH_ASKPASS", ap)
                .env("SSH_ASKPASS_REQUIRE", "force")
                .env("DISPLAY", ":0");
            cmd.args(["-tt", ssh_host, remote_cmd]);
            return Ok((cmd, Some(askpass)));
        }
    }

    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-tt", ssh_host, remote_cmd]);
    Ok((cmd, None))
}

async fn run_host_shell_process(
    app: AppHandle,
    stream_id: String,
    mut cmd: Command,
    _askpass_guard: Option<SshAskpassGuard>,
    store: Arc<HostShellStore>,
) {
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let end_error = match cmd.spawn() {
        Ok(mut child) => {
            let (stdin_tx, mut stdin_rx) = mpsc::channel::<Vec<u8>>(64);
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
            let mut stdout = child.stdout.take();
            let mut stderr = child.stderr.take();
            let stream_id_clone = stream_id.clone();
            let app_clone = app.clone();

            let task: tokio::task::JoinHandle<Option<String>> = tokio::spawn(async move {
                let mut stdout_buf = [0u8; 4096];
                let mut stderr_buf = [0u8; 4096];
                loop {
                    tokio::select! {
                        Some(data) = stdin_rx.recv() => {
                            if let Err(e) = stdin.write_all(&data).await {
                                break Some(format!("stdin write failed: {}", e));
                            }
                            let _ = stdin.flush().await;
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
                                    if app_clone.emit(HOST_SHELL_CHUNK_EVENT, serde_json::json!({
                                        "stream_id": stream_id_clone,
                                        "chunk_bytes": &stdout_buf[..n]
                                    })).is_err() {
                                        break Some("emit stdout chunk failed".to_string());
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
                                    if app_clone.emit(HOST_SHELL_CHUNK_EVENT, serde_json::json!({
                                        "stream_id": stream_id_clone,
                                        "chunk_bytes": &stderr_buf[..n]
                                    })).is_err() {
                                        break Some("emit stderr chunk failed".to_string());
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
    env_id: String,
) -> Result<String, String> {
    let env = EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())?;

    let stream_id = Uuid::new_v4().to_string();
    let stream_id_clone = stream_id.clone();
    let app_handle = app.clone();
    let store_clone = store.inner().clone();

    match env.source {
        EnvironmentSource::LocalKubeconfig => {
            let cmd = build_local_shell_command();
            tokio::spawn(async move {
                run_host_shell_process(app_handle, stream_id_clone, cmd, None, store_clone).await;
            });
        }
        EnvironmentSource::SshTunnel => {
            let tunnel_id = env
                .ssh_tunnel_id
                .clone()
                .ok_or_else(|| "环境缺少 ssh_tunnel_id".to_string())?;
            let tunnel = EnvService::get_ssh_tunnel(&tunnel_id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("未找到隧道配置: {}", tunnel_id))?;
            let (cmd, askpass_guard) = build_remote_shell_command(&tunnel.ssh_host, &tunnel.id, &manager)?;
            tokio::spawn(async move {
                run_host_shell_process(app_handle, stream_id_clone, cmd, askpass_guard, store_clone).await;
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
) -> Result<(), String> {
    store.send_stdin(&stream_id, data).await
}

#[tauri::command]
pub async fn host_shell_resize(
    _store: State<'_, Arc<HostShellStore>>,
    _stream_id: String,
    _cols: u16,
    _rows: u16,
) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn host_shell_stop(
    store: State<'_, Arc<HostShellStore>>,
    stream_id: String,
) -> Result<(), String> {
    store.stop(&stream_id).await;
    Ok(())
}
