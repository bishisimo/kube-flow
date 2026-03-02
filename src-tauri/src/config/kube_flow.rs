//! kube-flow.toml 的读写：环境（连接 + 多 context）与 SSH 隧道配置。

use crate::config::ConfigError;
use crate::env::types::{EnvironmentContext, EnvironmentSource};
use crate::env::{Environment, SshTunnel};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 根配置结构，对应 kube-flow.toml 顶层。
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct KubeFlowConfig {
    #[serde(default)]
    pub environments: Vec<Environment>,
    #[serde(default)]
    pub ssh_tunnels: Vec<SshTunnel>,
}

/// 单个 context 在 TOML 中的表示。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentContextRow {
    pub context_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_name: Option<String>,
}

/// 环境在 TOML 中的表示（连接 + contexts 数组）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentRow {
    pub id: String,
    pub source: EnvironmentSource,
    pub display_name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub is_favorite: bool,
    #[serde(default)]
    pub sort_order: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kubeconfig_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_tunnel_id: Option<String>,
    #[serde(default)]
    pub contexts: Vec<EnvironmentContextRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
    // 兼容旧版：单 context 时存在
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_name: Option<String>,
}

impl From<Environment> for EnvironmentRow {
    fn from(e: Environment) -> Self {
        Self {
            id: e.id,
            source: e.source,
            display_name: e.display_name,
            tags: e.tags,
            is_favorite: e.is_favorite,
            sort_order: e.sort_order,
            kubeconfig_path: e.kubeconfig_path,
            ssh_tunnel_id: e.ssh_tunnel_id,
            contexts: e.contexts.iter().cloned().map(EnvironmentContextRow::from).collect(),
            current_context: e.current_context,
            last_used_at: e.last_used_at,
            context_name: None,
            default_namespace: None,
            cluster_name: None,
        }
    }
}

impl From<EnvironmentContext> for EnvironmentContextRow {
    fn from(c: EnvironmentContext) -> Self {
        Self {
            context_name: c.context_name,
            display_name: c.display_name,
            default_namespace: c.default_namespace,
            cluster_name: c.cluster_name,
        }
    }
}

impl From<EnvironmentContextRow> for EnvironmentContext {
    fn from(r: EnvironmentContextRow) -> Self {
        Self {
            context_name: r.context_name,
            display_name: r.display_name,
            default_namespace: r.default_namespace,
            cluster_name: r.cluster_name,
        }
    }
}

impl From<EnvironmentRow> for Environment {
    fn from(r: EnvironmentRow) -> Self {
        let (contexts, current_context) = if !r.contexts.is_empty() {
            (r.contexts.into_iter().map(EnvironmentContext::from).collect(), r.current_context)
        } else if let Some(cn) = r.context_name {
            let ctx = EnvironmentContext {
                context_name: cn.clone(),
                display_name: None,
                default_namespace: r.default_namespace,
                cluster_name: r.cluster_name,
            };
            (vec![ctx], Some(cn))
        } else {
            (vec![], None)
        };
        Environment {
            id: r.id,
            source: r.source,
            display_name: r.display_name,
            tags: r.tags,
            is_favorite: r.is_favorite,
            sort_order: r.sort_order,
            kubeconfig_path: r.kubeconfig_path,
            ssh_tunnel_id: r.ssh_tunnel_id,
            contexts,
            current_context,
            last_used_at: r.last_used_at,
        }
    }
}

/// SSH 隧道在 TOML 中的表示。映射方式由设置统一配置，此处不再存储。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshTunnelRow {
    pub id: String,
    pub name: String,
    pub ssh_host: String,
    pub remote_kubeconfig_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_port: Option<u16>,
    #[serde(default)]
    pub auth_method: crate::credentials::AuthMethod,
    /// 持久化后端中是否已保存凭证（UI 展示用，由运行时维护）。
    #[serde(default)]
    pub has_saved_credential: bool,
}

impl From<SshTunnel> for SshTunnelRow {
    fn from(t: SshTunnel) -> Self {
        Self {
            id: t.id,
            name: t.name,
            ssh_host: t.ssh_host,
            remote_kubeconfig_path: t.remote_kubeconfig_path,
            local_port: t.local_port,
            remote_port: t.remote_port,
            auth_method: t.auth_method,
            has_saved_credential: t.has_saved_credential,
        }
    }
}

impl From<SshTunnelRow> for SshTunnel {
    fn from(r: SshTunnelRow) -> Self {
        SshTunnel {
            id: r.id,
            name: r.name,
            ssh_host: r.ssh_host,
            remote_kubeconfig_path: r.remote_kubeconfig_path,
            local_port: r.local_port,
            remote_port: r.remote_port,
            auth_method: r.auth_method,
            has_saved_credential: r.has_saved_credential,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct KubeFlowConfigFile {
    #[serde(default)]
    pub environments: Vec<EnvironmentRow>,
    #[serde(default)]
    pub ssh_tunnels: Vec<SshTunnelRow>,
}

impl KubeFlowConfigFile {
    pub fn load(path: &Path) -> Result<KubeFlowConfig, ConfigError> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(KubeFlowConfig::default());
            }
            Err(e) => return Err(ConfigError::Io(e)),
        };
        let file: KubeFlowConfigFile = toml::from_str(&content).map_err(ConfigError::Toml)?;
        Ok(KubeFlowConfig {
            environments: file.environments.into_iter().map(Environment::from).collect(),
            ssh_tunnels: file.ssh_tunnels.into_iter().map(SshTunnel::from).collect(),
        })
    }

    pub fn save(config: &KubeFlowConfig, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ConfigError::Io)?;
        }
        let file = KubeFlowConfigFile {
            environments: config.environments.iter().cloned().map(EnvironmentRow::from).collect(),
            ssh_tunnels: config.ssh_tunnels.iter().cloned().map(SshTunnelRow::from).collect(),
        };
        let content = toml::to_string_pretty(&file).map_err(ConfigError::TomlSer)?;
        std::fs::write(path, content).map_err(ConfigError::Io)
    }
}
