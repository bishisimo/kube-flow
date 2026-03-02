//! OS 系统钥匙串后端：macOS Keychain / Windows Credential Manager / libsecret。
//! 使用 `keyring` crate，service 固定为 "kube-flow"，username 为 "ssh/{tunnel_id}"。

use super::types::CredentialKey;

const KEYRING_SERVICE: &str = "kube-flow";

pub struct OsKeychainBackend;

impl OsKeychainBackend {
    /// 读取凭证；未找到返回 Ok(None)，钥匙串不可用时返回 Err。
    pub fn get(key: &CredentialKey) -> Result<Option<String>, String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, &key.keyring_username())
            .map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    /// 保存凭证；覆盖已存在的条目。
    pub fn set(key: &CredentialKey, password: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, &key.keyring_username())
            .map_err(|e| e.to_string())?;
        entry.set_password(password).map_err(|e| e.to_string())
    }

    /// 删除凭证；若条目不存在则视为成功。
    pub fn delete(key: &CredentialKey) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, &key.keyring_username())
            .map_err(|e| e.to_string())?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// 检查凭证是否存在（不获取密码内容）。
    pub fn exists(key: &CredentialKey) -> bool {
        let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &key.keyring_username()) else {
            return false;
        };
        matches!(entry.get_password(), Ok(_))
    }
}
