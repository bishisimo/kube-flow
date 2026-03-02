//! Pod Exec：在 Pod 容器内执行交互式 shell，支持 TTY、stdin/stdout、resize。
//! 通过 Tauri 事件推送 stdout，通过 invoke 接收 stdin 与 resize。

use futures::SinkExt;
use kube::api::{Api, AttachParams};
use kube::Client;
use k8s_openapi::api::core::v1::Pod;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, RwLock};

const POD_EXEC_CHUNK_EVENT: &str = "pod-exec-chunk";
const POD_EXEC_END_EVENT: &str = "pod-exec-end";

/// 单个 exec 会话的可写端：stdin 与 resize 的发送通道。
pub struct PodExecSession {
    pub stdin_tx: mpsc::Sender<Vec<u8>>,
    pub resize_tx: Option<mpsc::Sender<(u16, u16)>>,
    pub abort_handle: tokio::task::AbortHandle,
}

/// 按 stream_id 存储活跃的 Pod exec 会话。
pub struct PodExecStore {
    sessions: Arc<RwLock<HashMap<String, PodExecSession>>>,
}

impl PodExecStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn insert(&self, stream_id: String, session: PodExecSession) {
        let mut guard = self.sessions.write().await;
        if let Some(old) = guard.insert(stream_id, session) {
            old.abort_handle.abort();
        }
    }

    pub async fn remove(&self, stream_id: &str) -> Option<PodExecSession> {
        let mut guard = self.sessions.write().await;
        guard.remove(stream_id)
    }

    pub async fn send_stdin(&self, stream_id: &str, data: Vec<u8>) -> Result<(), String> {
        let guard = self.sessions.read().await;
        let session = guard.get(stream_id).ok_or_else(|| "session not found".to_string())?;
        session.stdin_tx.send(data).await.map_err(|e| e.to_string())
    }

    pub async fn send_resize(&self, stream_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let guard = self.sessions.read().await;
        let session = guard.get(stream_id).ok_or_else(|| "session not found".to_string())?;
        if let Some(ref tx) = session.resize_tx {
            tx.send((cols, rows)).await.map_err(|e| e.to_string())
        } else {
            Err("session has no tty".to_string())
        }
    }

    pub async fn stop(&self, stream_id: &str) {
        if let Some(session) = self.remove(stream_id).await {
            session.abort_handle.abort();
        }
    }
}

impl Default for PodExecStore {
    fn default() -> Self {
        Self::new()
    }
}

/// 启动 Pod exec 交互式 shell，通过 Tauri 事件推送 stdout。
/// 命令：优先 bash，fallback 到 sh。
#[allow(clippy::too_many_arguments)]
pub async fn run_pod_exec(
    app: AppHandle,
    stream_id: String,
    client: Client,
    namespace: String,
    pod_name: String,
    container: Option<String>,
    store: Arc<PodExecStore>,
) {
    let api: Api<Pod> = Api::namespaced(client, &namespace);
    let mut attach_params = AttachParams::interactive_tty()
        .stdin(true)
        .stdout(true)
        .stderr(false)
        .tty(true);
    if let Some(ref c) = container {
        attach_params = attach_params.container(c.clone());
    }

    let command: Vec<String> = vec![
        "/bin/sh".into(),
        "-c".into(),
        "if command -v bash >/dev/null 2>&1; then exec bash; else exec sh; fi".into(),
    ];

    let stream_id_final = stream_id.clone();

    match api.exec(&pod_name, command, &attach_params).await {
        Ok(mut attached) => {
            let (stdin_tx, mut stdin_rx) = mpsc::channel::<Vec<u8>>(64);
            let (resize_tx, mut resize_rx) = mpsc::channel::<(u16, u16)>(8);

            let mut stdout = attached.stdout();
            let mut stdin_opt = attached.stdin();
            let mut terminal_size_opt = attached.terminal_size();
            let has_resize = terminal_size_opt.is_some();

            let stream_id_clone = stream_id.clone();
            let app_clone = app.clone();

            let task = tokio::spawn(async move {
                let mut read_buf = [0u8; 4096];

                loop {
                    tokio::select! {
                        biased;

                        Some(data) = stdin_rx.recv() => {
                            if let Some(ref mut stdin) = stdin_opt {
                                let _ = stdin.write_all(&data).await;
                                let _ = stdin.flush().await;
                            }
                        }

                        Some((cols, rows)) = resize_rx.recv() => {
                            if let Some(ref mut ts) = terminal_size_opt {
                                let size = kube::api::TerminalSize {
                                    width: cols,
                                    height: rows,
                                };
                                let _ = ts.send(size).await;
                            }
                        }

                        result = async {
                            if let Some(ref mut stdout) = stdout {
                                stdout.read(&mut read_buf).await
                            } else {
                                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                                Ok(0)
                            }
                        } => {
                            match result {
                                Ok(0) => break,
                                Ok(n) => {
                                    let chunk = String::from_utf8_lossy(&read_buf[..n]).to_string();
                                    if app_clone.emit(POD_EXEC_CHUNK_EVENT, serde_json::json!({
                                        "stream_id": stream_id_clone,
                                        "chunk": chunk
                                    })).is_err() {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    }
                }
            });

            let abort_handle = task.abort_handle();
            store
                .insert(
                    stream_id,
                    PodExecSession {
                        stdin_tx,
                        resize_tx: if has_resize { Some(resize_tx) } else { None },
                        abort_handle,
                    },
                )
                .await;

            let _ = task.await;
        }
        Err(e) => {
            let _ = app.emit(
                POD_EXEC_END_EVENT,
                serde_json::json!({
                    "stream_id": stream_id,
                    "error": e.to_string()
                }),
            );
        }
    }

    store.remove(&stream_id_final).await;
    let _ = app.emit(
        POD_EXEC_END_EVENT,
        serde_json::json!({ "stream_id": stream_id_final }),
    );
}
