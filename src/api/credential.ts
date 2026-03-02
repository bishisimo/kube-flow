/**
 * 凭证管理与 Stronghold 操作的 Tauri invoke 封装。
 */
import { invoke } from "@tauri-apps/api/core";

export type CredentialStoreKind = "os_keychain" | "stronghold";
export type StrongholdStatus = "uninitialized" | "locked" | "unlocked";

export interface SecurityConfig {
  credential_store: CredentialStoreKind;
  stronghold_snapshot_path: string;
  auto_lock_minutes: number;
}

export interface CredentialInfo {
  tunnel_id: string;
  store: string;
}

// ──────────────────────────────────────────
// 安全设置
// ──────────────────────────────────────────

export function securityGetSettings(): Promise<SecurityConfig> {
  return invoke("security_get_settings");
}

export function securitySetCredentialStore(store: CredentialStoreKind): Promise<void> {
  return invoke("security_set_credential_store", { store });
}

export function securitySetStrongholdPath(path: string): Promise<void> {
  return invoke("security_set_stronghold_path", { path });
}

export function securitySetAutoLockMinutes(minutes: number): Promise<void> {
  return invoke("security_set_auto_lock_minutes", { minutes });
}

// ──────────────────────────────────────────
// 凭证 CRUD
// ──────────────────────────────────────────

/** 保存密码到持久化后端（同时更新 TOML 中的 has_saved_credential 标记）。 */
export function credentialSave(tunnelId: string, password: string): Promise<void> {
  return invoke("credential_save", { tunnelId, password });
}

/** 从持久化后端删除凭证。 */
export function credentialDelete(tunnelId: string): Promise<void> {
  return invoke("credential_delete", { tunnelId });
}

/** 检查持久化后端中是否存在凭证。 */
export function credentialExists(tunnelId: string): Promise<boolean> {
  return invoke("credential_exists", { tunnelId });
}

/** 列出持久化后端中所有已保存凭证摘要（不含密码）。 */
export function credentialList(): Promise<CredentialInfo[]> {
  return invoke("credential_list");
}

/**
 * 仅写入内存缓存（用户通过交互弹窗输入密码后调用）。
 * 不写持久化后端，应用重启后清除。
 */
export function credentialCacheOnly(tunnelId: string, password: string): Promise<void> {
  return invoke("credential_cache_only", { tunnelId, password });
}

// ──────────────────────────────────────────
// Stronghold 状态机
// ──────────────────────────────────────────

export function strongholdGetStatus(): Promise<StrongholdStatus> {
  return invoke("stronghold_get_status");
}

export function strongholdInitialize(masterPassword: string): Promise<void> {
  return invoke("stronghold_initialize", { masterPassword });
}

export function strongholdUnlock(masterPassword: string): Promise<void> {
  return invoke("stronghold_unlock", { masterPassword });
}

export function strongholdLock(): Promise<void> {
  return invoke("stronghold_lock");
}
