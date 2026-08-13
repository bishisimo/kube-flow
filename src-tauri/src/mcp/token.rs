//! MCP 访问令牌：生成、读取、校验（文件权限尽量 0600）。

use crate::config::{app_data_dir, ensure_app_data_dir, mcp_token_path};
use rand::RngCore;
use std::fs;
use std::path::PathBuf;

/// 确保 token 文件存在；若无则生成并返回。
pub fn ensure_token() -> Result<String, String> {
    ensure_app_data_dir().ok_or_else(|| "app data dir unavailable".to_string())?;
    let path = token_file_path()?;
    if path.exists() {
        return read_token();
    }
    let token = generate_token();
    write_token(&token)?;
    Ok(token)
}

pub fn read_token() -> Result<String, String> {
    let path = token_file_path()?;
    let s = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let t = s.trim().to_string();
    if t.is_empty() {
        return Err("mcp token 为空".into());
    }
    Ok(t)
}

pub fn validate_token(presented: &str) -> Result<(), String> {
    let expected = read_token()?;
    if presented == expected {
        Ok(())
    } else {
        Err("invalid mcp token".into())
    }
}

pub fn regenerate_token() -> Result<String, String> {
    let token = generate_token();
    write_token(&token)?;
    Ok(token)
}

fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn token_file_path() -> Result<PathBuf, String> {
    if let Ok(policy) = super::policy::McpPolicy::load_default() {
        if let Some(dir) = app_data_dir() {
            return Ok(dir.join(policy.mcp.token_path));
        }
    }
    mcp_token_path().ok_or_else(|| "token path unavailable".to_string())
}

fn write_token(token: &str) -> Result<(), String> {
    let path = token_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, token).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}
