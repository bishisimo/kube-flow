//! 运行时内存凭证缓存：进程退出后自动清除，所有模式下均启用。

use std::collections::HashMap;
use std::sync::Mutex;

use super::types::CredentialKey;

pub struct MemoryCache {
    store: Mutex<HashMap<CredentialKey, String>>,
}

impl MemoryCache {
    pub fn new() -> Self {
        Self { store: Mutex::new(HashMap::new()) }
    }

    pub fn get(&self, key: &CredentialKey) -> Option<String> {
        self.store.lock().unwrap().get(key).cloned()
    }

    pub fn set(&self, key: CredentialKey, password: String) {
        self.store.lock().unwrap().insert(key, password);
    }

    pub fn remove(&self, key: &CredentialKey) {
        self.store.lock().unwrap().remove(key);
    }
}
