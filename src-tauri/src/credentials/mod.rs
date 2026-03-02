//! 凭证管理层：运行时内存缓存 + 持久化后端（OS 钥匙串 / Stronghold 加密文件）。
//!
//! 外部使用入口：
//!   - `CredentialManager`：统一凭证读写，查询链为 内存 → 持久化后端
//!   - `new_from_settings`：从应用设置构建 CredentialManager 实例

pub mod manager;
pub mod memory;
pub mod os_keychain;
pub mod stronghold;
pub mod types;

pub use manager::{new_from_settings, CredentialManager};
pub use stronghold::StrongholdStatus;
pub use types::{AuthMethod, CredentialInfo, CredentialKey, CredentialStoreKind};
