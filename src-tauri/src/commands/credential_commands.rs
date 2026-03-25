//! Tauri 命令：凭证管理与 Stronghold 操作。
//!
//! 命令分组：
//!   安全设置   - security_get_settings / security_set_credential_store /
//!               security_set_stronghold_path / security_set_auto_lock_minutes
//!   凭证 CRUD  - credential_save / credential_delete / credential_exists /
//!               credential_get / credential_list / credential_cache_only
//!   Stronghold - stronghold_get_status / stronghold_initialize /
//!               stronghold_unlock / stronghold_lock

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{Manager, State};

use crate::config::{app_settings_config_path, ensure_app_data_dir, AppSettingsConfig, SecurityConfig};
use crate::credentials::{CredentialInfo, CredentialKey, CredentialManager, CredentialStoreKind, StrongholdStatus};

/// Stronghold 自动锁定调度器：通过递增序号取消旧计时任务。
pub struct StrongholdAutoLockController {
    ticket: AtomicU64,
}

impl Default for StrongholdAutoLockController {
    fn default() -> Self {
        Self {
            ticket: AtomicU64::new(0),
        }
    }
}

impl StrongholdAutoLockController {
    /// 取消当前自动锁定计时任务。
    pub fn cancel(&self) {
        self.ticket.fetch_add(1, Ordering::SeqCst);
    }

    /// 启动新的自动锁定计时任务；minutes=0 表示仅取消不启动。
    pub fn schedule(&self, app: tauri::AppHandle, minutes: u32) {
        let token = self.ticket.fetch_add(1, Ordering::SeqCst) + 1;
        if minutes == 0 {
            return;
        }
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(u64::from(minutes) * 60));
            let Some(ctrl) = app.try_state::<StrongholdAutoLockController>() else {
                return;
            };
            if ctrl.ticket.load(Ordering::SeqCst) != token {
                return;
            }
            let manager = app.state::<CredentialManager>();
            manager.stronghold_lock();
        });
    }
}

// ──────────────────────────────────────────
// 安全设置
// ──────────────────────────────────────────

fn load_settings() -> Result<AppSettingsConfig, String> {
    let path = app_settings_config_path().ok_or("app data dir 不可用")?;
    AppSettingsConfig::load(&path).map_err(|e| e.to_string())
}

fn save_settings(cfg: &AppSettingsConfig) -> Result<(), String> {
    ensure_app_data_dir().ok_or("无法创建 app data dir")?;
    let path = app_settings_config_path().ok_or("app data dir 不可用")?;
    cfg.save(&path).map_err(|e| e.to_string())
}

/// 读取安全配置（凭证存储类型、Stronghold 路径、自动锁定时间）。
#[tauri::command]
pub fn security_get_settings() -> Result<SecurityConfig, String> {
    Ok(load_settings()?.security)
}

/// 更新凭证存储后端类型（os_keychain / stronghold）。
/// 若切换到 stronghold，CredentialManager 会重置到新路径。
#[tauri::command]
pub fn security_set_credential_store(
    store: String,
    manager: State<'_, CredentialManager>,
    auto_lock: State<'_, StrongholdAutoLockController>,
) -> Result<(), String> {
    let kind = CredentialStoreKind::from_str(&store);
    let mut cfg = load_settings()?;
    cfg.security.credential_store = kind;
    save_settings(&cfg)?;
    auto_lock.cancel();
    // 若切换到 stronghold，重置后端实例（路径不变）
    if kind == CredentialStoreKind::Stronghold {
        let path = crate::credentials::stronghold::resolve_snapshot_path(
            &cfg.security.effective_stronghold_path(),
        );
        manager.stronghold_reset_path(path);
    }
    Ok(())
}

/// 更新 Stronghold 快照文件路径；切换路径后旧凭证不迁移。
#[tauri::command]
pub fn security_set_stronghold_path(
    path: String,
    manager: State<'_, CredentialManager>,
    auto_lock: State<'_, StrongholdAutoLockController>,
) -> Result<(), String> {
    let mut cfg = load_settings()?;
    cfg.security.stronghold_snapshot_path = path.clone();
    save_settings(&cfg)?;
    auto_lock.cancel();
    let resolved = crate::credentials::stronghold::resolve_snapshot_path(&path);
    manager.stronghold_reset_path(resolved);
    Ok(())
}

/// 更新 Stronghold 自动锁定时间（分钟）；0 表示不自动锁定。
#[tauri::command]
pub fn security_set_auto_lock_minutes(
    minutes: u32,
    manager: State<'_, CredentialManager>,
    auto_lock: State<'_, StrongholdAutoLockController>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut cfg = load_settings()?;
    cfg.security.auto_lock_minutes = minutes;
    save_settings(&cfg)?;
    if manager.stronghold_status() == StrongholdStatus::Unlocked {
        auto_lock.schedule(app, minutes);
    } else {
        auto_lock.cancel();
    }
    Ok(())
}

// ──────────────────────────────────────────
// 凭证 CRUD
// ──────────────────────────────────────────

/// 保存 SSH 隧道密码到持久化后端，同时更新 TOML 中的 has_saved_credential 标记。
#[tauri::command]
pub fn credential_save(tunnel_id: String, password: String, manager: State<'_, CredentialManager>) -> Result<(), String> {
    let cfg = load_settings()?;
    let key = CredentialKey::new(&tunnel_id);
    manager.save(&key, &password, &cfg.security)?;
    // 更新 has_saved_credential 标记
    update_has_saved_credential(&tunnel_id, true)?;
    Ok(())
}

/// 从持久化后端删除凭证，同时清除内存缓存和 TOML 标记。
#[tauri::command]
pub fn credential_delete(tunnel_id: String, manager: State<'_, CredentialManager>) -> Result<(), String> {
    let cfg = load_settings()?;
    let key = CredentialKey::new(&tunnel_id);
    manager.delete(&key, &cfg.security)?;
    update_has_saved_credential(&tunnel_id, false)?;
    Ok(())
}

/// 检查指定隧道在持久化后端中是否有已保存的凭证。
#[tauri::command]
pub fn credential_exists(tunnel_id: String, manager: State<'_, CredentialManager>) -> Result<bool, String> {
    let cfg = load_settings()?;
    let key = CredentialKey::new(&tunnel_id);
    Ok(manager.exists_in_backend(&key, &cfg.security))
}

/// 读取指定 key 对应的凭证内容；不存在时返回 None。
#[tauri::command]
pub fn credential_get(tunnel_id: String, manager: State<'_, CredentialManager>) -> Result<Option<String>, String> {
    let cfg = load_settings()?;
    let key = CredentialKey::new(&tunnel_id);
    manager.get(&key, &cfg.security)
}

/// 列出持久化后端中所有已保存凭证的摘要（不含密码）。
#[tauri::command]
pub fn credential_list(manager: State<'_, CredentialManager>) -> Result<Vec<CredentialInfo>, String> {
    let cfg = load_settings()?;
    Ok(manager.list(&cfg.security))
}

/// 仅将密码存入内存缓存（交互弹窗输入后调用），不写持久化后端。
#[tauri::command]
pub fn credential_cache_only(tunnel_id: String, password: String, manager: State<'_, CredentialManager>) {
    manager.cache_only(CredentialKey::new(tunnel_id), password);
}

// ──────────────────────────────────────────
// Stronghold 状态机
// ──────────────────────────────────────────

/// 获取 Stronghold 当前状态：uninitialized / locked / unlocked。
#[tauri::command]
pub fn stronghold_get_status(manager: State<'_, CredentialManager>) -> StrongholdStatus {
    manager.stronghold_status()
}

/// 初始化 Stronghold：使用主密码创建新的加密快照（覆盖已存在的文件）。
#[tauri::command]
pub fn stronghold_initialize(
    master_password: String,
    manager: State<'_, CredentialManager>,
    auto_lock: State<'_, StrongholdAutoLockController>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    manager.stronghold_initialize(&master_password)?;
    let minutes = load_settings()?.security.auto_lock_minutes;
    auto_lock.schedule(app, minutes);
    Ok(())
}

/// 解锁 Stronghold：输入主密码，将凭证加载到内存。
#[tauri::command]
pub fn stronghold_unlock(
    master_password: String,
    manager: State<'_, CredentialManager>,
    auto_lock: State<'_, StrongholdAutoLockController>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    manager.stronghold_unlock(&master_password)?;
    let minutes = load_settings()?.security.auto_lock_minutes;
    auto_lock.schedule(app, minutes);
    Ok(())
}

/// 锁定 Stronghold：清除内存中的解密数据，需重新输入主密码才能访问凭证。
#[tauri::command]
pub fn stronghold_lock(
    manager: State<'_, CredentialManager>,
    auto_lock: State<'_, StrongholdAutoLockController>,
) {
    auto_lock.cancel();
    manager.stronghold_lock();
}

// ──────────────────────────────────────────
// 内部辅助
// ──────────────────────────────────────────

/// 更新指定隧道的 has_saved_credential 字段并写回 TOML。
fn update_has_saved_credential(tunnel_id: &str, value: bool) -> Result<(), String> {
    use crate::config::{KubeFlowConfigFile, kube_flow_config_path};
    let path = kube_flow_config_path().ok_or("config path unavailable")?;
    let mut cfg = KubeFlowConfigFile::load(&path).map_err(|e| e.to_string())?;
    if let Some(t) = cfg.ssh_tunnels.iter_mut().find(|t| t.id == tunnel_id) {
        t.has_saved_credential = value;
    }
    KubeFlowConfigFile::save(&cfg, &path).map_err(|e| e.to_string())
}

// ──────────────────────────────────────────
// SecurityConfig 扩展方法
// ──────────────────────────────────────────

impl SecurityConfig {
    /// 返回实际使用的 Stronghold 快照路径（空字符串时使用默认路径）。
    pub fn effective_stronghold_path(&self) -> String {
        if self.stronghold_snapshot_path.is_empty() {
            crate::credentials::stronghold::default_snapshot_path()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "credentials.hold".to_string())
        } else {
            self.stronghold_snapshot_path.clone()
        }
    }
}
