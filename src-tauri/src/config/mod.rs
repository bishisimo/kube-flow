//! 配置层：路径解析、kube-flow.toml、crd-display.toml、~/.ssh/config 的读写。

mod paths;
mod kube_flow;
mod crd_display;
mod ssh_config;
mod app_settings;

pub use paths::{app_data_dir, app_settings_config_path, debug_log_path, ensure_app_data_dir, kube_flow_config_path};
pub use app_settings::{
    AppSettingsConfig, LogDisplayFormat, LogDisplayOrder, LogLevel, ResourceDeployStrategy, SecurityConfig,
};
pub use kube_flow::{KubeFlowConfig, KubeFlowConfigFile};
pub use ssh_config::{
    get_host_config as ssh_config_get_host_config,
    list_hosts as ssh_config_list_hosts,
    resolve_proxy_command as ssh_config_resolve_proxy_command,
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
}
