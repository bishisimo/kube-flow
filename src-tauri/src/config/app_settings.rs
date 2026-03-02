//! 应用设置：日志级别、SSH 隧道模式、凭证存储等，持久化到 app-settings.toml。

use crate::config::ConfigError;
use crate::credentials::types::CredentialStoreKind;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 日志级别：Off 表示不写日志，其余按优先级 Error < Warn < Info < Debug。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    #[default]
    Off,
    Error,
    Warn,
    Info,
    Debug,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Off => "off",
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "error" => LogLevel::Error,
            "warn" => LogLevel::Warn,
            "info" => LogLevel::Info,
            "debug" => LogLevel::Debug,
            _ => LogLevel::Off,
        }
    }

    pub fn allows(&self, level: LogLevel) -> bool {
        if *self == LogLevel::Off {
            return false;
        }
        let order = |l: LogLevel| match l {
            LogLevel::Off => 0,
            LogLevel::Error => 1,
            LogLevel::Warn => 2,
            LogLevel::Info => 3,
            LogLevel::Debug => 4,
        };
        order(level) <= order(*self)
    }
}

/// 凭证存储安全配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// 凭证持久化后端：os_keychain / stronghold（默认）。
    #[serde(default)]
    pub credential_store: CredentialStoreKind,
    /// Stronghold 快照文件路径；空字符串表示使用默认路径 `{app_data_dir}/credentials.hold`。
    #[serde(default)]
    pub stronghold_snapshot_path: String,
    /// Stronghold 自动锁定时间（分钟）；0 表示不自动锁定。
    #[serde(default = "default_auto_lock_minutes")]
    pub auto_lock_minutes: u32,
}

fn default_auto_lock_minutes() -> u32 {
    0
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            credential_store: CredentialStoreKind::Stronghold,
            stronghold_snapshot_path: String::new(),
            auto_lock_minutes: 0,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AppSettingsConfig {
    #[serde(default)]
    pub log_level: String,
    #[serde(default)]
    pub log_display_order: String,
    #[serde(default)]
    pub log_display_format: String,
    /// 默认 SSH 隧道映射方式：ssh 或 builtin，供新建隧道或未显式配置的隧道使用。
    #[serde(default)]
    pub default_ssh_tunnel_mode: String,
    /// 凭证存储与安全设置。
    #[serde(default)]
    pub security: SecurityConfig,
}

impl AppSettingsConfig {
    pub fn log_level(&self) -> LogLevel {
        LogLevel::from_str(&self.log_level)
    }

    pub fn set_log_level(&mut self, level: LogLevel) {
        self.log_level = level.as_str().to_string();
    }

    pub fn log_display_order(&self) -> LogDisplayOrder {
        LogDisplayOrder::from_str(&self.log_display_order)
    }

    pub fn set_log_display_order(&mut self, order: LogDisplayOrder) {
        self.log_display_order = order.as_str().to_string();
    }

    pub fn log_display_format(&self) -> LogDisplayFormat {
        LogDisplayFormat::from_str(&self.log_display_format)
    }

    pub fn set_log_display_format(&mut self, format: LogDisplayFormat) {
        self.log_display_format = format.as_str().to_string();
    }

    /// 默认 SSH 隧道映射方式，解析失败时返回 "ssh"。
    pub fn default_ssh_tunnel_mode(&self) -> String {
        let s = self.default_ssh_tunnel_mode.trim().to_lowercase();
        if s == "builtin" {
            "builtin".to_string()
        } else {
            "ssh".to_string()
        }
    }

    pub fn set_default_ssh_tunnel_mode(&mut self, mode: &str) {
        let s = mode.trim().to_lowercase();
        self.default_ssh_tunnel_mode = if s == "builtin" {
            "builtin".to_string()
        } else {
            "ssh".to_string()
        };
    }

}

/// 日志显示顺序：asc=正序（旧→新），desc=倒序（新→旧）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogDisplayOrder {
    #[default]
    Asc,
    Desc,
}

impl LogDisplayOrder {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogDisplayOrder::Asc => "asc",
            LogDisplayOrder::Desc => "desc",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "desc" => LogDisplayOrder::Desc,
            _ => LogDisplayOrder::Asc,
        }
    }
}

/// 日志显示格式：json=原始 JSON 行，text=可读文本。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogDisplayFormat {
    #[default]
    Json,
    Text,
}

impl LogDisplayFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogDisplayFormat::Json => "json",
            LogDisplayFormat::Text => "text",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "text" => LogDisplayFormat::Text,
            _ => LogDisplayFormat::Json,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AppSettingsFile {
    #[serde(default)]
    log_level: String,
    #[serde(default)]
    log_display_order: String,
    #[serde(default)]
    log_display_format: String,
    #[serde(default)]
    default_ssh_tunnel_mode: String,
    #[serde(default)]
    security: SecurityConfig,
}

impl AppSettingsConfig {
    pub fn load(path: &Path) -> Result<AppSettingsConfig, ConfigError> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(AppSettingsConfig::default());
            }
            Err(e) => return Err(ConfigError::Io(e)),
        };
        let file: AppSettingsFile = toml::from_str(&content).map_err(ConfigError::Toml)?;
        Ok(AppSettingsConfig {
            log_level: if file.log_level.is_empty() {
                "off".to_string()
            } else {
                file.log_level
            },
            log_display_order: if file.log_display_order.is_empty() {
                "asc".to_string()
            } else {
                file.log_display_order
            },
            log_display_format: if file.log_display_format.is_empty() {
                "json".to_string()
            } else {
                file.log_display_format
            },
            default_ssh_tunnel_mode: if file.default_ssh_tunnel_mode.is_empty() {
                "ssh".to_string()
            } else {
                file.default_ssh_tunnel_mode
            },
            security: file.security,
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ConfigError::Io)?;
        }
        let file = AppSettingsFile {
            log_level: self.log_level.clone(),
            log_display_order: self.log_display_order.clone(),
            log_display_format: self.log_display_format.clone(),
            default_ssh_tunnel_mode: self.default_ssh_tunnel_mode.clone(),
            security: self.security.clone(),
        };
        let content = toml::to_string_pretty(&file).map_err(ConfigError::TomlSer)?;
        std::fs::write(path, content).map_err(ConfigError::Io)
    }
}
