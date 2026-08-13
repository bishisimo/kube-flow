//! 配置层：路径解析、kube-flow.toml、crd-display.toml、~/.ssh/config 的读写。

mod app_settings;
mod crd_display;
pub mod env_view_state;
mod kube_flow;
pub mod orchestrator_data;
mod paths;
mod ssh_config;

pub use app_settings::{
    AppSettingsConfig, GpuResourceRule, LogDisplayFormat, LogDisplayOrder, LogLevel,
    ResourceDeployStrategy, SecurityConfig,
};
pub use env_view_state::EnvViewState;
pub use kube_flow::{KubeFlowConfig, KubeFlowConfigFile};
pub use paths::{
    app_data_dir, app_settings_config_path, debug_log_path, debug_logs_dir, ensure_app_data_dir,
    kube_flow_config_path, mcp_audit_log_path, mcp_gateway_port_path, mcp_http_port_path,
    mcp_policy_config_path, mcp_token_path,
};

#[cfg(unix)]
pub use paths::mcp_gateway_sock_path;
pub use ssh_config::{
    default_config_path_string as ssh_config_default_path, delete_entry as ssh_config_delete_entry,
    get_host_config as ssh_config_get_host_config, list_entries as ssh_config_list_entries,
    list_hosts as ssh_config_list_hosts, resolve_proxy_command as ssh_config_resolve_proxy_command,
    upsert_entry as ssh_config_upsert_entry, SshConfigEntry, SshHostConfig,
};

use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("toml parse: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("toml serialize: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}
