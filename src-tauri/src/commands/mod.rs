//! Tauri 命令：配置、环境、K8s 资源、凭证；统一注册便于扩展。

pub mod config_commands;
pub mod credential_commands;
pub mod env_commands;
pub mod kube_commands;
pub mod log_commands;
pub mod terminal_commands;

use crate::config::AppSettingsConfig;
use crate::credentials::new_from_settings;
use crate::kube::{KubeClientStore, PodExecStore, PodLogStreamStore, WatchStore};
use terminal_commands::HostShellStore;
use credential_commands::StrongholdAutoLockController;
use std::sync::Arc;
use tauri::Manager;

pub fn setup_app_state(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(KubeClientStore::new(app.handle().clone()));
    app.manage(Arc::new(WatchStore::new()));
    app.manage(Arc::new(PodLogStreamStore::new()));
    app.manage(Arc::new(PodExecStore::new()));
    app.manage(Arc::new(HostShellStore::new()));

    // 初始化凭证管理器（从应用设置加载 Stronghold 路径）
    let settings = crate::config::app_settings_config_path()
        .and_then(|p| AppSettingsConfig::load(&p).ok())
        .unwrap_or_default();
    app.manage(new_from_settings(&settings));
    app.manage(StrongholdAutoLockController::default());

    Ok(())
}
