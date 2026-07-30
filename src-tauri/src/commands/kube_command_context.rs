//! Kube 相关 Tauri 命令的公共上下文：按 env_id 解析 Environment 与 kube::Client。
//!
//! 同时提供横切关注点工具：
//! - `load_app_settings()` — 统一的应用设置加载，避免各命令重复读取配置文件
//! - `err_str()`          — 将任意 Display 错误转为 String，命令层边界统一用法
//! - `is_kube_transport_error()` / `retry_after_ssh_transport_error()` — SSH 隧道断开后重建并重试

use crate::config::{app_settings_config_path, AppSettingsConfig, LogLevel};
use crate::debug_log;
use crate::env::{EnvService, Environment, EnvironmentSource};
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

/// 判断错误是否像隧道/传输层故障（重建 SSH 隧道后可能恢复）。
pub fn is_kube_transport_error(msg: &str) -> bool {
    let s = msg.to_lowercase();
    s.contains("connection refused")
        || s.contains("connection reset")
        || s.contains("broken pipe")
        || s.contains("timed out")
        || s.contains("timeout")
        || s.contains("error trying to connect")
        || s.contains("tcp connect")
        || s.contains("client error")
        || s.contains("error sending request")
        || s.contains("connection closed")
        || s.contains("unexpected eof")
        || s.contains("failed to perform")
        || s.contains("error hyper")
        || s.contains("connect error")
}

/// SSH 隧道环境下，若首次结果为传输层错误，则拆除隧道与 Client 缓存后由调用方再执行一次。
/// 返回 `Some(err)` 表示不应重试，直接返回该错误；`None` 表示已 invalidate，应重试。
pub async fn prepare_ssh_transport_retry(
    store: &KubeClientStore,
    env: &Environment,
    err: &str,
) -> Option<String> {
    if !matches!(env.source, EnvironmentSource::SshTunnel) || !is_kube_transport_error(err) {
        return Some(err.to_string());
    }
    debug_log::log_tunnel(
        Some(&env.id),
        "api_retry",
        Some(&format!("传输错误，重建隧道后重试: {}", err)),
        LogLevel::Warn,
    );
    store.remove(&env.id).await;
    None
}

// ── 横切关注点工具 ─────────────────────────────────────────────────────────

/// 加载应用设置，统一错误转换。
/// 替代各命令函数中散落的 `app_settings_config_path().ok_or_else(...)?` 样板代码。
pub fn load_app_settings() -> CommandResult<AppSettingsConfig> {
    let path =
        app_settings_config_path().ok_or_else(|| "app data dir not available".to_string())?;
    AppSettingsConfig::load(&path).map_err(err_str)
}
