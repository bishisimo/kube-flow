//! 环境管理 Tauri 命令：CRUD、新建连接（本地/SSH）、切换 context。
//! 前端传 camelCase，通过 serde(rename_all = "camelCase") 映射到后端 snake_case。

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
pub fn env_list() -> Result<Vec<Environment>, String> {
    EnvService::list().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn env_add(env: Environment) -> Result<(), String> {
    EnvService::add(env).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn env_update(env: Environment) -> Result<(), String> {
    EnvService::update(env).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn env_delete(id: String) -> Result<(), String> {
    EnvService::delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn env_touch(id: String) -> Result<(), String> {
    EnvService::touch(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn env_set_current_context(args: EnvSetCurrentContextArgs) -> Result<(), String> {
    EnvService::set_current_context(&args.env_id, &args.context_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn env_list_contexts_from_kubeconfig(kubeconfig_path: String) -> Result<Vec<KubeContextInfo>, String> {
    EnvService::list_contexts_from_kubeconfig(&kubeconfig_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn env_create_local(args: EnvCreateLocalArgs) -> Result<Environment, String> {
    EnvService::create_local(
        args.display_name,
        args.kubeconfig_path,
        args.selected_contexts,
        args.tags,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn env_list_ssh_tunnels() -> Result<Vec<SshTunnel>, String> {
    EnvService::list_ssh_tunnels().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn env_list_ssh_config_hosts() -> Result<Vec<String>, String> {
    Ok(crate::config::ssh_config_list_hosts())
}

#[tauri::command]
pub fn env_create_ssh(args: EnvCreateSshArgs) -> Result<Environment, String> {
    EnvService::create_ssh(
        args.display_name,
        args.ssh_tunnel_id,
        args.contexts,
        args.tags,
        args.ssh_idle_protection,
    )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn env_create_ssh_with_host(args: EnvCreateSshWithHostArgs) -> Result<Environment, String> {
    EnvService::create_ssh_with_host(
        args.display_name,
        args.ssh_host,
        args.remote_kubeconfig_path,
        args.local_port,
        args.contexts,
        args.tags,
        args.ssh_idle_protection,
    )
    .map_err(|e| e.to_string())
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
pub fn env_ensure_ssh_tunnel_for_host(args: EnvEnsureSshTunnelForHostArgs) -> Result<String, String> {
    EnvService::ensure_ssh_tunnel_for_host(
        args.ssh_host,
        args.remote_kubeconfig_path,
        args.local_port,
    )
    .map_err(|e| e.to_string())
}
