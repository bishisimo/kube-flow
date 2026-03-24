//! 关联资源跳转：根据源资源解析可跳转的目标（Pods、Service 等）。
//! 复用现有 label 筛选机制，跳转时带入 labelSelector。网络层以 Service 为主，Endpoints/EndpointSlice 为 Service 的底层实现。

use crate::kube::resource_get::get_resource_value;
use crate::kube::resources::{
    list_daemon_sets, list_deployments, list_pods_using_pvc, list_services_matching_pod_labels,
    list_persistent_volume_claims, list_services_matching_workload_selector, list_stateful_sets,
    ResourceError,
};
use kube::Client;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize)]
pub struct RelatedTarget {
    /// 目标资源类型（前端 selectedKind），如 pods、endpoints、endpointslices
    pub target_kind: String,
    /// 展示文案，如 "查看 Pods"、"查看 EndpointSlice"
    pub label: String,
    /// 目标 namespace，集群级资源为 None
    pub namespace: Option<String>,
    /// 用于 list 的 label selector 字符串
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_selector: Option<String>,
    /// 精确匹配的资源名，跳转后前端可预填 nameFilter 以聚焦
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_name: Option<String>,
}

/// 将简单 map（如 Service.spec.selector）转为 label selector 字符串。
pub(crate) fn simple_map_to_selector(map: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    if map.is_empty() {
        return None;
    }
    let parts: Vec<String> = map
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| format!("{}={}", k, s)))
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(","))
    }
}

/// 将 spec.selector（LabelSelector）转为 K8s API 接受的字符串。
/// 支持 matchLabels；matchExpressions 的 In/NotIn 可扩展。
pub(crate) fn selector_to_string(sel: &serde_json::Value) -> Option<String> {
    let obj = sel.as_object()?;
    let mut parts = Vec::new();

    if let Some(ml) = obj.get("matchLabels").and_then(|v| v.as_object()) {
        for (k, v) in ml {
            let val = v.as_str().unwrap_or("");
            parts.push(format!("{}={}", k, val));
        }
    }

    if let Some(me) = obj.get("matchExpressions").and_then(|v| v.as_array()) {
        for expr in me {
            let _ = expr.as_object()?;
            let key = expr.get("key").and_then(|v| v.as_str())?;
            let op = expr.get("operator").and_then(|v| v.as_str()).unwrap_or("");
            let vals = expr.get("values").and_then(|v| v.as_array());
            match op {
                "In" => {
                    if let Some(arr) = vals {
                        let vstr: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                        if !vstr.is_empty() {
                            parts.push(format!("{} in ({})", key, vstr.join(",")));
                        }
                    }
                }
                "NotIn" => {
                    if let Some(arr) = vals {
                        let vstr: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                        if !vstr.is_empty() {
                            parts.push(format!("{} notin ({})", key, vstr.join(",")));
                        }
                    }
                }
                "Exists" => {
                    parts.push(key.to_string());
                }
                "DoesNotExist" => {
                    parts.push(format!("!{}", key));
                }
                _ => {}
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(","))
    }
}

/// 检查 workload（Deployment/StatefulSet/DaemonSet）的 pod template 是否引用指定 PVC。
fn workload_uses_pvc(workload: &serde_json::Value, pvc_name: &str) -> bool {
    let template = workload.get("spec").and_then(|v| v.get("template")).and_then(|v| v.as_object());
    let Some(template) = template else { return false };
    let pod_spec = template.get("spec").and_then(|v| v.as_object());
    let Some(pod_spec) = pod_spec else { return false };
    let vols = pod_spec.get("volumes").and_then(|v| v.as_array());
    let Some(vols) = vols else { return false };
    vols.iter().any(|v| {
        v.get("persistentVolumeClaim")
            .and_then(|x| x.get("claimName"))
            .and_then(|x| x.as_str())
            .map(|s| s == pvc_name)
            .unwrap_or(false)
    })
}

fn pod_pvc_names(pod: &serde_json::Value) -> Vec<String> {
    let volumes = pod
        .get("spec")
        .and_then(|v| v.get("volumes"))
        .and_then(|v| v.as_array());
    let Some(volumes) = volumes else { return Vec::new() };

    let mut names = BTreeSet::new();
    for volume in volumes {
        if let Some(name) = volume
            .get("persistentVolumeClaim")
            .and_then(|x| x.get("claimName"))
            .and_then(|x| x.as_str())
        {
            if !name.is_empty() {
                names.insert(name.to_string());
            }
        }
    }
    names.into_iter().collect()
}

fn workload_pvc_names(workload: &serde_json::Value) -> Vec<String> {
    let template = workload
        .get("spec")
        .and_then(|v| v.get("template"))
        .and_then(|v| v.get("spec"))
        .and_then(|v| v.get("volumes"))
        .and_then(|v| v.as_array());
    let Some(template) = template else { return Vec::new() };

    let mut names = BTreeSet::new();
    for volume in template {
        if let Some(name) = volume
            .get("persistentVolumeClaim")
            .and_then(|x| x.get("claimName"))
            .and_then(|x| x.as_str())
        {
            if !name.is_empty() {
                names.insert(name.to_string());
            }
        }
    }
    names.into_iter().collect()
}

fn statefulset_volume_claim_template_names(workload: &serde_json::Value) -> Vec<String> {
    let templates = workload
        .get("spec")
        .and_then(|v| v.get("volumeClaimTemplates"))
        .and_then(|v| v.as_array());
    let Some(templates) = templates else { return Vec::new() };

    let mut names = BTreeSet::new();
    for template in templates {
        if let Some(name) = template
            .get("metadata")
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
        {
            if !name.is_empty() {
                names.insert(name.to_string());
            }
        }
    }
    names.into_iter().collect()
}

async fn statefulset_generated_pvc_names(
    client: &Client,
    namespace: &str,
    statefulset_name: &str,
    template_names: &[String],
) -> Result<Vec<String>, ResourceError> {
    if template_names.is_empty() {
        return Ok(Vec::new());
    }

    let pvcs = list_persistent_volume_claims(client, Some(namespace), None).await?;
    let mut names = BTreeSet::new();
    for pvc in pvcs {
        if template_names
            .iter()
            .any(|template| pvc.name.starts_with(&format!("{template}-{statefulset_name}-")))
        {
            names.insert(pvc.name);
        }
    }
    Ok(names.into_iter().collect())
}

/// 获取指定资源的可跳转关联目标列表。
pub async fn get_related_targets(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<Vec<RelatedTarget>, ResourceError> {
    let obj = get_resource_value(client, kind, name, namespace).await?;
    let ns = namespace
        .map(String::from)
        .or_else(|| obj.get("metadata").and_then(|m| m.get("namespace")).and_then(|v| v.as_str()).map(String::from));

    let mut targets = Vec::new();

    match kind {
        "Service" => {
            let spec = obj.get("spec").and_then(|v| v.as_object());
            if let Some(spec) = spec {
                if let Some(sel) = spec.get("selector").and_then(|v| v.as_object()) {
                    if let Some(ls) = simple_map_to_selector(sel) {
                        targets.push(RelatedTarget {
                            target_kind: "pods".to_string(),
                            label: "查看 Pods".to_string(),
                            namespace: ns.clone(),
                            label_selector: Some(ls),
                            resource_name: None,
                        });
                    }
                }
            }
        }
        "Pod" => {
            let labels = obj
                .get("metadata")
                .and_then(|m| m.get("labels"))
                .and_then(|v| v.as_object());
            if let (Some(labels), Some(ns_ref)) = (labels, ns.as_deref()) {
                if let Ok(svc_names) = list_services_matching_pod_labels(client, ns_ref, labels).await {
                    for svc_name in svc_names {
                        targets.push(RelatedTarget {
                            target_kind: "services".to_string(),
                            label: format!("查看 Service {}", svc_name),
                            namespace: ns.clone(),
                            label_selector: None,
                            resource_name: Some(svc_name),
                        });
                    }
                }
            }
            for pvc_name in pod_pvc_names(&obj) {
                targets.push(RelatedTarget {
                    target_kind: "persistentvolumeclaims".to_string(),
                    label: format!("查看 PVC {}", pvc_name),
                    namespace: ns.clone(),
                    label_selector: None,
                    resource_name: Some(pvc_name),
                });
            }
        }
        "Deployment" | "StatefulSet" | "DaemonSet" => {
            let spec = obj.get("spec").and_then(|v| v.as_object());
            if let Some(spec) = spec {
                if let Some(sel) = spec.get("selector") {
                    if let Some(ls) = selector_to_string(sel) {
                        targets.push(RelatedTarget {
                            target_kind: "pods".to_string(),
                            label: "查看 Pods".to_string(),
                            namespace: ns.clone(),
                            label_selector: Some(ls),
                            resource_name: None,
                        });
                    }
                    let ns_ref = ns.as_deref().unwrap_or("default");
                    if let Ok(svc_names) =
                        list_services_matching_workload_selector(client, ns_ref, sel).await
                    {
                        for svc_name in svc_names {
                            targets.push(RelatedTarget {
                                target_kind: "services".to_string(),
                                label: format!("查看 Service {}", svc_name),
                                namespace: ns.clone(),
                                label_selector: None,
                                resource_name: Some(svc_name),
                            });
                        }
                    }
                }
            }
            for pvc_name in workload_pvc_names(&obj) {
                targets.push(RelatedTarget {
                    target_kind: "persistentvolumeclaims".to_string(),
                    label: format!("查看 PVC {}", pvc_name),
                    namespace: ns.clone(),
                    label_selector: None,
                    resource_name: Some(pvc_name),
                });
            }
            if kind == "StatefulSet" {
                let ns_ref = ns.as_deref().unwrap_or("default");
                let template_names = statefulset_volume_claim_template_names(&obj);
                if let Ok(pvc_names) =
                    statefulset_generated_pvc_names(client, ns_ref, name, &template_names).await
                {
                    for pvc_name in pvc_names {
                        targets.push(RelatedTarget {
                            target_kind: "persistentvolumeclaims".to_string(),
                            label: format!("查看 PVC {}", pvc_name),
                            namespace: ns.clone(),
                            label_selector: None,
                            resource_name: Some(pvc_name),
                        });
                    }
                }
            }
        }
        "RoleBinding" => {
            if let Some(rr) = obj.get("roleRef").and_then(|v| v.as_object()) {
                let kind = rr.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                let name = rr.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if !name.is_empty() {
                    if kind == "Role" {
                        targets.push(RelatedTarget {
                            target_kind: "roles".to_string(),
                            label: format!("查看 Role {}", name),
                            namespace: ns.clone(),
                            label_selector: None,
                            resource_name: Some(name.to_string()),
                        });
                    } else if kind == "ClusterRole" {
                        targets.push(RelatedTarget {
                            target_kind: "clusterroles".to_string(),
                            label: format!("查看 ClusterRole {}", name),
                            namespace: None,
                            label_selector: None,
                            resource_name: Some(name.to_string()),
                        });
                    }
                }
            }
            if let Some(subs) = obj.get("subjects").and_then(|v| v.as_array()) {
                for s in subs {
                    let Some(sobj) = s.as_object() else { continue };
                    let skind = sobj.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                    if skind != "ServiceAccount" {
                        continue;
                    }
                    let sname = sobj.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let sns = sobj.get("namespace").and_then(|v| v.as_str()).map(String::from);
                    if sname.is_empty() {
                        continue;
                    }
                    let label_ns = sns.as_deref().unwrap_or(ns.as_deref().unwrap_or("default"));
                    targets.push(RelatedTarget {
                        target_kind: "serviceaccounts".to_string(),
                        label: format!("查看 ServiceAccount {}/{}", label_ns, sname),
                        namespace: sns.or_else(|| ns.clone()),
                        label_selector: None,
                        resource_name: Some(sname.to_string()),
                    });
                }
            }
        }
        "ClusterRoleBinding" => {
            if let Some(rr) = obj.get("roleRef").and_then(|v| v.as_object()) {
                let kind = rr.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                let name = rr.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if !name.is_empty() && (kind == "ClusterRole" || kind.is_empty()) {
                    targets.push(RelatedTarget {
                        target_kind: "clusterroles".to_string(),
                        label: format!("查看 ClusterRole {}", name),
                        namespace: None,
                        label_selector: None,
                        resource_name: Some(name.to_string()),
                    });
                }
            }
            if let Some(subs) = obj.get("subjects").and_then(|v| v.as_array()) {
                for s in subs {
                    let Some(sobj) = s.as_object() else { continue };
                    let skind = sobj.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                    if skind != "ServiceAccount" {
                        continue;
                    }
                    let sname = sobj.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let sns = sobj.get("namespace").and_then(|v| v.as_str()).map(String::from);
                    if sname.is_empty() {
                        continue;
                    }
                    let label_ns = sns.as_deref().unwrap_or("default");
                    targets.push(RelatedTarget {
                        target_kind: "serviceaccounts".to_string(),
                        label: format!("查看 ServiceAccount {}/{}", label_ns, sname),
                        namespace: sns,
                        label_selector: None,
                        resource_name: Some(sname.to_string()),
                    });
                }
            }
        }
        "PersistentVolumeClaim" => {
            let spec = obj.get("spec").and_then(|v| v.as_object());
            if let Some(spec) = spec {
                if let Some(pv) = spec.get("volumeName").and_then(|v| v.as_str()) {
                    if !pv.is_empty() {
                        targets.push(RelatedTarget {
                            target_kind: "persistentvolumes".to_string(),
                            label: "查看 PersistentVolume".to_string(),
                            namespace: None,
                            label_selector: None,
                            resource_name: Some(pv.to_string()),
                        });
                    }
                }
                if let Some(sc) = spec.get("storageClassName").and_then(|v| v.as_str()) {
                    if !sc.is_empty() {
                        targets.push(RelatedTarget {
                            target_kind: "storageclasses".to_string(),
                            label: "查看 StorageClass".to_string(),
                            namespace: None,
                            label_selector: None,
                            resource_name: Some(sc.to_string()),
                        });
                    }
                }
            }
            let ns_ref = ns.as_deref().unwrap_or("default");
            let deps = list_deployments(client, Some(ns_ref), None).await?;
            for d in deps {
                let d_obj = get_resource_value(
                    client,
                    "Deployment",
                    &d.name,
                    Some(&d.namespace),
                )
                .await
                .ok();
                if let Some(vo) = d_obj {
                    if workload_uses_pvc(&vo, name) {
                        targets.push(RelatedTarget {
                            target_kind: "deployments".to_string(),
                            label: format!("查看 Deployment {}", d.name),
                            namespace: ns.clone(),
                            label_selector: None,
                            resource_name: Some(d.name),
                        });
                    }
                }
            }
            let stss = list_stateful_sets(client, Some(ns_ref), None).await?;
            for s in stss {
                let s_obj = get_resource_value(
                    client,
                    "StatefulSet",
                    &s.name,
                    Some(&s.namespace),
                )
                .await
                .ok();
                if let Some(vo) = s_obj {
                    if workload_uses_pvc(&vo, name) {
                        targets.push(RelatedTarget {
                            target_kind: "statefulsets".to_string(),
                            label: format!("查看 StatefulSet {}", s.name),
                            namespace: ns.clone(),
                            label_selector: None,
                            resource_name: Some(s.name.clone()),
                        });
                    }
                    if let Ok(pvc_names) = statefulset_generated_pvc_names(
                        client,
                        &s.namespace,
                        &s.name,
                        &statefulset_volume_claim_template_names(&vo),
                    )
                    .await
                    {
                        if pvc_names.iter().any(|pvc_name| pvc_name == name) {
                            targets.push(RelatedTarget {
                                target_kind: "statefulsets".to_string(),
                                label: format!("查看 StatefulSet {}", s.name),
                                namespace: ns.clone(),
                                label_selector: None,
                                resource_name: Some(s.name.clone()),
                            });
                        }
                    }
                }
            }
            let dss = list_daemon_sets(client, Some(ns_ref), None).await?;
            for d in dss {
                let d_obj = get_resource_value(
                    client,
                    "DaemonSet",
                    &d.name,
                    Some(&d.namespace),
                )
                .await
                .ok();
                if let Some(vo) = d_obj {
                    if workload_uses_pvc(&vo, name) {
                        targets.push(RelatedTarget {
                            target_kind: "daemonsets".to_string(),
                            label: format!("查看 DaemonSet {}", d.name),
                            namespace: ns.clone(),
                            label_selector: None,
                            resource_name: Some(d.name),
                        });
                    }
                }
            }
            if let Ok(pods) = list_pods_using_pvc(client, ns_ref, name).await {
                for p in pods {
                    targets.push(RelatedTarget {
                        target_kind: "pods".to_string(),
                        label: format!("查看 Pod {}", p.name),
                        namespace: ns.clone(),
                        label_selector: None,
                        resource_name: Some(p.name),
                    });
                }
            }
        }
        _ => {}
    }

    Ok(targets)
}
