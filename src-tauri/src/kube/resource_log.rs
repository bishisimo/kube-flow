//! Pod 日志获取：通过 K8s log subresource 拉取 Pod 容器 stdout/stderr。
//! 支持一次性拉取与流式 follow。

use futures::{AsyncBufReadExt, TryStreamExt};
use kube::api::{Api, LogParams};
use kube::Client;
use k8s_openapi::api::core::v1::Pod;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use crate::kube::resource_get;

const POD_LOG_CHUNK_EVENT: &str = "pod-log-chunk";
const POD_LOG_STREAM_END_EVENT: &str = "pod-log-stream-end";

/// 获取 Pod 的容器名称列表（含 initContainers），用于日志页面的容器选择。
pub async fn get_pod_container_names(
    client: &Client,
    namespace: &str,
    pod_name: &str,
) -> Result<Vec<String>, String> {
    let value = resource_get::get_resource_value(client, "Pod", pod_name, Some(namespace))
        .await
        .map_err(|e| e.to_string())?;
    let spec = value
        .get("spec")
        .and_then(|s| s.as_object())
        .ok_or_else(|| "Pod spec not found".to_string())?;
    let mut names = Vec::new();
    if let Some(init) = spec.get("initContainers").and_then(|c| c.as_array()) {
        for c in init {
            if let Some(name) = c.get("name").and_then(|n| n.as_str()) {
                names.push(name.to_string());
            }
        }
    }
    if let Some(containers) = spec.get("containers").and_then(|c| c.as_array()) {
        for c in containers {
            if let Some(name) = c.get("name").and_then(|n| n.as_str()) {
                names.push(name.to_string());
            }
        }
    }
    Ok(names)
}

/// 拉取 Pod 指定容器的日志（一次性，非流式）。
/// namespace 必填；container 为空时使用 Pod 第一个容器。
pub async fn get_pod_logs(
    client: &Client,
    namespace: &str,
    pod_name: &str,
    container: Option<&str>,
    tail_lines: Option<i64>,
    since_seconds: Option<i64>,
    timestamps: bool,
    previous: bool,
) -> Result<String, String> {
    let api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let lp = LogParams {
        container: container.map(String::from),
        tail_lines,
        since_seconds,
        timestamps,
        previous,
        follow: false,
        ..Default::default()
    };

    api.logs(pod_name, &lp).await.map_err(|e| e.to_string())
}

/// 按 stream_id 存储活跃的 Pod 日志流 AbortHandle
pub struct PodLogStreamStore {
    handles: Arc<RwLock<HashMap<String, tokio::task::AbortHandle>>>,
}

impl PodLogStreamStore {
    pub fn new() -> Self {
        Self {
            handles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn stop(&self, stream_id: &str) {
        let mut guard = self.handles.write().await;
        if let Some(h) = guard.remove(stream_id) {
            h.abort();
        }
    }

    pub async fn insert(&self, stream_id: String, handle: tokio::task::AbortHandle) {
        let mut guard = self.handles.write().await;
        if let Some(old) = guard.insert(stream_id, handle) {
            old.abort();
        }
    }
}

impl Default for PodLogStreamStore {
    fn default() -> Self {
        Self::new()
    }
}

/// 启动 Pod 日志流式拉取，通过 Tauri 事件推送 chunk。
/// 调用方需在返回后 await stream_store.insert，以注册 AbortHandle。
#[allow(clippy::too_many_arguments)]
pub async fn run_pod_log_stream(
    app: AppHandle,
    stream_id: String,
    client: Client,
    namespace: String,
    pod_name: String,
    container: Option<String>,
    tail_lines: Option<i64>,
    since_seconds: Option<i64>,
    timestamps: bool,
    previous: bool,
) {
    let api: Api<Pod> = Api::namespaced(client, &namespace);
    let lp = LogParams {
        container,
        tail_lines,
        since_seconds,
        timestamps,
        previous,
        follow: true,
        ..Default::default()
    };

    match api.log_stream(&pod_name, &lp).await {
        Ok(reader) => {
            let mut lines = reader.lines();
            loop {
                match lines.try_next().await {
                    Ok(Some(line)) => {
                        let payload = serde_json::json!({
                            "stream_id": stream_id,
                            "chunk": line + "\n"
                        });
                        if app.emit(POD_LOG_CHUNK_EVENT, payload).is_err() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        let _ = app.emit(
                            POD_LOG_STREAM_END_EVENT,
                            serde_json::json!({
                                "stream_id": stream_id,
                                "error": e.to_string()
                            }),
                        );
                        break;
                    }
                }
            }
        }
        Err(e) => {
            let _ = app.emit(
                POD_LOG_STREAM_END_EVENT,
                serde_json::json!({
                    "stream_id": stream_id,
                    "error": e.to_string()
                }),
            );
        }
    }
    let _ = app.emit(
        POD_LOG_STREAM_END_EVENT,
        serde_json::json!({ "stream_id": stream_id }),
    );
}
