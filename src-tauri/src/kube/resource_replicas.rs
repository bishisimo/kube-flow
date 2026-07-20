//! Workload 副本生命周期：停止 / 恢复（annotation 保存副本数）与滚动重启。

use crate::kube::resources::{
    ns_or_default, saved_replicas_from_annotations, ResourceError, DESIRED_REPLICAS_ANNOTATION,
};
use chrono::Utc;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, StatefulSet};
use kube::api::{Api, Patch, PatchParams};
use kube::Client;
use serde_json::{json, Map, Value};

/// 滚动重启写入 Pod 模板的 annotation（与 kubectl rollout restart 一致）。
const RESTARTED_AT_ANNOTATION: &str = "kubectl.kubernetes.io/restartedAt";

const STOP_RESUME_KINDS: &[&str] = &["Deployment", "StatefulSet"];
const RESTART_KINDS: &[&str] = &["Deployment", "StatefulSet", "DaemonSet"];

fn supports_stop_resume(kind: &str) -> bool {
    STOP_RESUME_KINDS.contains(&kind)
}

fn supports_restart(kind: &str) -> bool {
    RESTART_KINDS.contains(&kind)
}

/// 构建 metadata.annotations 对象；key 必须用字符串写入，不能直接把常量标识符放进 `json!`。
fn annotations_object(pairs: &[(&str, Value)]) -> Map<String, Value> {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    m
}

/// 停止可伸缩 workload：将当前副本数写入 annotation，再 scale 到 0。
pub async fn stop_workload(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<(), ResourceError> {
    if !supports_stop_resume(kind) {
        return Err(ResourceError::UnsupportedKind(kind.to_string()));
    }
    let ns = ns_or_default(namespace);
    match kind {
        "Deployment" => stop_deployment(client, name, ns).await,
        "StatefulSet" => stop_stateful_set(client, name, ns).await,
        _ => Err(ResourceError::UnsupportedKind(kind.to_string())),
    }
}

/// 恢复可伸缩 workload：从 annotation 还原副本数并清除 annotation。
pub async fn resume_workload(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<(), ResourceError> {
    if !supports_stop_resume(kind) {
        return Err(ResourceError::UnsupportedKind(kind.to_string()));
    }
    let ns = ns_or_default(namespace);
    match kind {
        "Deployment" => resume_deployment(client, name, ns).await,
        "StatefulSet" => resume_stateful_set(client, name, ns).await,
        _ => Err(ResourceError::UnsupportedKind(kind.to_string())),
    }
}

/// 滚动重启 workload：更新 Pod 模板 restartedAt annotation。
pub async fn restart_workload(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<(), ResourceError> {
    if !supports_restart(kind) {
        return Err(ResourceError::UnsupportedKind(kind.to_string()));
    }
    let ns = ns_or_default(namespace);
    let restarted_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    // Merge patch：可靠合并 template.metadata.annotations（与 kubectl rollout restart 等效）
    let patch_body = json!({
        "spec": {
            "template": {
                "metadata": {
                    "annotations": annotations_object(&[(
                        RESTARTED_AT_ANNOTATION,
                        Value::String(restarted_at),
                    )])
                }
            }
        }
    });
    let pp = PatchParams::default();
    match kind {
        "Deployment" => {
            let api: Api<Deployment> = Api::namespaced(client.clone(), ns);
            api.patch(name, &pp, &Patch::Merge(patch_body))
                .await
                .map_err(ResourceError::Kube)?;
        }
        "StatefulSet" => {
            let api: Api<StatefulSet> = Api::namespaced(client.clone(), ns);
            api.patch(name, &pp, &Patch::Merge(patch_body))
                .await
                .map_err(ResourceError::Kube)?;
        }
        "DaemonSet" => {
            let api: Api<DaemonSet> = Api::namespaced(client.clone(), ns);
            api.patch(name, &pp, &Patch::Merge(patch_body))
                .await
                .map_err(ResourceError::Kube)?;
        }
        _ => return Err(ResourceError::UnsupportedKind(kind.to_string())),
    }
    Ok(())
}

async fn stop_deployment(client: &Client, name: &str, ns: &str) -> Result<(), ResourceError> {
    let api: Api<Deployment> = Api::namespaced(client.clone(), ns);
    let obj = api.get(name).await.map_err(ResourceError::Kube)?;
    let current = obj.spec.as_ref().and_then(|s| s.replicas).unwrap_or(1);
    let saved = saved_replicas_from_annotations(obj.metadata.annotations.as_ref());
    if current == 0 {
        if saved.is_some() {
            return Ok(());
        }
        return Err(ResourceError::Serialize(
            "当前副本数为 0 且无已保存副本数，无法停止（请先手动设置副本数）".to_string(),
        ));
    }
    let to_save = current;
    let patch_body = json!({
        "metadata": {
            "annotations": annotations_object(&[(
                DESIRED_REPLICAS_ANNOTATION,
                Value::String(to_save.to_string()),
            )])
        },
        "spec": {
            "replicas": 0
        }
    });
    api.patch(name, &PatchParams::default(), &Patch::Merge(patch_body))
        .await
        .map_err(ResourceError::Kube)?;
    Ok(())
}

async fn stop_stateful_set(client: &Client, name: &str, ns: &str) -> Result<(), ResourceError> {
    let api: Api<StatefulSet> = Api::namespaced(client.clone(), ns);
    let obj = api.get(name).await.map_err(ResourceError::Kube)?;
    let current = obj.spec.as_ref().and_then(|s| s.replicas).unwrap_or(1);
    let saved = saved_replicas_from_annotations(obj.metadata.annotations.as_ref());
    if current == 0 {
        if saved.is_some() {
            return Ok(());
        }
        return Err(ResourceError::Serialize(
            "当前副本数为 0 且无已保存副本数，无法停止（请先手动设置副本数）".to_string(),
        ));
    }
    let to_save = current;
    let patch_body = json!({
        "metadata": {
            "annotations": annotations_object(&[(
                DESIRED_REPLICAS_ANNOTATION,
                Value::String(to_save.to_string()),
            )])
        },
        "spec": {
            "replicas": 0
        }
    });
    api.patch(name, &PatchParams::default(), &Patch::Merge(patch_body))
        .await
        .map_err(ResourceError::Kube)?;
    Ok(())
}

async fn resume_deployment(client: &Client, name: &str, ns: &str) -> Result<(), ResourceError> {
    let api: Api<Deployment> = Api::namespaced(client.clone(), ns);
    let obj = api.get(name).await.map_err(ResourceError::Kube)?;
    let saved = saved_replicas_from_annotations(obj.metadata.annotations.as_ref()).ok_or_else(
        || {
            ResourceError::Serialize(
                "无可恢复副本数（缺少 kube-flow.io/desired-replicas）".to_string(),
            )
        },
    )?;
    if saved <= 0 {
        return Err(ResourceError::Serialize(
            "已保存副本数无效，无法恢复".to_string(),
        ));
    }
    let patch_body = json!({
        "metadata": {
            "annotations": annotations_object(&[(DESIRED_REPLICAS_ANNOTATION, Value::Null)])
        },
        "spec": {
            "replicas": saved
        }
    });
    api.patch(name, &PatchParams::default(), &Patch::Merge(patch_body))
        .await
        .map_err(ResourceError::Kube)?;
    Ok(())
}

async fn resume_stateful_set(client: &Client, name: &str, ns: &str) -> Result<(), ResourceError> {
    let api: Api<StatefulSet> = Api::namespaced(client.clone(), ns);
    let obj = api.get(name).await.map_err(ResourceError::Kube)?;
    let saved = saved_replicas_from_annotations(obj.metadata.annotations.as_ref()).ok_or_else(
        || {
            ResourceError::Serialize(
                "无可恢复副本数（缺少 kube-flow.io/desired-replicas）".to_string(),
            )
        },
    )?;
    if saved <= 0 {
        return Err(ResourceError::Serialize(
            "已保存副本数无效，无法恢复".to_string(),
        ));
    }
    let patch_body = json!({
        "metadata": {
            "annotations": annotations_object(&[(DESIRED_REPLICAS_ANNOTATION, Value::Null)])
        },
        "spec": {
            "replicas": saved
        }
    });
    api.patch(name, &PatchParams::default(), &Patch::Merge(patch_body))
        .await
        .map_err(ResourceError::Kube)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotations_object_uses_real_key_not_ident_name() {
        let m = annotations_object(&[(
            RESTARTED_AT_ANNOTATION,
            Value::String("2026-07-17T12:00:00Z".into()),
        )]);
        assert!(m.contains_key("kubectl.kubernetes.io/restartedAt"));
        assert!(!m.contains_key("RESTARTED_AT_ANNOTATION"));
    }
}
