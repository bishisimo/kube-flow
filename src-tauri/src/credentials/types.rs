//! 凭证相关基础类型：键、认证方式、存储后端枚举。

use serde::{Deserialize, Serialize};

/// 凭证键：以 tunnel_id 唯一标识一条 SSH 隧道的认证凭证。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CredentialKey {
    pub tunnel_id: String,
}

impl CredentialKey {
    pub fn new(tunnel_id: impl Into<String>) -> Self {
        Self { tunnel_id: tunnel_id.into() }
    }

    /// OS 钥匙串中的 username 字段：`ssh/{tunnel_id}`。
    pub fn keyring_username(&self) -> String {
        format!("ssh/{}", self.tunnel_id)
    }

    /// Stronghold 加密存储中的键：`ssh/{tunnel_id}`。
    pub fn stronghold_key(&self) -> String {
        format!("ssh/{}", self.tunnel_id)
    }
}

/// SSH 认证方式，存于隧道配置中，指导连接时的认证策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    /// 自动探测：依次尝试 agent → publickey → password。
    #[default]
    Auto,
    /// 密码认证。
    Password,
    /// 公钥认证（无需密码）。
    PublicKey,
    /// 键盘交互式认证（PAM / 双因素）。
    KeyboardInteractive,
}

/// 凭证持久化后端类型，由应用设置决定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CredentialStoreKind {
    /// 系统钥匙串（macOS Keychain / Windows Credential Manager / libsecret）。
    OsKeychain,
    /// 本地 AES-256-GCM 加密文件，主密码经 Argon2id 派生密钥。
    #[default]
    Stronghold,
}

impl CredentialStoreKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CredentialStoreKind::OsKeychain => "os_keychain",
            CredentialStoreKind::Stronghold => "stronghold",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "os_keychain" => CredentialStoreKind::OsKeychain,
            _ => CredentialStoreKind::Stronghold,
        }
    }
}

/// 已保存凭证的摘要，供 UI 管理列表使用，不含密码本身。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialInfo {
    pub tunnel_id: String,
    pub store: String,
}
