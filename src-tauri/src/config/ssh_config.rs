//! 解析 ~/.ssh/config：列出 Host、按 Host 取连接参数（供 SSH 隧道使用）。

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// 单个 Host 的连接参数（从 ~/.ssh/config 解析）。
#[derive(Debug, Clone)]
pub struct SshHostConfig {
    #[allow(dead_code)]
    pub host: String,
    pub hostname: String,
    pub port: u16,
    pub user: String,
    pub identity_file: Option<PathBuf>,
    /// ProxyCommand 原始值；若存在则通过代理连接。
    pub proxy_command: Option<String>,
    /// ProxyJump 原始值；可转换为 ProxyCommand。
    pub proxy_jump: Option<String>,
}

fn default_ssh_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ssh").join("config"))
}

fn expand_tilde_path(p: &str) -> PathBuf {
    if p.starts_with("~/") {
        dirs::home_dir()
            .map(|h| h.join(&p[2..]))
            .unwrap_or_else(|| p.into())
    } else {
        p.into()
    }
}

/// 从 ~/.ssh/config 解析出所有 Host 名（去重、保持顺序）。
pub fn list_hosts() -> Vec<String> {
    let path = match default_ssh_config_path() {
        Some(p) => p,
        None => return vec![],
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.len() > 5 && line[..5].eq_ignore_ascii_case("host ") {
            let rest = line[5..].trim();
            if let Some(first) = rest.split_ascii_whitespace().next() {
                let host = first.to_string();
                if !host.is_empty() && seen.insert(host.clone()) {
                    out.push(host);
                }
            }
        }
    }
    out
}

/// 解析 config 得到每个 Host 块内的键值；块以 "Host" 行开始。同一 Host 行多个名会各占一条。
fn parse_blocks(content: &str) -> Vec<(String, HashMap<String, String>)> {
    let mut blocks = Vec::new();
    let mut current_hosts: Vec<String> = Vec::new();
    let mut current_kv: HashMap<String, String> = HashMap::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.len() > 5 && trimmed[..5].eq_ignore_ascii_case("host ") {
            for h in current_hosts.drain(..) {
                blocks.push((h, current_kv.clone()));
            }
            let rest = trimmed[5..].trim();
            current_hosts = rest.split_ascii_whitespace().map(String::from).filter(|s| !s.is_empty()).collect();
            current_kv = HashMap::new();
            continue;
        }
        if let Some(sep) = trimmed.find(|c: char| c.is_whitespace()) {
            let key = trimmed[..sep].trim().to_lowercase();
            let value = trimmed[sep..].trim().to_string();
            if !key.is_empty() && !value.is_empty() {
                current_kv.insert(key, value);
            }
        }
    }
    for h in current_hosts {
        blocks.push((h, current_kv.clone()));
    }
    blocks
}

/// 获取指定 Host 的连接参数；若不存在或缺少必要字段则返回 None。
/// 合并默认值：HostName 默认同 Host，Port 默认 22，User 默认当前系统用户。
pub fn get_host_config(host: &str) -> Option<SshHostConfig> {
    let path = default_ssh_config_path()?;
    let content = std::fs::read_to_string(&path).ok()?;
    let blocks = parse_blocks(&content);
    let (_, kv) = blocks.into_iter().find(|(h, _)| h == host)?;
    let hostname = kv
        .get("hostname")
        .cloned()
        .unwrap_or_else(|| host.to_string());
    let port = kv
        .get("port")
        .and_then(|s| s.parse().ok())
        .unwrap_or(22);
    let user = kv.get("user").cloned().unwrap_or_else(|| {
        std::env::var("USER").unwrap_or_else(|_| "root".to_string())
    });
    let identity_file = kv.get("identityfile").map(|s| expand_tilde_path(s));
    let proxy_command = kv.get("proxycommand").cloned();
    let proxy_jump = kv.get("proxyjump").cloned();
    Some(SshHostConfig {
        host: host.to_string(),
        hostname,
        port,
        user,
        identity_file,
        proxy_command,
        proxy_jump,
    })
}

/// 解析 ProxyCommand 或 ProxyJump，返回替换 %h、%p 后的命令行（可执行文件 + 参数列表）。
/// ProxyJump "jump" 等价于 ProxyCommand "ssh -W %h:%p jump"；多跳 "a,b,c" 为 "ssh -W %h:%p -J b,c a"。
pub fn resolve_proxy_command(host_config: &SshHostConfig) -> Option<Vec<String>> {
    let cmd_str = host_config.proxy_command.clone().or_else(|| {
        host_config.proxy_jump.as_ref().map(|jump| {
            let parts: Vec<&str> = jump.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if parts.len() > 1 {
                format!("ssh -W %h:%p -J {} {}", parts[1..].join(","), parts[0])
            } else if parts.len() == 1 {
                format!("ssh -W %h:%p {}", parts[0])
            } else {
                String::new()
            }
        })
    })?;
    let cmd_str = cmd_str.trim_start_matches("exec ").trim();
    if cmd_str.trim().is_empty() {
        return None;
    }
    let replaced = cmd_str
        .replace("%h", &host_config.hostname)
        .replace("%p", &host_config.port.to_string());
    let parts = shell_words::split(&replaced).unwrap_or_else(|_| {
        replaced.split_whitespace().map(String::from).collect()
    });
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}
