//! 编排中心数据读写 Tauri 命令。

use crate::commands::kube_command_context::{err_str, CommandResult};
use crate::config::orchestrator_data::{OrchestratorDataPayload, load, save};

#[tauri::command]
pub fn orchestrator_data_load() -> CommandResult<OrchestratorDataPayload> {
    load().map_err(err_str)
}

#[tauri::command]
pub fn orchestrator_data_save(payload: OrchestratorDataPayload) -> CommandResult<()> {
    save(payload).map_err(err_str)
}
