//! K8s 资源操作命令：describe、get、delete、apply、deploy、patch，以及 CRD 动态资源。

use super::super::kube_command_context::{self, err_str, CommandResult};
use crate::config::ResourceDeployStrategy;
use crate::kube::{
    apply_resource_yaml, build_graph, delete_dynamic_resource, delete_resource, deploy_resource_yaml,
    describe_dynamic_resource, describe_resource, get_dynamic_resource_yaml, get_pod_container_names,
    get_resource_yaml,
    patch_container_images, ContainerImagePatch, DescribeResult, KubeClientStore,
    ResourceGraph,
};
use crate::kube::resource_graph::registry::build_default_registry;
use tauri::State;

#[tauri::command]
pub async fn kube_describe_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> CommandResult<DescribeResult> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    describe_resource(&client, &kind, &name, namespace.as_deref())
        .await
        .map_err(err_str)
}

#[tauri::command]
pub async fn kube_get_resource_graph(
    store: State<'_, KubeClientStore>,
    env_id: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> CommandResult<ResourceGraph> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let registry = build_default_registry();
    build_graph(&client, &kind, &name, namespace.as_deref(), &registry, 3)
        .await
        .map_err(err_str)
}

#[tauri::command]
pub async fn kube_get_pod_containers(
    store: State<'_, KubeClientStore>,
    env_id: String,
    namespace: String,
    pod_name: String,
) -> CommandResult<Vec<String>> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    get_pod_container_names(&client, &namespace, &pod_name).await
}

#[tauri::command]
pub async fn kube_get_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> CommandResult<String> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    get_resource_yaml(&client, &kind, &name, namespace.as_deref())
        .await
        .map_err(err_str)
}

#[tauri::command]
pub async fn kube_delete_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> CommandResult<()> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    delete_resource(&client, &kind, &name, namespace.as_deref())
        .await
        .map_err(err_str)
}

#[tauri::command]
pub async fn kube_apply_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    yaml: String,
) -> CommandResult<()> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    apply_resource_yaml(&client, &yaml).await.map_err(err_str)
}

#[tauri::command]
pub async fn kube_deploy_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    yaml: String,
    strategy: String,
) -> CommandResult<()> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    let strategy = ResourceDeployStrategy::from_str(&strategy);
    deploy_resource_yaml(&client, &yaml, strategy)
        .await
        .map_err(err_str)
}

#[tauri::command]
pub async fn kube_patch_container_images(
    store: State<'_, KubeClientStore>,
    env_id: String,
    kind: String,
    name: String,
    namespace: Option<String>,
    patches: Vec<ContainerImagePatch>,
) -> CommandResult<()> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    patch_container_images(&client, &kind, &name, namespace.as_deref(), &patches)
        .await
        .map_err(err_str)
}

// ── CRD 动态资源操作 ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn kube_get_dynamic_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    api_version: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> CommandResult<String> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    get_dynamic_resource_yaml(&client, &api_version, &kind, &name, namespace.as_deref())
        .await
        .map_err(err_str)
}

#[tauri::command]
pub async fn kube_describe_dynamic_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    api_version: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> CommandResult<DescribeResult> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    describe_dynamic_resource(&client, &api_version, &kind, &name, namespace.as_deref())
        .await
        .map_err(err_str)
}

#[tauri::command]
pub async fn kube_delete_dynamic_resource(
    store: State<'_, KubeClientStore>,
    env_id: String,
    api_version: String,
    kind: String,
    name: String,
    namespace: Option<String>,
) -> CommandResult<()> {
    let (_env, client) = kube_command_context::kube_client_for_env_id(&store, &env_id).await?;
    delete_dynamic_resource(&client, &api_version, &kind, &name, namespace.as_deref())
        .await
        .map_err(err_str)
}
