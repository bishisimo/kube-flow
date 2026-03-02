//! 资源快捷 Patch：如批量修改容器镜像，使用 Strategic Merge Patch。

use crate::kube::resource_get;
use crate::kube::resources::ResourceError;
use kube::api::{Api, Patch, PatchParams};
use kube::Client;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, StatefulSet};
use serde::Deserialize;

const IMAGE_PATCH_KINDS: &[&str] = &["Deployment", "StatefulSet", "DaemonSet"];

/// 判断该 kind 是否支持 Change image 快捷操作。
pub fn supports_image_patch(kind: &str) -> bool {
    IMAGE_PATCH_KINDS.contains(&kind)
}

/// 容器镜像修改项。
#[derive(Debug, Deserialize)]
pub struct ContainerImagePatch {
    pub container_name: String,
    pub new_image: String,
}

/// 批量修改 workload 的容器镜像。
pub async fn patch_container_images(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
    patches: &[ContainerImagePatch],
) -> Result<(), ResourceError> {
    if !supports_image_patch(kind) {
        return Err(ResourceError::UnsupportedKind(kind.to_string()));
    }
    if patches.is_empty() {
        return Err(ResourceError::Serialize("patches cannot be empty".to_string()));
    }

    let yaml_str = resource_get::get_resource_yaml(client, kind, name, namespace).await?;
    let obj: serde_json::Value = serde_yaml::from_str(&yaml_str)
        .map_err(|e| ResourceError::Serialize(format!("yaml parse: {}", e)))?;

    let containers = obj
        .get("spec")
        .and_then(|s| s.get("template"))
        .and_then(|t| t.get("spec"))
        .and_then(|s| s.get("containers"))
        .and_then(|c| c.as_array())
        .ok_or_else(|| ResourceError::Serialize("missing spec.template.spec.containers".to_string()))?;

    let patch_containers: Vec<serde_json::Value> = patches
        .iter()
        .map(|p| {
            if !containers.iter().any(|c| {
                c.as_object()
                    .and_then(|o| o.get("name"))
                    .and_then(|v| v.as_str())
                    == Some(p.container_name.as_str())
            }) {
                return Err(ResourceError::Serialize(format!("container '{}' not found", p.container_name)));
            }
            Ok(serde_json::json!({
                "name": p.container_name,
                "image": p.new_image
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let patch_body = serde_json::json!({
        "spec": {
            "template": {
                "spec": {
                    "containers": patch_containers
                }
            }
        }
    });

    let ns = namespace.unwrap_or("default");
    let pp = PatchParams::default();

    match kind {
        "Deployment" => {
            let api: Api<Deployment> = Api::namespaced(client.clone(), ns);
            api.patch(name, &pp, &Patch::Strategic(patch_body))
                .await
                .map_err(ResourceError::Kube)?;
        }
        "StatefulSet" => {
            let api: Api<StatefulSet> = Api::namespaced(client.clone(), ns);
            api.patch(name, &pp, &Patch::Strategic(patch_body))
                .await
                .map_err(ResourceError::Kube)?;
        }
        "DaemonSet" => {
            let api: Api<DaemonSet> = Api::namespaced(client.clone(), ns);
            api.patch(name, &pp, &Patch::Strategic(patch_body))
                .await
                .map_err(ResourceError::Kube)?;
        }
        _ => return Err(ResourceError::UnsupportedKind(kind.to_string())),
    }

    Ok(())
}
