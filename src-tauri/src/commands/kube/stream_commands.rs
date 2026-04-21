//! Pod 日志流与交互式 exec 命令。

use super::super::kube_command_context::{self, CommandResult};
use crate::kube::{
    get_pod_logs, run_pod_exec, run_pod_log_stream, KubeClientStore, PodExecStore, PodLogStreamStore,
};
use std::sync::Arc;
use tauri::{AppHandle, State};
use uuid::Uuid;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn kube_pod_log_stream_start(
    app: AppHandle,
    store: State<'_, KubeClientStore>,
    stream_store: State<'_, Arc<PodLogStreamStore>>,
    env_id: String,
    namespace: String,
    pod_name: String,
    container: Option<String>,
    tail_lines: Option<i64>,
    since_seconds: Option<i64>,
    timestamps: Option<bool>,
    previous: Option<bool>,
) -> CommandResult<String> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let stream_id = Uuid::new_v4().to_string();
    let stream_id_clone = stream_id.clone();
    let task = tokio::spawn(async move {
        run_pod_log_stream(
            app,
            stream_id_clone,
            client,
            namespace,
            pod_name,
            container,
            tail_lines,
            since_seconds,
            timestamps.unwrap_or(false),
            previous.unwrap_or(false),
        )
        .await
    });
    stream_store.insert(stream_id.clone(), task.abort_handle()).await;
    Ok(stream_id)
}

#[tauri::command]
pub async fn kube_pod_log_stream_stop(
    stream_store: State<'_, Arc<PodLogStreamStore>>,
    stream_id: String,
) -> CommandResult<()> {
    stream_store.stop(&stream_id).await;
    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn kube_pod_logs(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: String,
    pod_name: String,
    container: Option<String>,
    tail_lines: Option<i64>,
    since_seconds: Option<i64>,
    timestamps: Option<bool>,
    previous: Option<bool>,
) -> CommandResult<String> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    get_pod_logs(
        &client,
        &namespace,
        &pod_name,
        container.as_deref(),
        tail_lines,
        since_seconds,
        timestamps.unwrap_or(false),
        previous.unwrap_or(false),
    )
    .await
}

#[tauri::command]
pub async fn kube_pod_exec_start(
    app: AppHandle,
    store: State<'_, KubeClientStore>,
    exec_store: State<'_, Arc<PodExecStore>>,
    env_id: String,
    namespace: String,
    pod_name: String,
    container: Option<String>,
) -> CommandResult<String> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let stream_id = Uuid::new_v4().to_string();
    let stream_id_clone = stream_id.clone();
    let exec_store_clone = exec_store.inner().clone();
    tokio::spawn(async move {
        run_pod_exec(app, stream_id_clone, client, namespace, pod_name, container, exec_store_clone).await
    });
    Ok(stream_id)
}

#[tauri::command]
pub async fn kube_pod_exec_stdin(
    exec_store: State<'_, Arc<PodExecStore>>,
    stream_id: String,
    data: Vec<u8>,
) -> CommandResult<()> {
    exec_store.send_stdin(&stream_id, data).await
}

#[tauri::command]
pub async fn kube_pod_exec_resize(
    exec_store: State<'_, Arc<PodExecStore>>,
    stream_id: String,
    cols: u16,
    rows: u16,
) -> CommandResult<()> {
    exec_store.send_resize(&stream_id, cols, rows).await
}

#[tauri::command]
pub async fn kube_pod_exec_stop(
    exec_store: State<'_, Arc<PodExecStore>>,
    stream_id: String,
) -> CommandResult<()> {
    exec_store.stop(&stream_id).await;
    Ok(())
}
