//! Kube 相关 Tauri 命令的公共上下文：按 env_id 解析 Environment 与 kube::Client。
//!
//! 同时提供横切关注点工具：
//! - `load_app_settings()` — 统一的应用设置加载，避免各命令重复读取配置文件
//! - `err_str()`          — 将任意 Display 错误转为 String，命令层边界统一用法

use crate::config::{app_settings_config_path, AppSettingsConfig};
use crate::env::{EnvService, Environment};
use crate::kube::KubeClientStore;

// ── 全局类型别名 ───────────────────────────────────────────────────────────────

/// 所有 Tauri 命令的统一返回类型别名：`Result<T, String>`。
pub type CommandResult<T> = Result<T, String>;

/// 将任意 `Display` 错误转换为 `String`，用于 `.map_err(err_str)`。
#[inline]
pub fn err_str<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ── 环境 / Client 解析 ─────────────────────────────────────────────────────

pub fn environment_by_id(env_id: &str) -> CommandResult<Environment> {
    EnvService::list()
        .map_err(err_str)?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| "environment not found".to_string())
}

pub async fn kube_client_for_env(
    store: &KubeClientStore,
    env: &Environment,
) -> CommandResult<kube::Client> {
    store.get_or_build(env).await.map_err(err_str)
}

/// 解析环境并构建或复用 Client，供各 `kube_*` 命令复用。
pub async fn kube_client_for_env_id(
    store: &KubeClientStore,
    env_id: &str,
) -> CommandResult<(Environment, kube::Client)> {
    let env = environment_by_id(env_id)?;
    let client = kube_client_for_env(store, &env).await?;
    Ok((env, client))
}

// ── 横切关注点工具 ─────────────────────────────────────────────────────────

/// 加载应用设置，统一错误转换。
/// 替代各命令函数中散落的 `app_settings_config_path().ok_or_else(...)?` 样板代码。
pub fn load_app_settings() -> CommandResult<AppSettingsConfig> {
    let path = app_settings_config_path()
        .ok_or_else(|| "app data dir not available".to_string())?;
    AppSettingsConfig::load(&path).map_err(err_str)
}
