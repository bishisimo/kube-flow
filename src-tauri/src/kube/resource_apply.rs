//! 资源下发：支持工作台的 replace，以及编排中心的 create+replace / apply。

use crate::config::ResourceDeployStrategy;
use crate::kube::resources::ResourceError;
use kube::api::{Api, DynamicObject, Patch, PatchParams, PostParams};
use kube::core::{GroupVersion, GroupVersionKind};
use kube::discovery::{self, ApiCapabilities, ApiResource, Scope};
use kube::Client;

fn sanitize_apply_value(mut obj: serde_json::Value) -> serde_json::Value {
    if let Some(root) = obj.as_object_mut() {
        if let Some(meta) = root.get_mut("metadata").and_then(|v| v.as_object_mut()) {
            meta.remove("managedFields");
            meta.remove("resourceVersion");
            meta.remove("uid");
            meta.remove("creationTimestamp");
            meta.remove("generation");
        }
        root.remove("status");
    }
    obj
}

fn parse_resource_identity(
    yaml: &str,
) -> Result<(serde_json::Value, GroupVersionKind, String, Option<String>), ResourceError> {
    let obj: serde_json::Value = serde_yaml::from_str(yaml)
        .map_err(|e| ResourceError::Serialize(format!("yaml parse: {}", e)))?;
    let api_version = obj
        .get("apiVersion")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ResourceError::Serialize("missing apiVersion".to_string()))?;
    let kind = obj
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ResourceError::Serialize("missing kind".to_string()))?;

    let meta = obj
        .get("metadata")
        .and_then(|v| v.as_object())
        .ok_or_else(|| ResourceError::Serialize("missing metadata".to_string()))?;

    let name = meta
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ResourceError::Serialize("missing metadata.name".to_string()))?
        .to_string();

    let namespace = meta
        .get("namespace")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let gv = api_version
        .parse::<GroupVersion>()
        .map_err(|e| ResourceError::Serialize(format!("invalid apiVersion: {}", e)))?;
    let gvk = gv.with_kind(kind);

    Ok((obj, gvk, name, namespace))
}

async fn resolve_api(
    client: &Client,
    gvk: &GroupVersionKind,
    namespace: Option<&str>,
) -> Result<(Api<DynamicObject>, ApiResource), ResourceError> {
    let (ar, caps): (ApiResource, ApiCapabilities) = discovery::pinned_kind(client, gvk)
        .await
        .map_err(|e| {
            ResourceError::Serialize(format!(
                "无法解析资源类型 {}/{} {}：{}",
                gvk.group,
                gvk.version,
                gvk.kind,
                e
            ))
        })?;

    let api = match caps.scope {
        Scope::Cluster => Api::<DynamicObject>::all_with(client.clone(), &ar),
        Scope::Namespaced => Api::<DynamicObject>::namespaced_with(
            client.clone(),
            namespace.unwrap_or("default"),
            &ar,
        ),
    };
    Ok((api, ar))
}

async fn replace_existing_resource(
    api: &Api<DynamicObject>,
    name: &str,
    mut desired: DynamicObject,
) -> Result<(), ResourceError> {
    let existing = api.get(name).await.map_err(ResourceError::Kube)?;
    desired.metadata.resource_version = existing.metadata.resource_version.clone();
    api.replace(name, &PostParams::default(), &desired)
        .await
        .map_err(ResourceError::Kube)?;
    Ok(())
}

async fn create_or_replace_resource(
    api: &Api<DynamicObject>,
    name: &str,
    desired: DynamicObject,
) -> Result<(), ResourceError> {
    if api.get_opt(name).await.map_err(ResourceError::Kube)?.is_some() {
        replace_existing_resource(api, name, desired).await
    } else {
        api.create(&PostParams::default(), &desired)
            .await
            .map_err(ResourceError::Kube)?;
        Ok(())
    }
}

async fn server_side_apply_resource(
    api: &Api<DynamicObject>,
    name: &str,
    desired: &DynamicObject,
) -> Result<(), ResourceError> {
    let pp = PatchParams::apply("kube-flow").force();
    api.patch(name, &pp, &Patch::Apply(desired))
        .await
        .map_err(ResourceError::Kube)?;
    Ok(())
}

fn build_dynamic_object(obj: serde_json::Value) -> Result<DynamicObject, ResourceError> {
    serde_json::from_value(obj).map_err(|e| ResourceError::Serialize(e.to_string()))
}

/// 工作台资源编辑：要求对象已存在，按 replace 语义覆盖。
pub async fn apply_resource_yaml(client: &Client, yaml: &str) -> Result<(), ResourceError> {
    let (obj, gvk, name, namespace) = parse_resource_identity(yaml)?;
    let obj = sanitize_apply_value(obj);
    let desired = build_dynamic_object(obj)?;
    let (api, _) = resolve_api(client, &gvk, namespace.as_deref()).await?;
    replace_existing_resource(&api, &name, desired).await
}

/// 编排中心资源下发：按策略使用 create+replace 或 server-side apply。
pub async fn deploy_resource_yaml(
    client: &Client,
    yaml: &str,
    strategy: ResourceDeployStrategy,
) -> Result<(), ResourceError> {
    let (obj, gvk, name, namespace) = parse_resource_identity(yaml)?;
    let obj = sanitize_apply_value(obj);
    let desired = build_dynamic_object(obj)?;
    let (api, _) = resolve_api(client, &gvk, namespace.as_deref()).await?;

    match strategy {
        ResourceDeployStrategy::CreateReplace => create_or_replace_resource(&api, &name, desired).await,
        ResourceDeployStrategy::Apply => server_side_apply_resource(&api, &name, &desired).await,
    }
}
