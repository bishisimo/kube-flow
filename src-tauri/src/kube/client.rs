//! 从环境配置构建 kube::Client，并按 env_id 缓存；支持 local_kubeconfig 与 ssh_tunnel。

use crate::config::LogLevel;
use crate::credentials::{CredentialKey, CredentialManager};
use crate::debug_log;
use crate::env::{EnvService, Environment, EnvironmentSource};
use crate::config::{app_settings_config_path, AppSettingsConfig};
use crate::env::TunnelMappingMode;
use crate::kube::tunnel::{ConnectionProgressPayload, SshTunnelRunner, TunnelError};
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::{Config, Client};
use std::path::Path;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use tauri::Emitter;
use tokio::sync::RwLock;

fn expand_tilde(p: &str) -> std::path::PathBuf {
    if p.starts_with("~/") {
        dirs::home_dir().map(|h| h.join(&p[2..])).unwrap_or_else(|| p.into())
    } else {
        p.into()
    }
}

/// 按环境 id 缓存 Client；支持 local_kubeconfig 与 ssh_tunnel（隧道由 SshTunnelRunner 维护）。
#[derive(Clone)]
pub struct KubeClientStore {
    cache: Arc<RwLock<std::collections::HashMap<String, kube::Client>>>,
    tunnel_runner: Arc<SshTunnelRunner>,
    app_handle: tauri::AppHandle,
}

impl KubeClientStore {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self {
            cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
            tunnel_runner: Arc::new(SshTunnelRunner::new()),
            app_handle,
        }
    }

    /// 为环境构建或获取已缓存的 Client。支持 local_kubeconfig 与 ssh_tunnel。
    pub async fn get_or_build(&self, env: &Environment) -> Result<kube::Client, KubeClientError> {
        {
            let guard = self.cache.read().await;
            if let Some(client) = guard.get(&env.id) {
                return Ok(client.clone());
            }
        }
        let client = match env.source {
            EnvironmentSource::LocalKubeconfig => self.build_local(env).await?,
            EnvironmentSource::SshTunnel => self.build_ssh_tunnel(env).await?,
        };
        {
            let mut guard = self.cache.write().await;
            guard.insert(env.id.clone(), client.clone());
        }
        Ok(client)
    }

    async fn build_local(&self, env: &Environment) -> Result<kube::Client, KubeClientError> {
        let path = env
            .kubeconfig_path
            .as_deref()
            .ok_or(KubeClientError::MissingKubeconfigPath)?;
        let path = expand_tilde(path);
        if !path.exists() {
            return Err(KubeClientError::FileNotFound(path.display().to_string()));
        }
        let context_name = env.effective_context().ok_or(KubeClientError::NoContext)?;
        let default_ns = env.default_namespace();
        build_client_from_kubeconfig_path(&path, context_name, default_ns).await
    }

    async fn build_ssh_tunnel(&self, env: &Environment) -> Result<kube::Client, KubeClientError> {
        let tunnel_id = env
            .ssh_tunnel_id
            .as_deref()
            .ok_or(KubeClientError::SshTunnel("环境缺少 ssh_tunnel_id".into()))?;
        let tunnel = EnvService::get_ssh_tunnel(tunnel_id)
            .map_err(|e| KubeClientError::SshTunnel(e.to_string()))?
            .ok_or_else(|| KubeClientError::SshTunnel(format!("未找到隧道配置: {}", tunnel_id)))?;
        let env_id = env.id.clone();
        let tunnel_id_owned = tunnel_id.to_string();
        let ssh_host = tunnel.ssh_host.clone();
        let remote_path = tunnel.remote_kubeconfig_path.clone();
        let preferred_port = tunnel.local_port;
        let preferred_context = env.effective_context().map(String::from);
        let settings = app_settings_config_path()
            .and_then(|p| AppSettingsConfig::load(&p).ok())
            .unwrap_or_default();
        let tunnel_mode = match settings.default_ssh_tunnel_mode().as_str() {
            "builtin" => TunnelMappingMode::Builtin,
            _ => TunnelMappingMode::Ssh,
        };

        // 从 CredentialManager 查询密码（内存缓存优先，再查持久化后端）
        let password: Option<String> = {
            use tauri::Manager;
            let manager: tauri::State<'_, CredentialManager> = self.app_handle.state();
            let key = CredentialKey::new(&tunnel_id_owned);
            manager.get(&key, &settings.security).ok().flatten()
        };

        let runner = Arc::clone(&self.tunnel_runner);
        let app = self.app_handle.clone();
        let (progress_tx, progress_rx) = mpsc::channel::<ConnectionProgressPayload>();
        let _recv_handle = thread::spawn(move || {
            while let Ok(payload) = progress_rx.recv() {
                let _ = app.emit("connection-progress", payload);
            }
        });
        let (local_port, virtual_yaml) = tokio::task::spawn_blocking(move || {
            runner.ensure_tunnel(
                env_id,
                tunnel_id_owned,
                ssh_host,
                remote_path,
                preferred_port,
                preferred_context,
                tunnel_mode,
                password,
                Some(progress_tx),
            )
        })
        .await
        .map_err(|e| KubeClientError::SshTunnel(e.to_string()))?
        .map_err(|e| match e {
            // AuthRequired 保留结构化错误码供前端解析
            TunnelError::AuthRequired(tid) => KubeClientError::AuthRequired(tid),
            other => KubeClientError::SshTunnel(other.to_string()),
        })?;
        let context_name = env
            .effective_context()
            .map(String::from)
            .or_else(|| current_context_from_yaml(&virtual_yaml))
            .ok_or(KubeClientError::NoContext)?;
        let default_ns = env.default_namespace();
        let server = format!("https://127.0.0.1:{}", local_port);
        debug_log::log_virtual_kubeconfig(
            &env.id,
            &context_name,
            &server,
            default_ns,
            LogLevel::Info,
        );
        build_client_from_kubeconfig_str(&virtual_yaml, &context_name, default_ns).await
    }

    /// 若该环境为 SSH 隧道且隧道已建立，返回本地映射端口；否则返回 None。
    pub fn get_tunnel_local_port(&self, env_id: &str) -> Option<u16> {
        self.tunnel_runner.get_local_port(env_id)
    }

    /// 移除某环境的缓存并关闭 SSH 隧道（若有）。
    pub async fn remove(&self, env_id: &str) {
        self.tunnel_runner.close_tunnel(env_id);
        let mut guard = self.cache.write().await;
        guard.remove(env_id);
    }

    /// 关闭所有 SSH 隧道；应用退出前调用，避免孤儿进程与端口占用。
    pub fn close_all_tunnels(&self) {
        self.tunnel_runner.close_all_tunnels();
    }
}

/// 从 kubeconfig YAML 中解析 current-context，供 SSH 环境无 context 时回退。
fn current_context_from_yaml(yaml: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Root {
        #[serde(rename = "current-context")]
        current_context: Option<String>,
    }
    let root: Root = serde_yaml::from_str(yaml).ok()?;
    root.current_context
}

async fn build_client_from_kubeconfig_path(
    path: &Path,
    context_name: &str,
    default_namespace: Option<&str>,
) -> Result<kube::Client, KubeClientError> {
    let content = std::fs::read_to_string(path).map_err(KubeClientError::Io)?;
    build_client_from_kubeconfig_str(&content, context_name, default_namespace).await
}

async fn build_client_from_kubeconfig_str(
    content: &str,
    context_name: &str,
    default_namespace: Option<&str>,
) -> Result<kube::Client, KubeClientError> {
    let kubeconfig: Kubeconfig = serde_yaml::from_str(content).map_err(|e| KubeClientError::Yaml(e.to_string()))?;
    let options = KubeConfigOptions {
        context: Some(context_name.to_string()),
        cluster: None,
        user: None,
    };
    let mut config = Config::from_custom_kubeconfig(kubeconfig, &options)
        .await
        .map_err(|e| KubeClientError::KubeConfig(e.to_string()))?;
    if let Some(ns) = default_namespace {
        config.default_namespace = ns.to_string();
    }
    let client = Client::try_from(config).map_err(|e: kube::Error| KubeClientError::Client(e.to_string()))?;
    Ok(client)
}

#[derive(Debug, thiserror::Error)]
pub enum KubeClientError {
    #[error("no context in environment")]
    NoContext,
    #[error("missing kubeconfig_path")]
    MissingKubeconfigPath,
    #[error("file not found: {0}")]
    FileNotFound(String),
    #[error("ssh tunnel: {0}")]
    SshTunnel(String),
    /// SSH 认证需要密码，错误字符串固定格式 "SSH_AUTH_REQUIRED:{tunnel_id}" 供前端解析。
    #[error("SSH_AUTH_REQUIRED:{0}")]
    AuthRequired(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml: {0}")]
    Yaml(String),
    #[error("kubeconfig: {0}")]
    KubeConfig(String),
    #[error("client: {0}")]
    Client(String),
}
