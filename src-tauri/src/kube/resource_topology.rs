//! 资源拓扑：以当前资源为中心，返回上游（引用）与下游（管理）关联，供钻取页面展示。

use crate::kube::related_targets::{selector_to_string, simple_map_to_selector};
use crate::kube::resource_get::get_resource_value;
use crate::kube::resources::{
    list_daemon_sets, list_deployments, list_persistent_volume_claims, list_pods,
    list_pods_using_pvc, list_services_matching_workload_selector, list_stateful_sets,
    ResourceError,
};
use kube::Client;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize)]
pub struct TopologyItem {
    /// API Kind，如 Pod、ConfigMap
    pub kind: String,
    /// 资源名称；聚合视图时可为空
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// 前端 kind id，如 pods、configmaps
    pub target_kind: String,
    /// 展示文案
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_selector: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_name: Option<String>,
    /// true = 具体资源，false = 聚合/列表视图
    pub is_concrete: bool,
    /// 关系类型：volumes、envFrom、selector、owner、runs-on、service-name、roleRef 等
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceTopology {
    pub upstream: Vec<TopologyItem>,
    pub downstream: Vec<TopologyItem>,
}

fn kind_to_target_id(kind: &str) -> &'static str {
    match kind {
        "Pod" => "pods",
        "Deployment" => "deployments",
        "ReplicaSet" => "replicasets",
        "StatefulSet" => "statefulsets",
        "DaemonSet" => "daemonsets",
        "ConfigMap" => "configmaps",
        "Secret" => "secrets",
        "Service" => "services",
        "Endpoints" => "endpoints",
        "EndpointSlice" => "endpointslices",
        "PersistentVolume" => "persistentvolumes",
        "PersistentVolumeClaim" => "persistentvolumeclaims",
        "StorageClass" => "storageclasses",
        "Role" => "roles",
        "ClusterRole" => "clusterroles",
        "ServiceAccount" => "serviceaccounts",
        "Node" => "nodes",
        _ => "pods",
    }
}

/// 从 workload spec 解析引用的 ConfigMap/Secret，返回 (kind, name, relation_type)
fn refs_from_workload_spec(spec: &serde_json::Value) -> Vec<(String, String, &'static str)> {
    let mut refs = Vec::new();
    let template = spec.get("template").and_then(|v| v.as_object());
    let Some(template) = template else { return refs };
    let pod_spec = template.get("spec").and_then(|v| v.as_object());
    let Some(pod_spec) = pod_spec else { return refs };

    if let Some(vols) = pod_spec.get("volumes").and_then(|v| v.as_array()) {
        for v in vols {
            let obj = match v.as_object() {
                Some(o) => o,
                None => continue,
            };
            if let Some(cm) = obj.get("configMap").and_then(|x| x.get("name")).and_then(|x| x.as_str()) {
                if !cm.is_empty() {
                    refs.push(("ConfigMap".to_string(), cm.to_string(), "volumes"));
                }
            }
            if let Some(sec) = obj.get("secret").and_then(|x| x.get("secretName")).and_then(|x| x.as_str()) {
                if !sec.is_empty() {
                    refs.push(("Secret".to_string(), sec.to_string(), "volumes"));
                }
            }
            if let Some(pvc) = obj.get("persistentVolumeClaim").and_then(|x| x.get("claimName")).and_then(|x| x.as_str()) {
                if !pvc.is_empty() {
                    refs.push(("PersistentVolumeClaim".to_string(), pvc.to_string(), "volumes"));
                }
            }
        }
    }
    if let Some(containers) = pod_spec.get("containers").and_then(|v| v.as_array()) {
        for c in containers {
            let obj = match c.as_object() {
                Some(o) => o,
                None => continue,
            };
            if let Some(env_from) = obj.get("envFrom").and_then(|v| v.as_array()) {
                for e in env_from {
                    let eo = match e.as_object() {
                        Some(o) => o,
                        None => continue,
                    };
                    if let Some(cm) = eo.get("configMapRef").and_then(|x| x.get("name")).and_then(|x| x.as_str()) {
                        if !cm.is_empty() {
                            refs.push(("ConfigMap".to_string(), cm.to_string(), "envFrom"));
                        }
                    }
                    if let Some(sec) = eo.get("secretRef").and_then(|x| x.get("name")).and_then(|x| x.as_str()) {
                        if !sec.is_empty() {
                            refs.push(("Secret".to_string(), sec.to_string(), "envFrom"));
                        }
                    }
                }
            }
        }
    }
    refs
}

fn refs_from_pod_spec(spec: &serde_json::Value) -> Vec<(String, String, &'static str)> {
    let mut refs = Vec::new();
    let pod_spec = match spec.as_object() {
        Some(obj) => obj,
        None => return refs,
    };

    if let Some(vols) = pod_spec.get("volumes").and_then(|v| v.as_array()) {
        for v in vols {
            let obj = match v.as_object() {
                Some(o) => o,
                None => continue,
            };
            if let Some(cm) = obj.get("configMap").and_then(|x| x.get("name")).and_then(|x| x.as_str()) {
                if !cm.is_empty() {
                    refs.push(("ConfigMap".to_string(), cm.to_string(), "volumes"));
                }
            }
            if let Some(sec) = obj.get("secret").and_then(|x| x.get("secretName")).and_then(|x| x.as_str()) {
                if !sec.is_empty() {
                    refs.push(("Secret".to_string(), sec.to_string(), "volumes"));
                }
            }
            if let Some(pvc) = obj.get("persistentVolumeClaim").and_then(|x| x.get("claimName")).and_then(|x| x.as_str()) {
                if !pvc.is_empty() {
                    refs.push(("PersistentVolumeClaim".to_string(), pvc.to_string(), "volumes"));
                }
            }
        }
    }
    refs
}

fn statefulset_volume_claim_template_names(obj: &serde_json::Value) -> Vec<String> {
    let templates = obj
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

/// 若 workload 引用指定 ConfigMap/Secret，返回关系类型（volumes/envFrom）
fn workload_ref_relation_type(workload: &serde_json::Value, kind: &str, name: &str) -> Option<&'static str> {
    let spec = workload.get("spec")?;
    for (k, n, rt) in refs_from_workload_spec(spec) {
        if k == kind && n == name {
            return Some(rt);
        }
    }
    None
}

/// 获取资源拓扑：上游（引用）与下游（管理）
pub async fn get_resource_topology(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<ResourceTopology, ResourceError> {
    let obj = get_resource_value(client, kind, name, namespace).await?;
    let ns = namespace
        .map(String::from)
        .or_else(|| {
            obj.get("metadata")
                .and_then(|m| m.get("namespace"))
                .and_then(|v| v.as_str())
                .map(String::from)
        });

    let mut upstream = Vec::new();
    let mut downstream = Vec::new();

    match kind {
        "Deployment" | "StatefulSet" | "DaemonSet" => {
            let spec = obj.get("spec");
            if let Some(spec) = spec {
                for (ref_kind, ref_name, rt) in refs_from_workload_spec(spec) {
                    upstream.push(TopologyItem {
                        kind: ref_kind.clone(),
                        name: ref_name.clone(),
                        namespace: ns.clone(),
                        target_kind: kind_to_target_id(&ref_kind).to_string(),
                        label: ref_name.clone(),
                        label_selector: None,
                        resource_name: Some(ref_name.clone()),
                        is_concrete: true,
                        relation_type: Some(rt.to_string()),
                    });
                }
            }
            if kind == "StatefulSet" {
                let ns_ref = ns.as_deref().unwrap_or("default");
                let template_names = statefulset_volume_claim_template_names(&obj);
                if let Ok(pvc_names) =
                    statefulset_generated_pvc_names(client, ns_ref, name, &template_names).await
                {
                    for pvc_name in pvc_names {
                        upstream.push(TopologyItem {
                            kind: "PersistentVolumeClaim".to_string(),
                            name: pvc_name.clone(),
                            namespace: ns.clone(),
                            target_kind: "persistentvolumeclaims".to_string(),
                            label: pvc_name.clone(),
                            label_selector: None,
                            resource_name: Some(pvc_name),
                            is_concrete: true,
                            relation_type: Some("volumeClaimTemplates".to_string()),
                        });
                    }
                }
            }
            if let Some(spec) = obj.get("spec").and_then(|v| v.as_object()) {
                if let Some(sel) = spec.get("selector") {
                    if let Some(ls) = selector_to_string(sel) {
                        let pods = list_pods(client, ns.as_deref(), Some(&ls)).await?;
                        let count = pods.len();
                        downstream.push(TopologyItem {
                            kind: "Pod".to_string(),
                            name: String::new(),
                            namespace: ns.clone(),
                            target_kind: "pods".to_string(),
                            label: format!("Pods ({})", count),
                            label_selector: Some(ls),
                            resource_name: None,
                            is_concrete: false,
                            relation_type: Some("selector".to_string()),
                        });
                    }
                    let ns_ref = ns.as_deref().unwrap_or("default");
                    if let Ok(svc_names) =
                        list_services_matching_workload_selector(client, ns_ref, sel).await
                    {
                        for svc_name in svc_names {
                            downstream.push(TopologyItem {
                                kind: "Service".to_string(),
                                name: svc_name.clone(),
                                namespace: ns.clone(),
                                target_kind: "services".to_string(),
                                label: svc_name.clone(),
                                label_selector: None,
                                resource_name: Some(svc_name),
                                is_concrete: true,
                                relation_type: Some("selector".to_string()),
                            });
                        }
                    }
                }
            }
        }
        "Service" => {
            let spec = obj.get("spec").and_then(|v| v.as_object());
            if let Some(spec) = spec {
                if let Some(sel) = spec.get("selector").and_then(|v| v.as_object()) {
                    if let Some(ls) = simple_map_to_selector(sel) {
                        let pods = list_pods(client, ns.as_deref(), Some(&ls)).await?;
                        let count = pods.len();
                        downstream.push(TopologyItem {
                            kind: "Pod".to_string(),
                            name: String::new(),
                            namespace: ns.clone(),
                            target_kind: "pods".to_string(),
                            label: format!("Pods ({})", count),
                            label_selector: Some(ls),
                            resource_name: None,
                            is_concrete: false,
                            relation_type: Some("selector".to_string()),
                        });
                    }
                }
            }
        }
        "ConfigMap" | "Secret" => {
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
                    if let Some(rt) = workload_ref_relation_type(&vo, kind, name) {
                        upstream.push(TopologyItem {
                            kind: "Deployment".to_string(),
                            name: d.name.clone(),
                            namespace: Some(d.namespace.clone()),
                            target_kind: "deployments".to_string(),
                            label: d.name.clone(),
                            label_selector: None,
                            resource_name: Some(d.name.clone()),
                            is_concrete: true,
                            relation_type: Some(rt.to_string()),
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
                    if let Some(rt) = workload_ref_relation_type(&vo, kind, name) {
                        upstream.push(TopologyItem {
                            kind: "StatefulSet".to_string(),
                            name: s.name.clone(),
                            namespace: Some(s.namespace.clone()),
                            target_kind: "statefulsets".to_string(),
                            label: s.name.clone(),
                            label_selector: None,
                            resource_name: Some(s.name.clone()),
                            is_concrete: true,
                            relation_type: Some(rt.to_string()),
                        });
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
                    if let Some(rt) = workload_ref_relation_type(&vo, kind, name) {
                        upstream.push(TopologyItem {
                            kind: "DaemonSet".to_string(),
                            name: d.name.clone(),
                            namespace: Some(d.namespace.clone()),
                            target_kind: "daemonsets".to_string(),
                            label: d.name.clone(),
                            label_selector: None,
                            resource_name: Some(d.name.clone()),
                            is_concrete: true,
                            relation_type: Some(rt.to_string()),
                        });
                    }
                }
            }
        }
        "Pod" => {
            if let Some(spec) = obj.get("spec") {
                for (ref_kind, ref_name, rt) in refs_from_pod_spec(spec) {
                    upstream.push(TopologyItem {
                        kind: ref_kind.clone(),
                        name: ref_name.clone(),
                        namespace: ns.clone(),
                        target_kind: kind_to_target_id(&ref_kind).to_string(),
                        label: ref_name.clone(),
                        label_selector: None,
                        resource_name: Some(ref_name),
                        is_concrete: true,
                        relation_type: Some(rt.to_string()),
                    });
                }
            }
            if let Some(owners) = obj.get("metadata").and_then(|m| m.get("ownerReferences")).and_then(|v| v.as_array()) {
                for o in owners {
                    let o_kind = o.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                    let o_name = o.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    if o_name.is_empty() {
                        continue;
                    }
                    if o_kind == "ReplicaSet" {
                        if let Ok(rs_obj) = get_resource_value(
                            client,
                            "ReplicaSet",
                            o_name,
                            ns.as_deref(),
                        )
                        .await
                        {
                            if let Some(rs_owners) = rs_obj.get("metadata").and_then(|m| m.get("ownerReferences")).and_then(|v| v.as_array()) {
                                for ro in rs_owners {
                                    let ro_kind = ro.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                                    let ro_name = ro.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                    if ro_kind == "Deployment" && !ro_name.is_empty() {
                                        downstream.push(TopologyItem {
                                            kind: "Deployment".to_string(),
                                            name: ro_name.to_string(),
                                            namespace: ns.clone(),
                                            target_kind: "deployments".to_string(),
                                            label: format!("Deployment / {}", ro_name),
                                            label_selector: None,
                                            resource_name: Some(ro_name.to_string()),
                                            is_concrete: true,
                                            relation_type: Some("owner".to_string()),
                                        });
                                        break;
                                    }
                                }
                            }
                        }
                        continue;
                    }
                    let (target_kind, kind_label) = match o_kind {
                        "StatefulSet" => ("statefulsets", "StatefulSet"),
                        "DaemonSet" => ("daemonsets", "DaemonSet"),
                        "Job" => ("jobs", "Job"),
                        _ => continue,
                    };
                    downstream.push(TopologyItem {
                        kind: o_kind.to_string(),
                        name: o_name.to_string(),
                        namespace: ns.clone(),
                        target_kind: target_kind.to_string(),
                        label: format!("{} / {}", kind_label, o_name),
                        label_selector: None,
                        resource_name: Some(o_name.to_string()),
                        is_concrete: true,
                        relation_type: Some("owner".to_string()),
                    });
                }
            }
            if let Some(node) = obj.get("spec").and_then(|s| s.get("nodeName")).and_then(|v| v.as_str()) {
                if !node.is_empty() {
                    downstream.push(TopologyItem {
                        kind: "Node".to_string(),
                        name: node.to_string(),
                        namespace: None,
                        target_kind: "nodes".to_string(),
                        label: format!("Node / {}", node),
                        label_selector: None,
                        resource_name: Some(node.to_string()),
                        is_concrete: true,
                        relation_type: Some("runs-on".to_string()),
                    });
                }
            }
        }
        "RoleBinding" => {
            if let Some(rr) = obj.get("roleRef").and_then(|v| v.as_object()) {
                let rkind = rr.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                let rname = rr.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if !rname.is_empty() {
                    let (target_kind, _) = if rkind == "Role" {
                        ("roles", ns.clone())
                    } else {
                        ("clusterroles", None)
                    };
                    upstream.push(TopologyItem {
                        kind: rkind.to_string(),
                        name: rname.to_string(),
                        namespace: if rkind == "Role" { ns.clone() } else { None },
                        target_kind: target_kind.to_string(),
                        label: format!("{} / {}", rkind, rname),
                        label_selector: None,
                        resource_name: Some(rname.to_string()),
                        is_concrete: true,
                        relation_type: Some("roleRef".to_string()),
                    });
                }
            }
        }
        "ClusterRoleBinding" => {
            if let Some(rr) = obj.get("roleRef").and_then(|v| v.as_object()) {
                let rname = rr.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if !rname.is_empty() {
                    upstream.push(TopologyItem {
                        kind: "ClusterRole".to_string(),
                        name: rname.to_string(),
                        namespace: None,
                        target_kind: "clusterroles".to_string(),
                        label: format!("ClusterRole / {}", rname),
                        label_selector: None,
                        resource_name: Some(rname.to_string()),
                        is_concrete: true,
                        relation_type: Some("roleRef".to_string()),
                    });
                }
            }
        }
        "PersistentVolumeClaim" => {
            let spec = obj.get("spec").and_then(|v| v.as_object());
            if let Some(spec) = spec {
                if let Some(pv) = spec.get("volumeName").and_then(|v| v.as_str()) {
                    if !pv.is_empty() {
                        upstream.push(TopologyItem {
                            kind: "PersistentVolume".to_string(),
                            name: pv.to_string(),
                            namespace: None,
                            target_kind: "persistentvolumes".to_string(),
                            label: format!("PV / {}", pv),
                            label_selector: None,
                            resource_name: Some(pv.to_string()),
                            is_concrete: true,
                            relation_type: None,
                        });
                    }
                }
                if let Some(sc) = spec.get("storageClassName").and_then(|v| v.as_str()) {
                    if !sc.is_empty() {
                        upstream.push(TopologyItem {
                            kind: "StorageClass".to_string(),
                            name: sc.to_string(),
                            namespace: None,
                            target_kind: "storageclasses".to_string(),
                            label: format!("StorageClass / {}", sc),
                            label_selector: None,
                            resource_name: Some(sc.to_string()),
                            is_concrete: true,
                            relation_type: None,
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
                    if let Some(rt) = workload_ref_relation_type(&vo, "PersistentVolumeClaim", name) {
                        upstream.push(TopologyItem {
                            kind: "Deployment".to_string(),
                            name: d.name.clone(),
                            namespace: Some(d.namespace.clone()),
                            target_kind: "deployments".to_string(),
                            label: d.name.clone(),
                            label_selector: None,
                            resource_name: Some(d.name.clone()),
                            is_concrete: true,
                            relation_type: Some(rt.to_string()),
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
                    if let Some(rt) = workload_ref_relation_type(&vo, "PersistentVolumeClaim", name) {
                        upstream.push(TopologyItem {
                            kind: "StatefulSet".to_string(),
                            name: s.name.clone(),
                            namespace: Some(s.namespace.clone()),
                            target_kind: "statefulsets".to_string(),
                            label: s.name.clone(),
                            label_selector: None,
                            resource_name: Some(s.name.clone()),
                            is_concrete: true,
                            relation_type: Some(rt.to_string()),
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
                            upstream.push(TopologyItem {
                                kind: "StatefulSet".to_string(),
                                name: s.name.clone(),
                                namespace: Some(s.namespace.clone()),
                                target_kind: "statefulsets".to_string(),
                                label: s.name.clone(),
                                label_selector: None,
                                resource_name: Some(s.name.clone()),
                                is_concrete: true,
                                relation_type: Some("volumeClaimTemplates".to_string()),
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
                    if let Some(rt) = workload_ref_relation_type(&vo, "PersistentVolumeClaim", name) {
                        upstream.push(TopologyItem {
                            kind: "DaemonSet".to_string(),
                            name: d.name.clone(),
                            namespace: Some(d.namespace.clone()),
                            target_kind: "daemonsets".to_string(),
                            label: d.name.clone(),
                            label_selector: None,
                            resource_name: Some(d.name.clone()),
                            is_concrete: true,
                            relation_type: Some(rt.to_string()),
                        });
                    }
                }
            }
            if let Ok(pods) = list_pods_using_pvc(client, ns_ref, name).await {
                for p in pods {
                    downstream.push(TopologyItem {
                        kind: "Pod".to_string(),
                        name: p.name.clone(),
                        namespace: ns.clone(),
                        target_kind: "pods".to_string(),
                        label: p.name.clone(),
                        label_selector: None,
                        resource_name: Some(p.name),
                        is_concrete: true,
                        relation_type: Some("volumes".to_string()),
                    });
                }
            }
        }
        _ => {}
    }

    Ok(ResourceTopology { upstream, downstream })
}
