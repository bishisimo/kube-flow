//! 文件传输会话：进度事件、取消令牌与统一结束事件。

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, RwLock};

pub const FILE_TRANSFER_PROGRESS_EVENT: &str = "file-transfer-progress";
pub const FILE_TRANSFER_END_EVENT: &str = "file-transfer-end";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTransferProgressPayload {
    pub transfer_id: String,
    pub transferred_bytes: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTransferEndPayload {
    pub transfer_id: String,
    pub error: Option<String>,
}

struct FileTransferSession {
    cancel_tx: oneshot::Sender<()>,
}

/// 按 transfer_id 管理可取消的文件传输任务。
pub struct FileTransferStore {
    sessions: Arc<RwLock<HashMap<String, FileTransferSession>>>,
}

impl FileTransferStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册可取消会话，返回接收端供传输任务监听。
    pub async fn register(&self, transfer_id: String) -> oneshot::Receiver<()> {
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let mut guard = self.sessions.write().await;
        if let Some(old) = guard.insert(transfer_id, FileTransferSession { cancel_tx }) {
            let _ = old.cancel_tx.send(());
        }
        cancel_rx
    }

    /// 取消指定传输；若不存在则忽略。
    pub async fn cancel(&self, transfer_id: &str) {
        if let Some(session) = self.sessions.write().await.remove(transfer_id) {
            let _ = session.cancel_tx.send(());
        }
    }

    pub async fn remove(&self, transfer_id: &str) {
        self.sessions.write().await.remove(transfer_id);
    }
}

impl Default for FileTransferStore {
    fn default() -> Self {
        Self::new()
    }
}

pub fn emit_progress(
    app: &AppHandle,
    transfer_id: &str,
    transferred_bytes: u64,
    total_bytes: Option<u64>,
) {
    let _ = app.emit(
        FILE_TRANSFER_PROGRESS_EVENT,
        FileTransferProgressPayload {
            transfer_id: transfer_id.to_string(),
            transferred_bytes,
            total_bytes,
        },
    );
}

/// 节流进度上报：起点、终点立即发；中间按时间间隔发，避免大文件刷爆事件。
pub struct ProgressEmitter {
    last_emit: std::time::Instant,
    interval: std::time::Duration,
}

impl ProgressEmitter {
    pub fn new() -> Self {
        Self {
            last_emit: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(1))
                .unwrap_or_else(std::time::Instant::now),
            interval: std::time::Duration::from_millis(120),
        }
    }

    pub fn emit(
        &mut self,
        app: &AppHandle,
        transfer_id: &str,
        transferred_bytes: u64,
        total_bytes: Option<u64>,
    ) {
        let is_start = transferred_bytes == 0;
        let is_done = total_bytes.is_some_and(|total| transferred_bytes >= total);
        if is_start || is_done || self.last_emit.elapsed() >= self.interval {
            emit_progress(app, transfer_id, transferred_bytes, total_bytes);
            self.last_emit = std::time::Instant::now();
        }
    }
}

pub fn emit_end(app: &AppHandle, transfer_id: &str, error: Option<String>) {
    let _ = app.emit(
        FILE_TRANSFER_END_EVENT,
        FileTransferEndPayload {
            transfer_id: transfer_id.to_string(),
            error,
        },
    );
}

/// 若已收到取消信号则返回错误。
pub fn check_cancelled(cancel_rx: &mut oneshot::Receiver<()>) -> Result<(), String> {
    match cancel_rx.try_recv() {
        Ok(()) => Err("已取消".to_string()),
        Err(oneshot::error::TryRecvError::Empty) => Ok(()),
        Err(oneshot::error::TryRecvError::Closed) => Ok(()),
    }
}
