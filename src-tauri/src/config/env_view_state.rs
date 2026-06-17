//! 各环境工作台视图状态（kind / namespace / 筛选等），持久化到 app data 目录 JSON 文件。

use crate::config::{ensure_app_data_dir, paths::app_data_dir, ConfigError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedAliasTarget {
    pub group: String,
    pub version: String,
    pub api_version: String,
    pub kind: String,
    pub plural: String,
    pub namespaced: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_names: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub singular: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvViewState {
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub name_filter: String,
    #[serde(default = "default_node_filter")]
    pub node_filter: String,
    #[serde(default)]
    pub pod_ip_filter: String,
    #[serde(default)]
    pub label_selector: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_target: Option<ResolvedAliasTarget>,
}

fn default_kind() -> String {
    "namespaces".into()
}

fn default_node_filter() -> String {
    "all".into()
}

impl Default for EnvViewState {
    fn default() -> Self {
        Self {
            namespace: None,
            kind: default_kind(),
            name_filter: String::new(),
            node_filter: default_node_filter(),
            pod_ip_filter: String::new(),
            label_selector: String::new(),
            custom_target: None,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct EnvViewStateFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    states: HashMap<String, EnvViewState>,
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
        .map(|p| p.join("env-view-states.json"))
        .ok_or_else(|| {
            ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "app data dir not available",
            ))
        })
}

fn load_file() -> Result<EnvViewStateFile, ConfigError> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(EnvViewStateFile::default());
    }
    let content = std::fs::read_to_string(&path).map_err(ConfigError::Io)?;
    serde_json::from_str(&content).map_err(ConfigError::Json)
}

fn save_file(file: &EnvViewStateFile) -> Result<(), ConfigError> {
    let path = config_path()?;
    let content = serde_json::to_string_pretty(file).map_err(ConfigError::Json)?;
    std::fs::write(path, content).map_err(ConfigError::Io)
}

pub fn list_all() -> Result<HashMap<String, EnvViewState>, ConfigError> {
    Ok(load_file()?.states)
}

pub fn set(env_id: &str, state: EnvViewState) -> Result<(), ConfigError> {
    let mut file = load_file()?;
    file.version = FILE_VERSION;
    file.states.insert(env_id.to_string(), state);
    save_file(&file)
}

pub fn remove(env_id: &str) -> Result<(), ConfigError> {
    let mut file = load_file()?;
    if file.states.remove(env_id).is_some() {
        save_file(&file)?;
    }
    Ok(())
}
