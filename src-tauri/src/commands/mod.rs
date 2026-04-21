//! Tauri 命令：配置、环境、K8s 资源、凭证；统一注册便于扩展。
//!
//! K8s 命令已按职责拆分到 `kube/` 子目录：
//! - `kube/list_commands`     — kube_list_* 资源列表
//! - `kube/resource_commands` — describe/get/delete/apply/patch
//! - `kube/stream_commands`   — Pod 日志流与 exec
//! - `kube/watch_commands`    — Watch、别名缓存、Client 生命周期

pub mod config_commands;
pub mod credential_commands;
pub mod env_commands;
pub mod kube_command_context;
pub mod kube;
pub mod log_commands;
pub mod terminal_commands;

/// 向后兼容：保留 kube_commands 路径，所有符号来自 kube 子模块。
pub mod kube_commands {
    pub use super::kube::*;
}

use crate::config::AppSettingsConfig;
use crate::credentials::new_from_settings;
use crate::kube::{KubeClientStore, PodExecStore, PodLogStreamStore, ResourceAliasCacheStore, WatchStore};
use terminal_commands::HostShellStore;
use credential_commands::StrongholdAutoLockController;
use std::sync::Arc;
use tauri::Manager;

pub fn setup_app_state(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(KubeClientStore::new(app.handle().clone()));
    app.manage(Arc::new(WatchStore::new()));
    app.manage(Arc::new(ResourceAliasCacheStore::new()));
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
