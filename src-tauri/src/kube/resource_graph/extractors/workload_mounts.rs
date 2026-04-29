//! Workload 挂载引用：volumes / envFrom / env.valueFrom / imagePullSecrets。
//! 适用于 Deployment / StatefulSet / DaemonSet / Pod。

use crate::kube::resource_graph::{extractor::RelationExtractor, RelationType, ResourceEdge, ResourceRef};
use async_trait::async_trait;

pub struct WorkloadMountsExtractor;

fn extract_from_pod_spec(
    node_ref: &ResourceRef,
    pod_spec: &serde_json::Map<String, serde_json::Value>,
    edges: &mut Vec<ResourceEdge>,
) {
    let ns = node_ref.namespace.clone();

    // volumes[]
    if let Some(vols) = pod_spec.get("volumes").and_then(|v| v.as_array()) {
        for v in vols {
            let obj = match v.as_object() { Some(o) => o, None => continue };
            if let Some(cm) = obj.get("configMap").and_then(|x| x.get("name")).and_then(|x| x.as_str()) {
                if !cm.is_empty() {
                    edges.push(ResourceEdge {
                        from: node_ref.clone(),
                        to: ResourceRef::new("ConfigMap", ns.clone(), cm),
                        relation_type: RelationType::Volume,
                        label_selector: None, to_display: None,
                    });
                }
            }
            if let Some(sec) = obj.get("secret").and_then(|x| x.get("secretName")).and_then(|x| x.as_str()) {
                if !sec.is_empty() {
                    edges.push(ResourceEdge {
                        from: node_ref.clone(),
                        to: ResourceRef::new("Secret", ns.clone(), sec),
                        relation_type: RelationType::Volume,
                        label_selector: None, to_display: None,
                    });
                }
            }
            if let Some(pvc) = obj.get("persistentVolumeClaim").and_then(|x| x.get("claimName")).and_then(|x| x.as_str()) {
                if !pvc.is_empty() {
                    edges.push(ResourceEdge {
                        from: node_ref.clone(),
                        to: ResourceRef::new("PersistentVolumeClaim", ns.clone(), pvc),
                        relation_type: RelationType::Volume,
                        label_selector: None, to_display: None,
                    });
                }
            }
        }
    }

    // imagePullSecrets[]
    if let Some(ips) = pod_spec.get("imagePullSecrets").and_then(|v| v.as_array()) {
        for s in ips {
            if let Some(name) = s.get("name").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    edges.push(ResourceEdge {
                        from: node_ref.clone(),
                        to: ResourceRef::new("Secret", ns.clone(), name),
                        relation_type: RelationType::ImagePullSecret,
                        label_selector: None, to_display: None,
                    });
                }
            }
        }
    }

    // containers[] + initContainers[]
    let container_arrays = ["containers", "initContainers"];
    for arr_key in container_arrays {
        if let Some(containers) = pod_spec.get(arr_key).and_then(|v| v.as_array()) {
            for c in containers {
                let c_obj = match c.as_object() { Some(o) => o, None => continue };

                // envFrom[]
                if let Some(env_from) = c_obj.get("envFrom").and_then(|v| v.as_array()) {
                    for ef in env_from {
                        let ef_obj = match ef.as_object() { Some(o) => o, None => continue };
                        if let Some(cm) = ef_obj.get("configMapRef").and_then(|x| x.get("name")).and_then(|x| x.as_str()) {
                            if !cm.is_empty() {
                                edges.push(ResourceEdge {
                                    from: node_ref.clone(),
                                    to: ResourceRef::new("ConfigMap", ns.clone(), cm),
                                    relation_type: RelationType::EnvFrom,
                                    label_selector: None, to_display: None,
                                });
                            }
                        }
                        if let Some(sec) = ef_obj.get("secretRef").and_then(|x| x.get("name")).and_then(|x| x.as_str()) {
                            if !sec.is_empty() {
                                edges.push(ResourceEdge {
                                    from: node_ref.clone(),
                                    to: ResourceRef::new("Secret", ns.clone(), sec),
                                    relation_type: RelationType::EnvFrom,
                                    label_selector: None, to_display: None,
                                });
                            }
                        }
                    }
                }

                // env[].valueFrom
                if let Some(env_list) = c_obj.get("env").and_then(|v| v.as_array()) {
                    for env_item in env_list {
                        let vf = match env_item.get("valueFrom").and_then(|v| v.as_object()) {
                            Some(o) => o,
                            None => continue,
                        };
                        if let Some(cm) = vf.get("configMapKeyRef").and_then(|x| x.get("name")).and_then(|x| x.as_str()) {
                            if !cm.is_empty() {
                                edges.push(ResourceEdge {
                                    from: node_ref.clone(),
                                    to: ResourceRef::new("ConfigMap", ns.clone(), cm),
                                    relation_type: RelationType::EnvValue,
                                    label_selector: None, to_display: None,
                                });
                            }
                        }
                        if let Some(sec) = vf.get("secretKeyRef").and_then(|x| x.get("name")).and_then(|x| x.as_str()) {
                            if !sec.is_empty() {
                                edges.push(ResourceEdge {
                                    from: node_ref.clone(),
                                    to: ResourceRef::new("Secret", ns.clone(), sec),
                                    relation_type: RelationType::EnvValue,
                                    label_selector: None, to_display: None,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

#[async_trait]
impl RelationExtractor for WorkloadMountsExtractor {
    fn source_kinds(&self) -> &[&str] {
        &["Deployment", "StatefulSet", "DaemonSet", "Pod"]
    }

    fn extract_static(&self, node_ref: &ResourceRef, value: &serde_json::Value) -> Vec<ResourceEdge> {
        let mut edges = Vec::new();

        let pod_spec = if node_ref.kind == "Pod" {
            value.get("spec").and_then(|v| v.as_object())
        } else {
            value.get("spec")
                .and_then(|v| v.get("template"))
                .and_then(|v| v.get("spec"))
                .and_then(|v| v.as_object())
        };

        if let Some(ps) = pod_spec {
            extract_from_pod_spec(node_ref, ps, &mut edges);
        }

        edges
    }
}
