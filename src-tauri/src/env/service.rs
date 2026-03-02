//! 环境 CRUD：连接（本地/SSH）+ 多 context；发现 context、设置当前 context。

use crate::config::{self, KubeFlowConfig, KubeFlowConfigFile};
use crate::env::types::{Environment, EnvironmentContext, EnvironmentSource};
use crate::env::SshTunnel;
use chrono::Utc;
use uuid::Uuid;

pub struct EnvService;

impl EnvService {
    fn config_path() -> Option<std::path::PathBuf> {
        config::kube_flow_config_path()
    }

    fn load_config() -> Result<KubeFlowConfig, config::ConfigError> {
        let path = Self::config_path().ok_or_else(|| {
            config::ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "app data dir not found",
            ))
        })?;
        config::ensure_app_data_dir().ok_or_else(|| {
            config::ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "could not create app data dir",
            ))
        })?;
        KubeFlowConfigFile::load(&path)
    }

    fn save_config(cfg: &KubeFlowConfig) -> Result<(), config::ConfigError> {
        let path = Self::config_path().ok_or_else(|| {
            config::ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "app data dir not found",
            ))
        })?;
        KubeFlowConfigFile::save(cfg, &path)
    }

    /// 列出所有环境（连接），按收藏、sort_order、last_used_at 排序。
    pub fn list() -> Result<Vec<Environment>, config::ConfigError> {
        let mut envs = Self::load_config()?.environments;
        envs.sort_by(|a, b| {
            match (a.is_favorite, b.is_favorite) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.sort_order.cmp(&b.sort_order).reverse().then_with(|| {
                    let a_ts = a.last_used_at.as_deref().unwrap_or("");
                    let b_ts = b.last_used_at.as_deref().unwrap_or("");
                    b_ts.cmp(a_ts)
                }),
            }
        });
        Ok(envs)
    }

    /// 添加环境并写回。
    pub fn add(env: Environment) -> Result<(), config::ConfigError> {
        let mut cfg = Self::load_config()?;
        if cfg.environments.iter().any(|e| e.id == env.id) {
            return Err(config::ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "environment id already exists",
            )));
        }
        cfg.environments.push(env);
        Self::save_config(&cfg)
    }

    /// 更新指定 id 的环境。
    pub fn update(env: Environment) -> Result<(), config::ConfigError> {
        let mut cfg = Self::load_config()?;
        let pos = cfg.environments.iter().position(|e| e.id == env.id).ok_or_else(|| {
            config::ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "environment not found",
            ))
        })?;
        cfg.environments[pos] = env;
        Self::save_config(&cfg)
    }

    /// 删除环境。
    pub fn delete(id: &str) -> Result<(), config::ConfigError> {
        let mut cfg = Self::load_config()?;
        cfg.environments.retain(|e| e.id != id);
        Self::save_config(&cfg)
    }

    /// 设置当前选中的 context。
    pub fn set_current_context(env_id: &str, context_name: &str) -> Result<(), config::ConfigError> {
        let mut cfg = Self::load_config()?;
        let env = cfg.environments.iter_mut().find(|e| e.id == env_id).ok_or_else(|| {
            config::ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "environment not found",
            ))
        })?;
        if env.contexts.iter().any(|c| c.context_name == context_name) {
            env.current_context = Some(context_name.to_string());
            Self::save_config(&cfg)
        } else {
            Err(config::ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "context not found in environment",
            )))
        }
    }

    /// 标记环境为最近使用。
    pub fn touch(id: &str) -> Result<(), config::ConfigError> {
        let mut cfg = Self::load_config()?;
        if let Some(e) = cfg.environments.iter_mut().find(|e| e.id == id) {
            e.last_used_at = Some(Utc::now().to_rfc3339());
        }
        Self::save_config(&cfg)
    }

    /// 从 kubeconfig 文件解析出 context 列表。
    pub fn list_contexts_from_kubeconfig(
        kubeconfig_path: &str,
    ) -> Result<Vec<KubeContextInfo>, config::ConfigError> {
        let path = expand_tilde(kubeconfig_path);
        let content = std::fs::read_to_string(&path).map_err(config::ConfigError::Io)?;
        let raw: KubeconfigRaw = serde_yaml::from_str(&content).map_err(|e| {
            config::ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            ))
        })?;
        let mut out = Vec::new();
        for ctx in raw.contexts.unwrap_or_default() {
            let name = ctx.name.clone();
            let cluster = ctx.context.as_ref().and_then(|c| c.cluster.clone()).unwrap_or_default();
            let namespace = ctx.context.as_ref().and_then(|c| c.namespace.clone());
            out.push(KubeContextInfo {
                context_name: name,
                cluster_name: cluster,
                namespace,
            });
        }
        Ok(out)
    }

    /// 新建本地连接：从 kubeconfig 发现 contexts，创建一条环境（含多 context）。
    pub fn create_local(
        display_name: String,
        kubeconfig_path: String,
        selected_contexts: Vec<KubeContextInfo>,
        tags: Vec<String>,
    ) -> Result<Environment, config::ConfigError> {
        if selected_contexts.is_empty() {
            return Err(config::ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "at least one context required",
            )));
        }
        let contexts: Vec<EnvironmentContext> = selected_contexts
            .into_iter()
            .map(|c| EnvironmentContext {
                context_name: c.context_name,
                display_name: None,
                default_namespace: c.namespace,
                cluster_name: Some(c.cluster_name),
            })
            .collect();
        let current = contexts.first().map(|c| c.context_name.clone());
        let tags: Vec<String> = tags.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let env = Environment {
            id: Uuid::new_v4().to_string(),
            source: EnvironmentSource::LocalKubeconfig,
            display_name,
            tags,
            is_favorite: false,
            sort_order: 0,
            kubeconfig_path: Some(kubeconfig_path),
            ssh_tunnel_id: None,
            contexts,
            current_context: current,
            last_used_at: None,
        };
        Self::add(env.clone())?;
        Ok(env)
    }

    /// 列出配置中的 SSH 隧道，供前端「新建 SSH 环境」时选择。
    pub fn list_ssh_tunnels() -> Result<Vec<SshTunnel>, config::ConfigError> {
        Ok(Self::load_config()?.ssh_tunnels)
    }

    /// 按 id 获取 SSH 隧道配置；供工作台构建 Client 时使用。
    pub fn get_ssh_tunnel(id: &str) -> Result<Option<SshTunnel>, config::ConfigError> {
        Ok(Self::load_config()?.ssh_tunnels.into_iter().find(|t| t.id == id))
    }

    /// 新建 SSH 隧道连接：先需有 ssh_tunnels 中配置，contexts 可先空或后续通过隧道发现后更新。
    pub fn create_ssh(
        display_name: String,
        ssh_tunnel_id: String,
        contexts: Vec<EnvironmentContext>,
        tags: Vec<String>,
    ) -> Result<Environment, config::ConfigError> {
        let current = contexts.first().map(|c| c.context_name.clone());
        let tags: Vec<String> = tags.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let env = Environment {
            id: Uuid::new_v4().to_string(),
            source: EnvironmentSource::SshTunnel,
            display_name,
            tags,
            is_favorite: false,
            sort_order: 0,
            kubeconfig_path: None,
            ssh_tunnel_id: Some(ssh_tunnel_id),
            contexts,
            current_context: current,
            last_used_at: None,
        };
        Self::add(env.clone())?;
        Ok(env)
    }

    /// 使用 ~/.ssh/config 中的 Host 创建 SSH 环境：若该 Host 尚无对应隧道配置则自动写入一条，再创建环境。
    pub fn create_ssh_with_host(
        display_name: String,
        ssh_host: String,
        remote_kubeconfig_path: String,
        local_port: Option<u16>,
        contexts: Vec<EnvironmentContext>,
        tags: Vec<String>,
    ) -> Result<Environment, config::ConfigError> {
        let tunnel_id = format!(
            "ssh-{}",
            ssh_host
                .chars()
                .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
                .collect::<String>()
        );
        let mut cfg = Self::load_config()?;
        let existing_idx = cfg
            .ssh_tunnels
            .iter()
            .position(|t| t.id == tunnel_id || t.ssh_host == ssh_host);
        let id = if let Some(idx) = existing_idx {
            cfg.ssh_tunnels[idx].local_port = local_port;
            Self::save_config(&cfg)?;
            cfg.ssh_tunnels[idx].id.clone()
        } else {
            cfg.ssh_tunnels.push(SshTunnel {
                id: tunnel_id.clone(),
                name: ssh_host.clone(),
                ssh_host: ssh_host.clone(),
                remote_kubeconfig_path: remote_kubeconfig_path.clone(),
                local_port,
                remote_port: None,
                auth_method: Default::default(),
                has_saved_credential: false,
            });
            Self::save_config(&cfg)?;
            tunnel_id
        };
        Self::create_ssh(display_name, id, contexts, tags)
    }

    /// 确保存在对应 Host 的隧道配置，返回 tunnel id；若已存在则更新 remote_kubeconfig_path、local_port。
    pub fn ensure_ssh_tunnel_for_host(
        ssh_host: String,
        remote_kubeconfig_path: String,
        local_port: Option<u16>,
    ) -> Result<String, config::ConfigError> {
        let tunnel_id = format!(
            "ssh-{}",
            ssh_host
                .chars()
                .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
                .collect::<String>()
        );
        let mut cfg = Self::load_config()?;
        if let Some(idx) = cfg.ssh_tunnels.iter().position(|t| t.id == tunnel_id || t.ssh_host == ssh_host) {
            let id = cfg.ssh_tunnels[idx].id.clone();
            cfg.ssh_tunnels[idx].remote_kubeconfig_path = remote_kubeconfig_path;
            cfg.ssh_tunnels[idx].name = ssh_host.clone();
            cfg.ssh_tunnels[idx].local_port = local_port;
            Self::save_config(&cfg)?;
            return Ok(id);
        }
        cfg.ssh_tunnels.push(SshTunnel {
            id: tunnel_id.clone(),
            name: ssh_host.clone(),
            ssh_host: ssh_host.clone(),
            remote_kubeconfig_path: remote_kubeconfig_path.clone(),
            local_port,
            remote_port: None,
            auth_method: Default::default(),
            has_saved_credential: false,
        });
        Self::save_config(&cfg)?;
        Ok(tunnel_id)
    }
}

fn expand_tilde(p: &str) -> std::path::PathBuf {
    if p.starts_with("~/") {
        dirs::home_dir().map(|h| h.join(&p[2..])).unwrap_or_else(|| p.into())
    } else {
        p.into()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KubeContextInfo {
    pub context_name: String,
    pub cluster_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct KubeconfigRaw {
    contexts: Option<Vec<KubeContextRaw>>,
}

#[derive(Debug, serde::Deserialize)]
struct KubeContextRaw {
    name: String,
    context: Option<KubeContextInner>,
}

#[derive(Debug, serde::Deserialize)]
struct KubeContextInner {
    cluster: Option<String>,
    namespace: Option<String>,
}
