//! 编排中心数据（Manifest、导入批次、应用包、组件元数据），持久化到 app data 目录 JSON 文件。

use crate::config::{ensure_app_data_dir, paths::app_data_dir, ConfigError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorDataPayload {
    #[serde(default = "empty_array")]
    pub manifests: Value,
    #[serde(default = "empty_array")]
    pub import_batches: Value,
    #[serde(default = "empty_array")]
    pub packages: Value,
    #[serde(default = "empty_array")]
    pub components: Value,
}

fn empty_array() -> Value {
    Value::Array(vec![])
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct OrchestratorDataFile {
    #[serde(default)]
    version: u32,
    #[serde(flatten)]
    payload: OrchestratorDataPayload,
}

const FILE_VERSION: u32 = 1;

fn config_path() -> Result<std::path::PathBuf, ConfigError> {
    ensure_app_data_dir().ok_or_else(|| {
        ConfigError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "app data dir not available",
        ))
    })?;
    app_data_dir()
        .map(|p| p.join("orchestrator-data.json"))
        .ok_or_else(|| {
            ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "app data dir not available",
            ))
        })
}

fn load_file() -> Result<OrchestratorDataFile, ConfigError> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(OrchestratorDataFile::default());
    }
    let content = std::fs::read_to_string(&path).map_err(ConfigError::Io)?;
    serde_json::from_str(&content).map_err(ConfigError::Json)
}

fn save_file(file: &OrchestratorDataFile) -> Result<(), ConfigError> {
    let path = config_path()?;
    let content = serde_json::to_string_pretty(file).map_err(ConfigError::Json)?;
    std::fs::write(path, content).map_err(ConfigError::Io)
}

pub fn load() -> Result<OrchestratorDataPayload, ConfigError> {
    Ok(load_file()?.payload)
}

pub fn save(payload: OrchestratorDataPayload) -> Result<(), ConfigError> {
    let file = OrchestratorDataFile {
        version: FILE_VERSION,
        payload,
    };
    save_file(&file)
}
