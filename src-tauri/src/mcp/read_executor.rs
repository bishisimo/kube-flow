//! MCP 只读执行器：本地 kubeconfig 直连；SSH 环境需走 App Gateway。

use crate::env::{EnvService, Environment, EnvironmentSource};
use crate::kube::{
    build_local_client, describe_resource, get_pod_logs, get_resource_yaml, list_config_maps,
    list_cron_jobs, list_daemon_sets, list_deployments, list_ingresses, list_jobs, list_namespaces,
    list_nodes, list_persistent_volume_claims, list_pods, list_replica_sets, list_secrets,
    list_services, list_stateful_sets,
};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct EnvSummary {
    pub id: String,
    pub display_name: String,
    pub tags: Vec<String>,
    pub current_context: Option<String>,
    pub source: String,
}

/// 列出策略白名单内的环境摘要（不含敏感路径）。
pub fn list_environment_summaries(allowed_env_ids: &[String]) -> Result<Vec<EnvSummary>, String> {
    let envs = EnvService::list().map_err(|e| e.to_string())?;
    let allow: std::collections::HashSet<&str> =
        allowed_env_ids.iter().map(|s| s.as_str()).collect();
    Ok(envs
        .into_iter()
        .filter(|e| allow.contains(e.id.as_str()))
        .map(|e| EnvSummary {
            id: e.id,
            display_name: e.display_name,
            tags: e.tags,
            current_context: e.current_context,
            source: match e.source {
                EnvironmentSource::LocalKubeconfig => "local_kubeconfig".into(),
                EnvironmentSource::SshTunnel => "ssh_tunnel".into(),
            },
        })
        .collect())
}

pub fn load_environment(env_id: &str) -> Result<Environment, String> {
    EnvService::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|e| e.id == env_id)
        .ok_or_else(|| format!("environment not found: {env_id}"))
}

/// 仅为 local_kubeconfig 构建客户端；SSH 返回错误提示走 Gateway。
pub async fn client_for_local_env(env: &Environment) -> Result<kube::Client, String> {
    match env.source {
        EnvironmentSource::LocalKubeconfig => {
            let path = env
                .kubeconfig_path
                .as_deref()
                .ok_or_else(|| "missing kubeconfig_path".to_string())?;
            let ctx = env
                .effective_context()
                .ok_or_else(|| "no context".to_string())?;
            build_local_client(path, ctx, env.default_namespace())
                .await
                .map_err(|e| e.to_string())
        }
        EnvironmentSource::SshTunnel => Err(
            "SSH 环境只读操作需要 kube-flow App Gateway 在线（stdio 无法独立建隧道）".into(),
        ),
    }
}

pub async fn list_resources_json(
    client: &kube::Client,
    kind: &str,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Value, String> {
    let kind_l = kind.to_ascii_lowercase();
    match kind_l.as_str() {
        "namespace" | "namespaces" => {
            let items = list_namespaces(client, label_selector)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_value(items).map_err(|e| e.to_string())
        }
        "node" | "nodes" => {
            let items = list_nodes(client, label_selector, &[])
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_value(items).map_err(|e| e.to_string())
        }
        "pod" | "pods" => {
            let items = list_pods(client, namespace, label_selector)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_value(items).map_err(|e| e.to_string())
        }
        "deployment" | "deployments" => {
            let items = list_deployments(client, namespace, label_selector)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_value(items).map_err(|e| e.to_string())
        }
        "service" | "services" | "svc" => {
            let items = list_services(client, namespace, label_selector)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_value(items).map_err(|e| e.to_string())
        }
        "statefulset" | "statefulsets" | "sts" => {
            let items = list_stateful_sets(client, namespace, label_selector)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_value(items).map_err(|e| e.to_string())
        }
        "daemonset" | "daemonsets" | "ds" => {
            let items = list_daemon_sets(client, namespace, label_selector)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_value(items).map_err(|e| e.to_string())
        }
        "replicaset" | "replicasets" | "rs" => {
            let items = list_replica_sets(client, namespace, label_selector)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_value(items).map_err(|e| e.to_string())
        }
        "configmap" | "configmaps" | "cm" => {
            let items = list_config_maps(client, namespace, label_selector)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_value(items).map_err(|e| e.to_string())
        }
        "secret" | "secrets" => {
            let items = list_secrets(client, namespace, label_selector)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_value(items).map_err(|e| e.to_string())
        }
        "job" | "jobs" => {
            let items = list_jobs(client, namespace, label_selector)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_value(items).map_err(|e| e.to_string())
        }
        "cronjob" | "cronjobs" | "cj" => {
            let items = list_cron_jobs(client, namespace, label_selector)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_value(items).map_err(|e| e.to_string())
        }
        "ingress" | "ingresses" | "ing" => {
            let items = list_ingresses(client, namespace, label_selector)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_value(items).map_err(|e| e.to_string())
        }
        "persistentvolumeclaim" | "persistentvolumeclaims" | "pvc" => {
            let items = list_persistent_volume_claims(client, namespace, label_selector)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::to_value(items).map_err(|e| e.to_string())
        }
        _ => Err(format!("unsupported kind for list: {kind}")),
    }
}

pub async fn get_yaml(
    client: &kube::Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<String, String> {
    get_resource_yaml(client, kind, name, namespace)
        .await
        .map_err(|e| e.to_string())
}

pub async fn describe(
    client: &kube::Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<String, String> {
    describe_resource(client, kind, name, namespace)
        .await
        .map(|r| r.markdown)
        .map_err(|e| e.to_string())
}

pub async fn logs(
    client: &kube::Client,
    namespace: &str,
    pod_name: &str,
    container: Option<&str>,
    tail_lines: Option<i64>,
) -> Result<String, String> {
    get_pod_logs(
        client,
        namespace,
        pod_name,
        container,
        tail_lines.or(Some(200)),
        None,
        false,
        false,
    )
    .await
}
