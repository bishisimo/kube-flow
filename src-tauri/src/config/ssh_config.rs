//! 解析和维护 ~/.ssh/config：列出 Host、按 Host 取连接参数（供 SSH 隧道使用），
//! 并为设置页提供普通 Host 块的可视化编辑能力。

use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConfigOption {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConfigEntry {
    pub host: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_jump: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_command: Option<String>,
    #[serde(default)]
    pub options: Vec<SshConfigOption>,
    pub source_file: String,
    pub editable: bool,
    #[serde(default)]
    pub issues: Vec<String>,
}

#[derive(Debug, Clone)]
struct ParsedHostBlock {
    hosts: Vec<String>,
    kv: HashMap<String, String>,
    options: Vec<SshConfigOption>,
    start_line: usize,
    end_line: usize,
}

fn default_ssh_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ssh").join("config"))
}

pub fn default_config_path_string() -> Option<String> {
    default_ssh_config_path().map(|p| p.to_string_lossy().to_string())
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

fn parse_host_blocks(content: &str) -> Vec<ParsedHostBlock> {
    let lines: Vec<&str> = content.lines().collect();
    let mut blocks = Vec::new();
    let mut current_hosts: Vec<String> = Vec::new();
    let mut current_kv: HashMap<String, String> = HashMap::new();
    let mut current_options: Vec<SshConfigOption> = Vec::new();
    let mut start_line: Option<usize> = None;

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.len() > 5 && trimmed[..5].eq_ignore_ascii_case("host ") {
            let host_start = if idx > 0 && lines[idx - 1].trim() == "# Managed by kube-flow" {
                idx - 1
            } else {
                idx
            };
            if let Some(start) = start_line {
                blocks.push(ParsedHostBlock {
                    hosts: std::mem::take(&mut current_hosts),
                    kv: std::mem::take(&mut current_kv),
                    options: std::mem::take(&mut current_options),
                    start_line: start,
                    end_line: host_start,
                });
            }
            let rest = trimmed[5..].trim();
            current_hosts = rest
                .split_ascii_whitespace()
                .map(String::from)
                .filter(|s| !s.is_empty())
                .collect();
            start_line = Some(host_start);
            continue;
        }
        if start_line.is_some() {
            if let Some(sep) = trimmed.find(|c: char| c.is_whitespace()) {
                let key = trimmed[..sep].trim().to_string();
                let value = trimmed[sep..].trim().to_string();
                if !key.is_empty() && !value.is_empty() {
                    current_kv.insert(key.to_lowercase(), value.clone());
                    current_options.push(SshConfigOption { key, value });
                }
            }
        }
    }
    if let Some(start) = start_line {
        blocks.push(ParsedHostBlock {
            hosts: current_hosts,
            kv: current_kv,
            options: current_options,
            start_line: start,
            end_line: lines.len(),
        });
    }
    blocks
}

fn is_plain_host_name(host: &str) -> bool {
    !host.trim().is_empty()
        && !host.chars().any(char::is_whitespace)
        && !host.contains('*')
        && !host.contains('?')
        && !host.starts_with('!')
}

fn entry_from_block(block: ParsedHostBlock, source_file: &str) -> Option<SshConfigEntry> {
    let host = block.hosts.first()?.clone();
    let aliases = block.hosts.iter().skip(1).cloned().collect::<Vec<_>>();
    let mut issues = Vec::new();
    let editable = block.hosts.iter().all(|h| is_plain_host_name(h));
    if !editable {
        issues.push("包含通配符、取反或空白字符，暂不支持可视化编辑".to_string());
    }
    let reserved = [
        "hostname",
        "user",
        "port",
        "identityfile",
        "proxyjump",
        "proxycommand",
    ];
    let options = block
        .options
        .into_iter()
        .filter(|item| {
            !reserved
                .iter()
                .any(|key| item.key.eq_ignore_ascii_case(key))
        })
        .collect();
    Some(SshConfigEntry {
        host,
        aliases,
        hostname: block.kv.get("hostname").cloned(),
        user: block.kv.get("user").cloned(),
        port: block.kv.get("port").and_then(|s| s.parse().ok()),
        identity_file: block.kv.get("identityfile").cloned(),
        proxy_jump: block.kv.get("proxyjump").cloned(),
        proxy_command: block.kv.get("proxycommand").cloned(),
        options,
        source_file: source_file.to_string(),
        editable,
        issues,
    })
}

pub fn list_entries() -> Result<Vec<SshConfigEntry>, std::io::Error> {
    let path = default_ssh_config_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "home dir not found"))?;
    let source_file = path.to_string_lossy().to_string();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    Ok(parse_host_blocks(&content)
        .into_iter()
        .filter_map(|block| entry_from_block(block, &source_file))
        .collect())
}

fn validate_entry(entry: &SshConfigEntry) -> Result<(), std::io::Error> {
    if !is_plain_host_name(&entry.host) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Host 别名不能为空，且不能包含空白、通配符或 !",
        ));
    }
    for alias in &entry.aliases {
        if !is_plain_host_name(alias) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Host 别名不能包含空白、通配符或 !",
            ));
        }
    }
    if entry.proxy_jump.as_deref().unwrap_or("").trim().len() > 0
        && entry.proxy_command.as_deref().unwrap_or("").trim().len() > 0
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "ProxyJump 与 ProxyCommand 只能配置一个",
        ));
    }
    Ok(())
}

fn push_option(lines: &mut Vec<String>, key: &str, value: Option<&str>) {
    if let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) {
        lines.push(format!("  {} {}", key, value));
    }
}

fn render_entry(entry: &SshConfigEntry) -> String {
    let mut host_names = vec![entry.host.trim().to_string()];
    host_names.extend(
        entry
            .aliases
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
    );
    let mut lines = vec![
        "# Managed by kube-flow".to_string(),
        format!("Host {}", host_names.join(" ")),
    ];
    push_option(&mut lines, "HostName", entry.hostname.as_deref());
    push_option(&mut lines, "User", entry.user.as_deref());
    if let Some(port) = entry.port {
        lines.push(format!("  Port {}", port));
    }
    push_option(&mut lines, "IdentityFile", entry.identity_file.as_deref());
    push_option(&mut lines, "ProxyJump", entry.proxy_jump.as_deref());
    push_option(&mut lines, "ProxyCommand", entry.proxy_command.as_deref());
    for option in &entry.options {
        let key = option.key.trim();
        let value = option.value.trim();
        if !key.is_empty() && !value.is_empty() {
            lines.push(format!("  {} {}", key, value));
        }
    }
    lines.join("\n")
}

fn backup_config(path: &std::path::Path, content: &str) -> Result<(), std::io::Error> {
    if content.is_empty() {
        return Ok(());
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup = path.with_file_name(format!("config.kube-flow.{}.bak", ts));
    std::fs::write(backup, content)
}

fn write_config_atomically(path: &std::path::Path, content: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_file_name("config.kube-flow.tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(tmp, path)
}

fn replace_or_append_block(
    content: &str,
    entry: &SshConfigEntry,
) -> Result<String, std::io::Error> {
    let blocks = parse_host_blocks(content);
    let lines: Vec<&str> = content.lines().collect();
    let rendered = render_entry(entry);
    if let Some(block) = blocks
        .iter()
        .find(|block| block.hosts.iter().any(|h| h == &entry.host))
    {
        if !block.hosts.iter().all(|h| is_plain_host_name(h)) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "该 Host 块包含通配符或特殊规则，暂不支持可视化编辑",
            ));
        }
        let mut next = Vec::new();
        next.extend(lines[..block.start_line].iter().map(|s| (*s).to_string()));
        next.push(rendered);
        next.extend(lines[block.end_line..].iter().map(|s| (*s).to_string()));
        return Ok(next.join("\n") + "\n");
    }
    let mut next = content.trim_end_matches('\n').to_string();
    if !next.is_empty() {
        next.push_str("\n\n");
    }
    next.push_str(&rendered);
    next.push('\n');
    Ok(next)
}

pub fn upsert_entry(entry: SshConfigEntry) -> Result<(), std::io::Error> {
    validate_entry(&entry)?;
    let path = default_ssh_config_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "home dir not found"))?;
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let next = replace_or_append_block(&content, &entry)?;
    backup_config(&path, &content)?;
    write_config_atomically(&path, &next)
}

pub fn delete_entry(host: &str) -> Result<(), std::io::Error> {
    if !is_plain_host_name(host) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Host 无效",
        ));
    }
    let path = default_ssh_config_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "home dir not found"))?;
    let content = std::fs::read_to_string(&path)?;
    let blocks = parse_host_blocks(&content);
    let block = blocks
        .iter()
        .find(|block| block.hosts.iter().any(|h| h == host))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Host 不存在"))?;
    if !block.hosts.iter().all(|h| is_plain_host_name(h)) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "该 Host 块包含通配符或特殊规则，暂不支持删除",
        ));
    }
    let lines: Vec<&str> = content.lines().collect();
    let mut next = Vec::new();
    next.extend(lines[..block.start_line].iter().map(|s| (*s).to_string()));
    next.extend(lines[block.end_line..].iter().map(|s| (*s).to_string()));
    backup_config(&path, &content)?;
    write_config_atomically(&path, &(next.join("\n") + "\n"))
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
            current_hosts = rest
                .split_ascii_whitespace()
                .map(String::from)
                .filter(|s| !s.is_empty())
                .collect();
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
    let port = kv.get("port").and_then(|s| s.parse().ok()).unwrap_or(22);
    let user = kv
        .get("user")
        .cloned()
        .unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "root".to_string()));
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
            let parts: Vec<&str> = jump
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
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
    let parts = shell_words::split(&replaced)
        .unwrap_or_else(|_| replaced.split_whitespace().map(String::from).collect());
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}
