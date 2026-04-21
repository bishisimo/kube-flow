//! 环境管理 Tauri 命令：CRUD、新建连接（本地/SSH）、切换 context。
//! 前端传 camelCase，通过 serde(rename_all = "camelCase") 映射到后端 snake_case。

use crate::commands::kube_command_context::{err_str, CommandResult};
use crate::env::types::EnvironmentContext;
use crate::env::{EnvService, Environment, KubeContextInfo, SshTunnel};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnvSetCurrentContextArgs {
    pub env_id: String,
    pub context_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnvCreateLocalArgs {
    pub display_name: String,
    pub kubeconfig_path: String,
    pub selected_contexts: Vec<KubeContextInfo>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnvCreateSshArgs {
    pub display_name: String,
    pub ssh_tunnel_id: String,
    pub contexts: Vec<EnvironmentContext>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub ssh_idle_protection: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnvCreateSshWithHostArgs {
    pub display_name: String,
    pub ssh_host: String,
    pub remote_kubeconfig_path: String,
    #[serde(default)]
    pub local_port: Option<u16>,
    pub contexts: Vec<EnvironmentContext>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub ssh_idle_protection: Option<bool>,
}

#[tauri::command]
pub fn env_list() -> CommandResult<Vec<Environment>> {
    EnvService::list().map_err(err_str)
}

#[tauri::command]
pub fn env_add(env: Environment) -> CommandResult<()> {
    EnvService::add(env).map_err(err_str)
}

#[tauri::command]
pub fn env_update(env: Environment) -> CommandResult<()> {
    EnvService::update(env).map_err(err_str)
}

#[tauri::command]
pub fn env_delete(id: String) -> CommandResult<()> {
    EnvService::delete(&id).map_err(err_str)
}

#[tauri::command]
pub fn env_touch(id: String) -> CommandResult<()> {
    EnvService::touch(&id).map_err(err_str)
}

#[tauri::command]
pub fn env_set_current_context(args: EnvSetCurrentContextArgs) -> CommandResult<()> {
    EnvService::set_current_context(&args.env_id, &args.context_name).map_err(err_str)
}

#[tauri::command]
pub fn env_list_contexts_from_kubeconfig(kubeconfig_path: String) -> CommandResult<Vec<KubeContextInfo>> {
    EnvService::list_contexts_from_kubeconfig(&kubeconfig_path).map_err(err_str)
}

#[tauri::command]
pub fn env_create_local(args: EnvCreateLocalArgs) -> CommandResult<Environment> {
    EnvService::create_local(
        args.display_name,
        args.kubeconfig_path,
        args.selected_contexts,
        args.tags,
    )
    .map_err(err_str)
}

#[tauri::command]
pub fn env_list_ssh_tunnels() -> CommandResult<Vec<SshTunnel>> {
    EnvService::list_ssh_tunnels().map_err(err_str)
}

#[tauri::command]
pub fn env_list_ssh_config_hosts() -> CommandResult<Vec<String>> {
    Ok(crate::config::ssh_config_list_hosts())
}

#[tauri::command]
pub fn env_create_ssh(args: EnvCreateSshArgs) -> CommandResult<Environment> {
    EnvService::create_ssh(
        args.display_name,
        args.ssh_tunnel_id,
        args.contexts,
        args.tags,
        args.ssh_idle_protection,
    )
        .map_err(err_str)
}

#[tauri::command]
pub fn env_create_ssh_with_host(args: EnvCreateSshWithHostArgs) -> CommandResult<Environment> {
    EnvService::create_ssh_with_host(
        args.display_name,
        args.ssh_host,
        args.remote_kubeconfig_path,
        args.local_port,
        args.contexts,
        args.tags,
        args.ssh_idle_protection,
    )
    .map_err(err_str)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnvEnsureSshTunnelForHostArgs {
    pub ssh_host: String,
    pub remote_kubeconfig_path: String,
    #[serde(default)]
    pub local_port: Option<u16>,
}

#[tauri::command]
pub fn env_ensure_ssh_tunnel_for_host(args: EnvEnsureSshTunnelForHostArgs) -> CommandResult<String> {
    EnvService::ensure_ssh_tunnel_for_host(
        args.ssh_host,
        args.remote_kubeconfig_path,
        args.local_port,
    )
    .map_err(err_str)
}
