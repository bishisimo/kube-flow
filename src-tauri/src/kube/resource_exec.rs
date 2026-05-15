//! Pod Exec：在 Pod 容器内执行交互式 shell，支持 TTY、stdin/stdout、resize。
//! 通过 Tauri 事件推送 stdout，通过 invoke 接收 stdin 与 resize。

use crate::config::LogLevel;
use crate::debug_log::{self, DebugEntry};
use crate::kube::session_store::{SessionHandle, SessionStore};
use futures::SinkExt;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, AttachParams};
use kube::Client;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

const POD_EXEC_CHUNK_EVENT: &str = "pod-exec-chunk";
const POD_EXEC_END_EVENT: &str = "pod-exec-end";

fn log_pod_shell(
    level: LogLevel,
    result: &str,
    stream_id: &str,
    namespace: &str,
    pod_name: &str,
    container: Option<&str>,
    error: Option<&str>,
) {
    let mut detail = format!(
        "stream_id={} namespace={} pod={}",
        stream_id, namespace, pod_name
    );
    if let Some(c) = container.filter(|v| !v.is_empty()) {
        detail.push_str(&format!(" container={}", c));
    }
    debug_log::log_debug_entry(DebugEntry {
        ts: chrono::Utc::now().to_rfc3339(),
        level: level.as_str().to_string(),
        resource: "pod_shell".to_string(),
        env_id: None,
        result: Some(result.to_string()),
        item_count: None,
        error: error.map(ToString::to_string),
        raw_sample: Some(detail),
    });
}

fn normalize_pod_exec_start_error(raw: String) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("no such file or directory")
        || lower.contains("executable file not found")
        || lower.contains("not found in $path")
        || lower.contains("container_linux.go")
    {
        return "容器内未发现可用 shell（/bin/sh）。该镜像可能为 distroless/极简镜像，请使用调试容器或节点终端。"
            .to_string();
    }
    raw
}

/// 单个 exec 会话的可写端：stdin 与 resize 的发送通道。
pub struct PodExecSession {
    pub stdin_tx: mpsc::Sender<Vec<u8>>,
    pub resize_tx: Option<mpsc::Sender<(u16, u16)>>,
    pub abort_handle: tokio::task::AbortHandle,
}

impl SessionHandle for PodExecSession {
    fn abort_handle(&self) -> &tokio::task::AbortHandle {
        &self.abort_handle
    }
    fn stdin_tx(&self) -> &mpsc::Sender<Vec<u8>> {
        &self.stdin_tx
    }
    fn resize_tx(&self) -> Option<&mpsc::Sender<(u16, u16)>> {
        self.resize_tx.as_ref()
    }
}

/// 按 stream_id 存储活跃的 Pod exec 会话。
pub type PodExecStore = SessionStore<PodExecSession>;

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
    log_pod_shell(
        LogLevel::Info,
        "start",
        &stream_id,
        &namespace,
        &pod_name,
        container.as_deref(),
        None,
    );
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
        "export TERM=\"xterm-256color\"; \
export LANG=\"${LANG:-C.UTF-8}\"; \
export LC_CTYPE=\"${LC_CTYPE:-$LANG}\"; \
if command -v bash >/dev/null 2>&1; then exec bash -il; else exec sh -i; fi"
            .into(),
    ];

    let stream_id_final = stream_id.clone();
    let end_error: Option<String> = match api.exec(&pod_name, command, &attach_params).await {
        Ok(mut attached) => {
            let (stdin_tx, mut stdin_rx) = mpsc::channel::<Vec<u8>>(64);
            let (resize_tx, mut resize_rx) = mpsc::channel::<(u16, u16)>(8);

            let mut stdout = attached.stdout();
            let mut stdin_opt = attached.stdin();
            let mut terminal_size_opt = attached.terminal_size();
            let has_resize = terminal_size_opt.is_some();

            let stream_id_clone = stream_id.clone();
            let app_clone = app.clone();

            let task: tokio::task::JoinHandle<Option<String>> = tokio::spawn(async move {
                let mut read_buf = [0u8; 4096];
                let end_reason = loop {
                    tokio::select! {
                        Some(data) = stdin_rx.recv() => {
                            if let Some(ref mut stdin) = stdin_opt {
                                let mut merged = data;
                                while let Ok(extra) = stdin_rx.try_recv() {
                                    merged.extend_from_slice(&extra);
                                }
                                if let Err(e) = stdin.write_all(&merged).await {
                                    break Some(format!("stdin write failed: {}", e));
                                }
                            }
                        }

                        Some((cols, rows)) = resize_rx.recv() => {
                            if let Some(ref mut ts) = terminal_size_opt {
                                let size = kube::api::TerminalSize {
                                    width: cols,
                                    height: rows,
                                };
                                if let Err(e) = ts.send(size).await {
                                    break Some(format!("terminal resize failed: {}", e));
                                }
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
                                Ok(0) => break None,
                                Ok(n) => {
                                    if app_clone.emit(POD_EXEC_CHUNK_EVENT, serde_json::json!({
                                        "stream_id": stream_id_clone,
                                        "chunk_bytes": &read_buf[..n]
                                    })).is_err() {
                                        break Some("emit chunk failed".to_string());
                                    }
                                }
                                Err(e) => break Some(format!("stdout read failed: {}", e)),
                            }
                        }
                    }
                };
                end_reason
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

            match task.await {
                Ok(err) => err,
                Err(e) if e.is_cancelled() => None,
                Err(e) => Some(format!("exec task join failed: {}", e)),
            }
        }
        Err(e) => Some(normalize_pod_exec_start_error(e.to_string())),
    };

    store.remove(&stream_id_final).await;
    match end_error.as_deref() {
        Some(err) => log_pod_shell(
            LogLevel::Error,
            "end",
            &stream_id_final,
            &namespace,
            &pod_name,
            container.as_deref(),
            Some(err),
        ),
        None => log_pod_shell(
            LogLevel::Info,
            "end",
            &stream_id_final,
            &namespace,
            &pod_name,
            container.as_deref(),
            None,
        ),
    }
    let _ = app.emit(
        POD_EXEC_END_EVENT,
        serde_json::json!({
            "stream_id": stream_id_final,
            "error": end_error
        }),
    );
}
