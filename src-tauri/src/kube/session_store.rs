//! 泛型会话存储：管理按 stream_id 索引的活跃会话（Pod exec、Host shell 等）。

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// 会话必须提供的能力：abort 句柄、stdin 发送通道、可选的 resize 发送通道。
pub trait SessionHandle: Send + Sync + 'static {
    fn abort_handle(&self) -> &tokio::task::AbortHandle;
    fn stdin_tx(&self) -> &mpsc::Sender<Vec<u8>>;
    fn resize_tx(&self) -> Option<&mpsc::Sender<(u16, u16)>>;
}

/// 泛型会话存储，供 PodExecStore 和 HostShellStore 复用。
pub struct SessionStore<S: SessionHandle> {
    sessions: Arc<RwLock<HashMap<String, S>>>,
}

impl<S: SessionHandle> SessionStore<S> {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 插入会话；若 stream_id 已存在则 abort 旧会话。
    pub async fn insert(&self, stream_id: String, session: S) {
        let mut guard = self.sessions.write().await;
        if let Some(old) = guard.insert(stream_id, session) {
            old.abort_handle().abort();
        }
    }

    /// 移除并返回会话。
    pub async fn remove(&self, stream_id: &str) -> Option<S> {
        let mut guard = self.sessions.write().await;
        guard.remove(stream_id)
    }

    /// 向指定会话发送 stdin 数据。
    pub async fn send_stdin(&self, stream_id: &str, data: Vec<u8>) -> Result<(), String> {
        let guard = self.sessions.read().await;
        let session = guard.get(stream_id).ok_or_else(|| "session not found".to_string())?;
        session.stdin_tx().send(data).await.map_err(|e| e.to_string())
    }

    /// 向指定会话发送终端 resize 事件；会话无 TTY 时返回 Err。
    pub async fn send_resize(&self, stream_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let guard = self.sessions.read().await;
        let session = guard.get(stream_id).ok_or_else(|| "session not found".to_string())?;
        if let Some(tx) = session.resize_tx() {
            tx.send((cols, rows)).await.map_err(|e| e.to_string())
        } else {
            Err("session has no tty".to_string())
        }
    }

    /// 停止指定会话（移除并 abort）。
    pub async fn stop(&self, stream_id: &str) {
        if let Some(session) = self.remove(stream_id).await {
            session.abort_handle().abort();
        }
    }
}

impl<S: SessionHandle> Default for SessionStore<S> {
    fn default() -> Self {
        Self::new()
    }
}
