//! 凭证管理器：统一入口，查询链为 内存缓存 → 持久化后端 → None。
//! 持久化后端由应用设置（SecurityConfig）决定，每次操作时读取当前设置。

use std::path::PathBuf;
use std::sync::Mutex;

use super::memory::MemoryCache;
use super::os_keychain::OsKeychainBackend;
use super::stronghold::{resolve_snapshot_path, StrongholdBackend, StrongholdStatus};
use super::types::{CredentialInfo, CredentialKey, CredentialStoreKind};
use crate::config::SecurityConfig;

pub struct CredentialManager {
    memory: MemoryCache,
    stronghold: Mutex<StrongholdBackend>,
}

impl CredentialManager {
    /// 使用给定 Stronghold 快照路径初始化（应用启动时调用）。
    pub fn new(stronghold_path: PathBuf) -> Self {
        Self {
            memory: MemoryCache::new(),
            stronghold: Mutex::new(StrongholdBackend::new(stronghold_path)),
        }
    }

    // ──────────────────────────────────────────
    // 查询链
    // ──────────────────────────────────────────

    /// 完整查询链：内存缓存 → 持久化后端。
    /// 找到后自动写入内存缓存供本次 session 复用。
    /// 返回 None 表示两者均无记录，应由调用方触发交互输入弹窗。
    pub fn get(&self, key: &CredentialKey, cfg: &SecurityConfig) -> Result<Option<String>, String> {
        if let Some(p) = self.memory.get(key) {
            return Ok(Some(p));
        }
        let result = self.get_from_backend(key, cfg)?;
        if let Some(ref p) = result {
            self.memory.set(key.clone(), p.clone());
        }
        Ok(result)
    }

    // ──────────────────────────────────────────
    // 写操作
    // ──────────────────────────────────────────

    /// 仅写入内存缓存（用于交互输入后的 session 复用，不持久化）。
    pub fn cache_only(&self, key: CredentialKey, password: String) {
        self.memory.set(key, password);
    }

    /// 写入持久化后端，同时更新内存缓存。
    pub fn save(&self, key: &CredentialKey, password: &str, cfg: &SecurityConfig) -> Result<(), String> {
        self.save_to_backend(key, password, cfg)?;
        self.memory.set(key.clone(), password.to_string());
        Ok(())
    }

    /// 从持久化后端与内存缓存中删除凭证。
    pub fn delete(&self, key: &CredentialKey, cfg: &SecurityConfig) -> Result<(), String> {
        self.delete_from_backend(key, cfg)?;
        self.memory.remove(key);
        Ok(())
    }

    /// 检查持久化后端中是否存在凭证（不获取内容）。
    pub fn exists_in_backend(&self, key: &CredentialKey, cfg: &SecurityConfig) -> bool {
        match cfg.credential_store {
            CredentialStoreKind::OsKeychain => OsKeychainBackend::exists(key),
            CredentialStoreKind::Stronghold => {
                self.stronghold.lock().unwrap().exists(key)
            }
        }
    }

    /// 列出持久化后端中所有已保存凭证的摘要。
    pub fn list(&self, cfg: &SecurityConfig) -> Vec<CredentialInfo> {
        let store_name = cfg.credential_store.as_str().to_string();
        let keys = match cfg.credential_store {
            CredentialStoreKind::OsKeychain => {
                // OS 钥匙串无法枚举，只返回空列表（UI 通过 has_saved_credential 标记展示）
                vec![]
            }
            CredentialStoreKind::Stronghold => {
                self.stronghold.lock().unwrap().list_keys()
            }
        };
        keys.into_iter()
            .map(|tunnel_id| CredentialInfo { tunnel_id, store: store_name.clone() })
            .collect()
    }

    // ──────────────────────────────────────────
    // Stronghold 状态机操作
    // ──────────────────────────────────────────

    pub fn stronghold_status(&self) -> StrongholdStatus {
        self.stronghold.lock().unwrap().status()
    }

    pub fn stronghold_initialize(&self, master_password: &str) -> Result<(), String> {
        self.stronghold.lock().unwrap().initialize(master_password)
    }

    pub fn stronghold_unlock(&self, master_password: &str) -> Result<(), String> {
        self.stronghold.lock().unwrap().unlock(master_password)
    }

    pub fn stronghold_lock(&self) {
        self.stronghold.lock().unwrap().lock();
    }

    /// 更新 Stronghold 快照路径（设置变更时调用）；旧凭证不迁移。
    pub fn stronghold_reset_path(&self, new_path: PathBuf) {
        *self.stronghold.lock().unwrap() = StrongholdBackend::new(new_path);
    }

    // ──────────────────────────────────────────
    // 内部：后端读写路由
    // ──────────────────────────────────────────

    fn get_from_backend(
        &self,
        key: &CredentialKey,
        cfg: &SecurityConfig,
    ) -> Result<Option<String>, String> {
        match cfg.credential_store {
            CredentialStoreKind::OsKeychain => OsKeychainBackend::get(key),
            CredentialStoreKind::Stronghold => self.stronghold.lock().unwrap().get(key),
        }
    }

    fn save_to_backend(
        &self,
        key: &CredentialKey,
        password: &str,
        cfg: &SecurityConfig,
    ) -> Result<(), String> {
        match cfg.credential_store {
            CredentialStoreKind::OsKeychain => OsKeychainBackend::set(key, password),
            CredentialStoreKind::Stronghold => self.stronghold.lock().unwrap().set(key, password),
        }
    }

    fn delete_from_backend(&self, key: &CredentialKey, cfg: &SecurityConfig) -> Result<(), String> {
        match cfg.credential_store {
            CredentialStoreKind::OsKeychain => OsKeychainBackend::delete(key),
            CredentialStoreKind::Stronghold => self.stronghold.lock().unwrap().delete(key),
        }
    }
}

/// 从当前 AppSettingsConfig 构建 CredentialManager 的工厂函数。
pub fn new_from_settings(settings: &crate::config::AppSettingsConfig) -> CredentialManager {
    let path = if settings.security.stronghold_snapshot_path.is_empty() {
        super::stronghold::default_snapshot_path()
            .unwrap_or_else(|| PathBuf::from("credentials.hold"))
    } else {
        resolve_snapshot_path(&settings.security.stronghold_snapshot_path)
    };
    CredentialManager::new(path)
}
