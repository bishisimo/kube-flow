//! 短时会话授权：对非破坏类写操作可在确认后免重复弹窗。

use super::capabilities::Capability;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Grant {
    env_id: String,
    capability: Capability,
    expires_at: DateTime<Utc>,
}

/// 内存中的 SessionGrant 表。
#[derive(Debug, Default)]
pub struct SessionGrantStore {
    inner: Mutex<HashMap<String, Grant>>,
}

impl SessionGrantStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn key(env_id: &str, capability: Capability) -> String {
        format!("{}:{}", env_id, capability.as_str())
    }

    /// 授予指定 env+capability，持续 `minutes` 分钟。破坏类能力不会被接受。
    pub fn grant(&self, env_id: &str, capability: Capability, minutes: u32) -> Result<(), String> {
        if capability.requires_forced_approval() {
            return Err("破坏类能力不支持 SessionGrant".into());
        }
        let expires_at = Utc::now() + Duration::minutes(minutes as i64);
        let mut guard = self.inner.lock().map_err(|_| "lock error".to_string())?;
        guard.insert(
            Self::key(env_id, capability),
            Grant {
                env_id: env_id.to_string(),
                capability,
                expires_at,
            },
        );
        Ok(())
    }

    pub fn has_valid(&self, env_id: &str, capability: Capability) -> bool {
        if capability.requires_forced_approval() {
            return false;
        }
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return false,
        };
        let k = Self::key(env_id, capability);
        if let Some(g) = guard.get(&k) {
            if g.expires_at > Utc::now() {
                return true;
            }
            guard.remove(&k);
        }
        false
    }

    pub fn revoke_all(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.clear();
        }
    }
}
