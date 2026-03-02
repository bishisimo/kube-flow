//! 环境 = 连接（本地或 SSH），每个连接下可有多个 context。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentSource {
    LocalKubeconfig,
    SshTunnel,
}

/// 单个 context 信息（隶属于一个连接）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentContext {
    pub context_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_name: Option<String>,
}

/// 环境 = 一个连接（本地 kubeconfig 或 SSH 隧道），包含多个 context。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub id: String,
    pub source: EnvironmentSource,
    /// 连接级别的显示名称（如「本地 Minikube」「生产跳板机」）。
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
    /// 该连接下的所有 context。
    #[serde(default)]
    pub contexts: Vec<EnvironmentContext>,
    /// 当前选中的 context 名；None 时取 contexts 第一个。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
}

impl Environment {
    /// 返回当前要使用的 context 名；无则取第一个。
    pub fn effective_context(&self) -> Option<&str> {
        if let Some(ref name) = self.current_context {
            if self.contexts.iter().any(|c| c.context_name == *name) {
                return Some(name);
            }
        }
        self.contexts.first().map(|c| c.context_name.as_str())
    }

    /// 当前 context 的默认 namespace。
    pub fn default_namespace(&self) -> Option<&str> {
        self.effective_context()
            .and_then(|name| self.contexts.iter().find(|c| c.context_name == name))
            .and_then(|c| c.default_namespace.as_deref())
    }
}

/// 隧道映射方式：ssh 使用系统 ssh -L 子进程，builtin 使用 libssh2 内置转发。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TunnelMappingMode {
    /// 系统 ssh -L 子进程，兼容性最好。
    #[default]
    Ssh,
    /// libssh2 内置 direct-tcpip，无子进程。
    Builtin,
}

/// SSH 隧道配置：对应 [[ssh_tunnels]] 一条。映射方式由设置中的「SSH 隧道模式」统一配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshTunnel {
    pub id: String,
    pub name: String,
    pub ssh_host: String,
    pub remote_kubeconfig_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_port: Option<u16>,
    /// 认证方式偏好；Auto 表示依次尝试 agent → publickey → password。
    #[serde(default)]
    pub auth_method: crate::credentials::AuthMethod,
    /// 持久化后端中是否已保存凭证（UI 展示用，由运行时维护，不作为认证依据）。
    #[serde(default)]
    pub has_saved_credential: bool,
}
