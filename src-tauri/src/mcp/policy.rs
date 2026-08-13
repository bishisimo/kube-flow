//! MCP 策略文件：`mcp-policy.toml` 的加载与保存。

use super::capabilities::{Capability, Preset};
use crate::config::{ensure_app_data_dir, mcp_policy_config_path, ConfigError};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 全局 MCP 开关与 Gateway 配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpGlobalConfig {
    /// 总开关；默认关闭。
    #[serde(default)]
    pub enabled: bool,
    /// App 是否监听 Gateway。
    #[serde(default = "default_true")]
    pub listen: bool,
    /// SessionGrant 有效分钟数。
    #[serde(default = "default_session_grant_minutes")]
    pub session_grant_minutes: u32,
    /// 相对 app data 的 token 文件名。
    #[serde(default = "default_token_path")]
    pub token_path: String,
    /// 相对 app data 的审计日志文件名。
    #[serde(default = "default_audit_log")]
    pub audit_log: String,
}

fn default_true() -> bool {
    true
}

fn default_session_grant_minutes() -> u32 {
    15
}

fn default_token_path() -> String {
    "mcp.token".into()
}

fn default_audit_log() -> String {
    "mcp-audit.jsonl".into()
}

impl Default for McpGlobalConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            listen: true,
            session_grant_minutes: default_session_grant_minutes(),
            token_path: default_token_path(),
            audit_log: default_audit_log(),
        }
    }
}

/// 单个环境的 MCP 绑定。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpEnvBinding {
    pub env_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub preset: Preset,
    /// 相对预设额外授予的能力（字符串形式：`env.list` 等）。
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// 从预设中扣除的能力。
    #[serde(default)]
    pub deny_capabilities: Vec<String>,
    /// 允许的 namespace；空 = 全部。支持尾部 `*` 前缀匹配。
    #[serde(default)]
    pub namespaces: Vec<String>,
    /// 允许的 kind 白名单；空 = 全部。
    #[serde(default)]
    pub kinds_allow: Vec<String>,
    /// 写操作是否要求确认（破坏类始终强制）。
    #[serde(default = "default_true")]
    pub require_approval: bool,
    /// 是否允许读写 Secret（默认 false：get 脱敏/拒绝 apply）。
    #[serde(default)]
    pub allow_secrets: bool,
}

impl McpEnvBinding {
    pub fn parsed_extra_capabilities(&self) -> Vec<Capability> {
        self.capabilities
            .iter()
            .filter_map(|s| Capability::parse(s))
            .collect()
    }

    pub fn parsed_deny_capabilities(&self) -> Vec<Capability> {
        self.deny_capabilities
            .iter()
            .filter_map(|s| Capability::parse(s))
            .collect()
    }
}

/// 完整策略文档。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpPolicy {
    #[serde(default)]
    pub mcp: McpGlobalConfig,
    #[serde(default)]
    pub bindings: Vec<McpEnvBinding>,
}

impl McpPolicy {
    pub fn load_default() -> Result<Self, ConfigError> {
        let path = mcp_policy_config_path().ok_or_else(|| {
            ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "app data dir not found",
            ))
        })?;
        Self::load(&path)
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let policy: Self = toml::from_str(&content).map_err(ConfigError::Toml)?;
                Ok(policy)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(ConfigError::Io(e)),
        }
    }

    pub fn save_default(&self) -> Result<(), ConfigError> {
        ensure_app_data_dir().ok_or_else(|| {
            ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "could not create app data dir",
            ))
        })?;
        let path = mcp_policy_config_path().ok_or_else(|| {
            ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "app data dir not found",
            ))
        })?;
        self.save(&path)
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ConfigError::Io)?;
        }
        let content = toml::to_string_pretty(self).map_err(ConfigError::TomlSer)?;
        std::fs::write(path, content).map_err(ConfigError::Io)
    }

    pub fn binding_for(&self, env_id: &str) -> Option<&McpEnvBinding> {
        self.bindings
            .iter()
            .find(|b| b.env_id == env_id && b.enabled)
    }
}
