//! crd-display.toml 独立配置：用户指定要展示的 CR（GVK），避免全量 CR。
//! 当前未接入资源树，预留供后续 CRD 展示功能使用。
#![allow(dead_code)]

use crate::config::ConfigError;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 单条要展示的 CR，可用 group/version/kind 或 group/kind。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrdDisplayEntry {
    pub group: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub kind: String,
}

/// CRD 展示配置：仅展示列表中的 CR。
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CrdDisplayConfig {
    /// 全局要展示的 CR 列表；未配置则资源树不展示任何 CR。
    #[serde(default)]
    pub include: Vec<CrdDisplayEntry>,
    /// 可选：按环境 id 覆盖该环境下的 include。
    #[serde(default)]
    pub per_env: std::collections::HashMap<String, Vec<CrdDisplayEntry>>,
}

impl CrdDisplayConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(ConfigError::Io(e)),
        };
        toml::from_str(&content).map_err(ConfigError::Toml)
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ConfigError::Io)?;
        }
        let content = toml::to_string_pretty(self).map_err(ConfigError::TomlSer)?;
        std::fs::write(path, content).map_err(ConfigError::Io)
    }

    /// 返回某环境应展示的 CR 列表；若该环境有覆盖则用覆盖，否则用全局 include。
    pub fn entries_for_env(&self, env_id: Option<&str>) -> &[CrdDisplayEntry] {
        if let Some(id) = env_id {
            if let Some(per) = self.per_env.get(id) {
                return per.as_slice();
            }
        }
        self.include.as_slice()
    }
}
