//! K8s Tauri 命令，按职责分为四个子模块：
//! - `list_commands`   — 所有 kube_list_* 资源列表命令
//! - `resource_commands` — describe / get / delete / apply / patch 操作
//! - `stream_commands` — Pod 日志流与 exec 交互
//! - `watch_commands`  — Watch、别名缓存、Client 生命周期

pub mod list_commands;
pub mod resource_commands;
pub mod stream_commands;
pub mod watch_commands;

// 重新导出所有命令，保持外部调用路径兼容
pub use list_commands::*;
pub use resource_commands::*;
pub use stream_commands::*;
pub use watch_commands::*;

use crate::config::LogLevel;
use crate::debug_log;

/// 统一列表命令的日志记录：成功时记录 Info，失败时记录 Error，并转换错误为 String。
pub(crate) async fn with_list_log<T, E>(
    resource: &str,
    env_id: &str,
    future: impl std::future::Future<Output = Result<Vec<T>, E>>,
) -> Result<Vec<T>, String>
where
    E: std::fmt::Display,
{
    match future.await {
        Ok(items) => {
            debug_log::log_list_ok(resource, Some(env_id), items.len() as u32, LogLevel::Info);
            Ok(items)
        }
        Err(e) => {
            debug_log::log_list_err(resource, Some(env_id), &e.to_string(), LogLevel::Error);
            Err(e.to_string())
        }
    }
}
