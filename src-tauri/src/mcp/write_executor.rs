//! MCP 写路径与 Gateway 侧只读：复用 KubeClientStore。

use crate::kube::{
    apply_resource_yaml, delete_resource, deploy_resource_yaml, download_file_from_pod,
    patch_resource_strategic, restart_workload, resume_workload, stop_workload,
    upload_file_to_pod, KubeClientStore,
};
use crate::mcp::read_executor;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, AttachParams};
use serde_json::{json, Value};
use tauri::AppHandle;
use tokio::io::AsyncReadExt;
use tokio::sync::oneshot;
use uuid::Uuid;

pub async fn resolve_client(
    store: &KubeClientStore,
    env_id: &str,
) -> Result<(crate::env::Environment, kube::Client), String> {
    crate::commands::kube_command_context::kube_client_for_env_id(store, env_id).await
}

pub async fn handle_method(
    store: &KubeClientStore,
    app: &AppHandle,
    method: &str,
    params: &Value,
) -> Result<Value, String> {
    match method {
        "list_resources" => {
            let env_id = req_str(params, "env_id")?;
            let kind = req_str(params, "kind")?;
            let namespace = opt_str(params, "namespace");
            let label = opt_str(params, "label_selector");
            let (_env, client) = resolve_client(store, env_id).await?;
            read_executor::list_resources_json(
                &client,
                kind,
                namespace,
                label,
            )
            .await
        }
        "get_resource_yaml" => {
            let env_id = req_str(params, "env_id")?;
            let kind = req_str(params, "kind")?;
            let name = req_str(params, "name")?;
            let namespace = opt_str(params, "namespace");
            let (_env, client) = resolve_client(store, env_id).await?;
            let yaml = read_executor::get_yaml(&client, kind, name, namespace).await?;
            Ok(json!({ "yaml": yaml }))
        }
        "describe_resource" => {
            let env_id = req_str(params, "env_id")?;
            let kind = req_str(params, "kind")?;
            let name = req_str(params, "name")?;
            let namespace = opt_str(params, "namespace");
            let (_env, client) = resolve_client(store, env_id).await?;
            let markdown = read_executor::describe(&client, kind, name, namespace).await?;
            Ok(json!({ "markdown": markdown }))
        }
        "get_logs" => {
            let env_id = req_str(params, "env_id")?;
            let namespace = req_str(params, "namespace")?;
            let pod = req_str(params, "pod")?;
            let container = opt_str(params, "container");
            let tail = params.get("tail_lines").and_then(|v| v.as_i64());
            let (_env, client) = resolve_client(store, env_id).await?;
            let text = read_executor::logs(&client, namespace, pod, container, tail).await?;
            Ok(json!({ "logs": text }))
        }
        "apply_resource_yaml" => {
            let env_id = req_str(params, "env_id")?;
            let yaml = req_str(params, "yaml")?;
            let (_env, client) = resolve_client(store, env_id).await?;
            apply_resource_yaml(&client, yaml)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({ "applied": true }))
        }
        "patch_resource" => {
            let env_id = req_str(params, "env_id")?;
            let kind = req_str(params, "kind")?;
            let name = req_str(params, "name")?;
            let namespace = opt_str(params, "namespace");
            let patch = params
                .get("patch")
                .cloned()
                .ok_or_else(|| "missing patch".to_string())?;
            let (_env, client) = resolve_client(store, env_id).await?;
            patch_resource_strategic(&client, kind, name, namespace, patch)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({ "patched": true }))
        }
        "workload_action" => {
            let env_id = req_str(params, "env_id")?;
            let action = req_str(params, "action")?;
            let kind = req_str(params, "kind")?;
            let name = req_str(params, "name")?;
            let namespace = opt_str(params, "namespace");
            let (_env, client) = resolve_client(store, env_id).await?;
            match action {
                "stop" => stop_workload(&client, kind, name, namespace)
                    .await
                    .map_err(|e| e.to_string())?,
                "resume" => resume_workload(&client, kind, name, namespace)
                    .await
                    .map_err(|e| e.to_string())?,
                "restart" => restart_workload(&client, kind, name, namespace)
                    .await
                    .map_err(|e| e.to_string())?,
                _ => return Err(format!("unknown action: {action}")),
            }
            Ok(json!({ "ok": true, "action": action }))
        }
        "create_or_deploy" => {
            let env_id = req_str(params, "env_id")?;
            let yaml = req_str(params, "yaml")?;
            let (_env, client) = resolve_client(store, env_id).await?;
            let strategy = crate::commands::kube_command_context::load_app_settings()
                .map(|s| crate::config::ResourceDeployStrategy::from_str(&s.resource_deploy_strategy))
                .unwrap_or_default();
            deploy_resource_yaml(&client, yaml, strategy)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({ "deployed": true }))
        }
        "delete_resource" => {
            let env_id = req_str(params, "env_id")?;
            let kind = req_str(params, "kind")?;
            let name = req_str(params, "name")?;
            let namespace = opt_str(params, "namespace");
            let (_env, client) = resolve_client(store, env_id).await?;
            delete_resource(&client, kind, name, namespace)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({ "deleted": true }))
        }
        "pod_exec" => {
            let env_id = req_str(params, "env_id")?;
            let namespace = req_str(params, "namespace")?;
            let pod = req_str(params, "pod")?;
            let container = opt_str(params, "container");
            let command = params
                .get("command")
                .and_then(|v| v.as_array())
                .ok_or_else(|| "command must be a string array".to_string())?;
            let cmd: Vec<String> = command
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            if cmd.is_empty() {
                return Err("command cannot be empty".into());
            }
            let (_env, client) = resolve_client(store, env_id).await?;
            let output = exec_once(&client, namespace, pod, container, &cmd).await?;
            Ok(json!({ "output": output }))
        }
        "pod_file_transfer" => {
            let env_id = req_str(params, "env_id")?;
            let direction = req_str(params, "direction")?;
            let namespace = req_str(params, "namespace")?;
            let pod = req_str(params, "pod")?;
            let container = opt_str(params, "container");
            let local_path = req_str(params, "local_path")?;
            let remote_path = req_str(params, "remote_path")?;
            let overwrite = params
                .get("overwrite")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let (_env, client) = resolve_client(store, env_id).await?;
            let transfer_id = Uuid::new_v4().to_string();
            let (_tx, mut cancel_rx) = oneshot::channel::<()>();
            match direction {
                "upload" => {
                    upload_file_to_pod(
                        app,
                        &transfer_id,
                        &mut cancel_rx,
                        client,
                        namespace,
                        pod,
                        container,
                        local_path,
                        remote_path,
                        overwrite,
                    )
                    .await?;
                }
                "download" => {
                    download_file_from_pod(
                        app,
                        &transfer_id,
                        &mut cancel_rx,
                        client,
                        namespace,
                        pod,
                        container,
                        remote_path,
                        local_path,
                        overwrite,
                    )
                    .await?;
                }
                _ => return Err(format!("unknown direction: {direction}")),
            }
            Ok(json!({ "ok": true, "direction": direction }))
        }
        _ => Err(format!("unknown method: {method}")),
    }
}

async fn exec_once(
    client: &kube::Client,
    namespace: &str,
    pod: &str,
    container: Option<&str>,
    command: &[String],
) -> Result<String, String> {
    let api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let mut params = AttachParams::default()
        .stdin(false)
        .stdout(true)
        .stderr(true)
        .tty(false);
    if let Some(c) = container {
        params = params.container(c);
    }
    let mut attached = api
        .exec(pod, command.iter().map(|s| s.as_str()), &params)
        .await
        .map_err(|e| e.to_string())?;
    let mut stdout = attached
        .stdout()
        .ok_or_else(|| "no stdout".to_string())?;
    let mut buf = Vec::new();
    stdout
        .read_to_end(&mut buf)
        .await
        .map_err(|e| e.to_string())?;
    // 等待进程结束
    let _ = attached.take_status();
    Ok(String::from_utf8_lossy(&buf).to_string())
}

fn req_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, String> {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("missing string param: {key}"))
}

fn opt_str<'a>(params: &'a Value, key: &str) -> Option<&'a str> {
    params.get(key).and_then(|v| v.as_str())
}

/// 从 YAML 粗提取 kind / namespace / name，供门控使用。
pub fn meta_from_yaml(yaml: &str) -> (Option<String>, Option<String>, Option<String>) {
    let Ok(v): Result<Value, _> = serde_yaml::from_str(yaml) else {
        return (None, None, None);
    };
    let kind = v.get("kind").and_then(|x| x.as_str()).map(str::to_string);
    let name = v
        .pointer("/metadata/name")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    let namespace = v
        .pointer("/metadata/namespace")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    (kind, namespace, name)
}

pub fn capability_for_method(method: &str) -> Option<crate::mcp::capabilities::Capability> {
    use crate::mcp::capabilities::Capability::*;
    Some(match method {
        "list_environments" => EnvList,
        "list_resources" => ResourceList,
        "get_resource_yaml" | "describe_resource" => ResourceGet,
        "get_logs" => LogsRead,
        "apply_resource_yaml" => ResourceApply,
        "patch_resource" => ResourcePatch,
        "workload_action" => WorkloadLifecycle,
        "create_or_deploy" => ResourceCreate,
        "delete_resource" => ResourceDelete,
        "pod_exec" => PodExec,
        "pod_file_transfer" => PodFiles,
        _ => return None,
    })
}
