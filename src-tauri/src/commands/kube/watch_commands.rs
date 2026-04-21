//! Watch、资源别名缓存、Client 生命周期管理命令。

use super::super::kube_command_context::{self, err_str, CommandResult};
use crate::kube::{
    start_watch, KubeClientStore, ResourceAliasCacheStore, ResourceAliasRefreshResult,
    ResolvedAliasTarget, WatchStore,
};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn kube_start_watch(
    app: AppHandle,
    store: State<'_, KubeClientStore>,
    watch_store: State<'_, Arc<WatchStore>>,
    env_id: String,
    kind: String,
    namespace: Option<String>,
    label_selector: Option<String>,
    watch_token: Option<String>,
) -> CommandResult<()> {
    start_watch(
        app,
        store.inner().clone(),
        Arc::clone(watch_store.inner()),
        env_id,
        kind,
        namespace,
        label_selector,
        watch_token,
    )
    .await
}

#[tauri::command]
pub async fn kube_stop_watch(
    watch_store: State<'_, Arc<WatchStore>>,
    env_id: String,
) -> CommandResult<()> {
    watch_store.stop(&env_id).await;
    Ok(())
}

/// 按当前集群重建资源别名索引（shortNames、plural、kind、singular），写入按环境缓存。
#[tauri::command]
pub async fn kube_refresh_resource_aliases(
    store: State<'_, KubeClientStore>,
    alias_store: State<'_, Arc<ResourceAliasCacheStore>>,
    env_id: String,
) -> CommandResult<ResourceAliasRefreshResult> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    alias_store.refresh(&env_id, &client).await.map_err(err_str)
}

/// 在已刷新的发现缓存中解析别名；`preferred_group` 非空时优先匹配该 API 组。
#[tauri::command]
pub async fn kube_resolve_resource_alias(
    alias_store: State<'_, Arc<ResourceAliasCacheStore>>,
    env_id: String,
    query: String,
    preferred_group: Option<String>,
) -> CommandResult<Vec<ResolvedAliasTarget>> {
    alias_store
        .resolve(&env_id, &query, preferred_group.as_deref())
        .await
}

#[tauri::command]
pub fn kube_get_tunnel_local_port(
    store: State<'_, KubeClientStore>,
    env_id: String,
) -> Option<u16> {
    store.get_tunnel_local_port(&env_id)
}

#[tauri::command]
pub async fn kube_remove_client(
    store: State<'_, KubeClientStore>,
    watch_store: State<'_, Arc<WatchStore>>,
    alias_store: State<'_, Arc<ResourceAliasCacheStore>>,
    env_id: String,
) -> CommandResult<()> {
    watch_store.stop(&env_id).await;
    alias_store.remove_env(&env_id).await;
    store.remove(&env_id).await;
    Ok(())
}
