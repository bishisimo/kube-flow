//! 本地加密文件后端（credentials.hold）。
//! 使用 Argon2id 从主密码派生 AES-256-GCM 密钥，凭证以加密 JSON 存储。
//!
//! 文件格式（JSON）：
//!   { "version": 1, "salt": "<base64>", "nonce": "<base64>", "ciphertext": "<base64>" }
//! 解密后内容：
//!   { "ssh/tunnel-id": "password", ... }

use super::err_str;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use super::types::CredentialKey;

/// Stronghold 对外可见状态，序列化给前端使用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrongholdStatus {
    /// 未初始化：快照文件不存在，需先设置主密码创建。
    Uninitialized,
    /// 已锁定：快照文件存在但尚未解锁，需输入主密码。
    Locked,
    /// 已解锁：凭证可读写。
    Unlocked,
}

/// 内部状态，持有解密后的数据与加密密钥（仅 Unlocked 时有效）。
enum StrongholdState {
    Uninitialized,
    Locked,
    Unlocked {
        data: HashMap<String, String>,
        key: [u8; 32],
        salt: Vec<u8>,
    },
}

/// 加密文件的磁盘格式。
#[derive(Serialize, Deserialize)]
struct HoldFile {
    version: u32,
    /// Argon2 salt，base64 编码。
    salt: String,
    /// AES-GCM nonce（96 bit），base64 编码。
    nonce: String,
    /// AES-GCM 密文，base64 编码。
    ciphertext: String,
}

pub struct StrongholdBackend {
    path: PathBuf,
    state: Mutex<StrongholdState>,
}

impl StrongholdBackend {
    /// 根据快照文件路径创建后端实例；若文件存在则初始为 Locked，否则为 Uninitialized。
    pub fn new(path: PathBuf) -> Self {
        let state = if path.exists() {
            StrongholdState::Locked
        } else {
            StrongholdState::Uninitialized
        };
        Self { path, state: Mutex::new(state) }
    }

    pub fn status(&self) -> StrongholdStatus {
        // 每次 status() 前重新检查文件是否存在，防止外部删除后状态不同步。
        let exists = self.path.exists();
        match &*self.state.lock().unwrap_or_else(|p| p.into_inner()) {
            StrongholdState::Uninitialized if exists => StrongholdStatus::Locked,
            StrongholdState::Uninitialized => StrongholdStatus::Uninitialized,
            StrongholdState::Locked => StrongholdStatus::Locked,
            StrongholdState::Unlocked { .. } => StrongholdStatus::Unlocked,
        }
    }

    /// 初始化：用主密码创建新的加密快照（若文件已存在则覆盖）。
    pub fn initialize(&self, master_password: &str) -> Result<(), String> {
        let mut salt = [0u8; 16];
        rand::RngCore::fill_bytes(&mut OsRng, &mut salt);

        let key = derive_key(master_password, &salt)?;
        let empty_data: HashMap<String, String> = HashMap::new();
        self.write_file(&empty_data, &key, &salt)?;

        *self.state.lock().map_err(|_| "internal lock error".to_string())? = StrongholdState::Unlocked {
            data: empty_data,
            key,
            salt: salt.to_vec(),
        };
        Ok(())
    }

    /// 解锁：用主密码解密快照，将凭证加载到内存。
    pub fn unlock(&self, master_password: &str) -> Result<(), String> {
        let content = std::fs::read_to_string(&self.path)
            .map_err(|_| "无法读取凭证库文件，请检查文件是否存在且可访问".to_string())?;
        let hold: HoldFile = serde_json::from_str(&content)
            .map_err(|_| "凭证库文件格式异常，无法解锁".to_string())?;

        if hold.version != 1 {
            return Err("凭证库文件版本不受支持，无法解锁".to_string());
        }

        let salt = B64
            .decode(&hold.salt)
            .map_err(|_| "凭证库文件内容异常，无法解锁".to_string())?;
        let key = derive_key(master_password, &salt)?;

        let nonce_bytes = B64
            .decode(&hold.nonce)
            .map_err(|_| "凭证库文件内容异常，无法解锁".to_string())?;
        let ciphertext = B64
            .decode(&hold.ciphertext)
            .map_err(|_| "凭证库文件内容异常，无法解锁".to_string())?;

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let nonce = Nonce::from_slice(&nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_slice())
            .map_err(|_| "主密码不正确，无法解锁凭证库".to_string())?;

        let data: HashMap<String, String> = serde_json::from_slice(&plaintext)
            .map_err(|_| "凭证库内容异常，无法完成解锁".to_string())?;

        *self.state.lock().map_err(|_| "internal lock error".to_string())? = StrongholdState::Unlocked { data, key, salt };
        Ok(())
    }

    /// 锁定：清除内存中的解密数据与密钥。
    pub fn lock(&self) {
        let mut guard = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if matches!(&*guard, StrongholdState::Unlocked { .. }) {
            *guard = StrongholdState::Locked;
        }
    }

    /// 读取凭证；Uninitialized 时返回 None，Locked 时返回 Err。
    pub fn get(&self, key: &CredentialKey) -> Result<Option<String>, String> {
        match &*self.state.lock().map_err(|_| "internal lock error".to_string())? {
            StrongholdState::Unlocked { data, .. } => {
                Ok(data.get(&key.stronghold_key()).cloned())
            }
            StrongholdState::Locked => Err("Stronghold 已锁定，请先输入主密码解锁".to_string()),
            StrongholdState::Uninitialized => Ok(None),
        }
    }

    /// 保存凭证并立即将变更写入磁盘。
    pub fn set(&self, key: &CredentialKey, password: &str) -> Result<(), String> {
        let (data_snapshot, key_bytes, salt) = {
            let mut guard = self.state.lock().map_err(|_| "internal lock error".to_string())?;
            match &mut *guard {
                StrongholdState::Unlocked { data, key: k, salt: s } => {
                    data.insert(key.stronghold_key(), password.to_string());
                    (data.clone(), *k, s.clone())
                }
                StrongholdState::Locked => {
                    return Err("Stronghold 已锁定，请先输入主密码解锁".to_string())
                }
                StrongholdState::Uninitialized => {
                    return Err("Stronghold 未初始化，请先设置主密码".to_string())
                }
            }
        };
        self.write_file(&data_snapshot, &key_bytes, &salt)
    }

    /// 删除凭证并写入磁盘；条目不存在时视为成功。
    pub fn delete(&self, key: &CredentialKey) -> Result<(), String> {
        let (data_snapshot, key_bytes, salt) = {
            let mut guard = self.state.lock().map_err(|_| "internal lock error".to_string())?;
            match &mut *guard {
                StrongholdState::Unlocked { data, key: k, salt: s } => {
                    data.remove(&key.stronghold_key());
                    (data.clone(), *k, s.clone())
                }
                StrongholdState::Locked => {
                    return Err("Stronghold 已锁定，请先输入主密码解锁".to_string())
                }
                StrongholdState::Uninitialized => return Ok(()),
            }
        };
        self.write_file(&data_snapshot, &key_bytes, &salt)
    }

    /// 检查凭证是否存在（不获取内容）。
    pub fn exists(&self, key: &CredentialKey) -> bool {
        match &*self.state.lock().unwrap_or_else(|p| p.into_inner()) {
            StrongholdState::Unlocked { data, .. } => data.contains_key(&key.stronghold_key()),
            _ => false,
        }
    }

    /// 列出所有已保存凭证的 tunnel_id。
    pub fn list_keys(&self) -> Vec<String> {
        match &*self.state.lock().unwrap_or_else(|p| p.into_inner()) {
            StrongholdState::Unlocked { data, .. } => data
                .keys()
                .filter_map(|k| k.strip_prefix("ssh/").map(|s| s.to_string()))
                .collect(),
            _ => vec![],
        }
    }

    fn write_file(
        &self,
        data: &HashMap<String, String>,
        key: &[u8; 32],
        salt: &[u8],
    ) -> Result<(), String> {
        let plaintext = serde_json::to_string(data).map_err(err_str)?;
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(err_str)?;

        let hold = HoldFile {
            version: 1,
            salt: B64.encode(salt),
            nonce: B64.encode(nonce.as_slice()),
            ciphertext: B64.encode(&ciphertext),
        };
        let json = serde_json::to_string_pretty(&hold).map_err(err_str)?;

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(err_str)?;
        }
        std::fs::write(&self.path, json).map_err(err_str)
    }
}

/// 从主密码和 salt 派生 32 字节 AES 密钥（Argon2id 默认参数）。
fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    argon2::Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(err_str)?;
    Ok(key)
}

/// 快照文件的推荐默认路径辅助函数。
pub fn default_snapshot_path() -> Option<PathBuf> {
    crate::config::app_data_dir().map(|p| p.join("credentials.hold"))
}

/// 从路径字符串解析快照路径，支持 `~` 展开。
pub fn resolve_snapshot_path(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        dirs::home_dir()
            .map(|h| h.join(&path[2..]))
            .unwrap_or_else(|| PathBuf::from(path))
    } else {
        PathBuf::from(path)
    }
}
