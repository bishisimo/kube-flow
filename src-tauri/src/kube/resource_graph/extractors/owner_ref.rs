//! OwnerReference 关联：Pod → RS → Deployment（穿透），Pod → StatefulSet/DaemonSet/Job。
//! 静态提取直接 owner；RS 穿透需要 fetch RS，故用 dynamic。

use crate::kube::resource_graph::{extractor::RelationExtractor, RelationType, ResourceEdge, ResourceRef};
use crate::kube::resource_get::get_resource_value;
use async_trait::async_trait;
use kube::Client;

pub struct OwnerRefExtractor;

#[async_trait]
impl RelationExtractor for OwnerRefExtractor {
    fn source_kinds(&self) -> &[&str] {
        &["Pod", "ReplicaSet", "Job"]
    }

    fn extract_static(&self, node_ref: &ResourceRef, value: &serde_json::Value) -> Vec<ResourceEdge> {
        let mut edges = Vec::new();
        let owners = match value.get("metadata").and_then(|m| m.get("ownerReferences")).and_then(|v| v.as_array()) {
            Some(o) => o,
            None => return edges,
        };

        let ns = node_ref.namespace.clone();

        for o in owners {
            let o_kind = o.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let o_name = o.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if o_name.is_empty() { continue; }

            match o_kind {
                // Pod → ReplicaSet：先放一条 OwnerRef 边，dynamic 步骤再穿透
                "ReplicaSet" => {
                    edges.push(ResourceEdge {
                        from: node_ref.clone(),
                        to: ResourceRef::new("ReplicaSet", ns.clone(), o_name),
                        relation_type: RelationType::OwnerRef,
                        label_selector: None, to_display: None,
                    });
                }
                // Pod → StatefulSet / DaemonSet / Job 直接 Manages
                "StatefulSet" | "DaemonSet" | "Job" => {
                    edges.push(ResourceEdge {
                        from: node_ref.clone(),
                        to: ResourceRef::new(o_kind, ns.clone(), o_name),
                        relation_type: RelationType::Manages,
                        label_selector: None, to_display: None,
                    });
                }
                // ReplicaSet → Deployment（static，ReplicaSet 作为 source）
                "Deployment" if node_ref.kind == "ReplicaSet" => {
                    edges.push(ResourceEdge {
                        from: node_ref.clone(),
                        to: ResourceRef::new("Deployment", ns.clone(), o_name),
                        relation_type: RelationType::Manages,
                        label_selector: None, to_display: None,
                    });
                }
                _ => {}
            }
        }

        edges
    }

    /// Pod → ReplicaSet → Deployment 穿透：fetch RS 获取其 ownerRef。
    async fn extract_dynamic(
        &self,
        node_ref: &ResourceRef,
        value: &serde_json::Value,
        client: &Client,
        namespace: Option<&str>,
    ) -> Vec<ResourceEdge> {
        if node_ref.kind != "Pod" { return vec![]; }

        let mut edges = Vec::new();
        let owners = match value.get("metadata").and_then(|m| m.get("ownerReferences")).and_then(|v| v.as_array()) {
            Some(o) => o,
            None => return edges,
        };
        let ns = node_ref.namespace.clone();

        for o in owners {
            let o_kind = o.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let o_name = o.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if o_kind != "ReplicaSet" || o_name.is_empty() { continue; }

            if let Ok(rs_obj) = get_resource_value(client, "ReplicaSet", o_name, namespace).await {
                if let Some(rs_owners) = rs_obj.get("metadata").and_then(|m| m.get("ownerReferences")).and_then(|v| v.as_array()) {
                    for ro in rs_owners {
                        let ro_kind = ro.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                        let ro_name = ro.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        if ro_kind == "Deployment" && !ro_name.is_empty() {
                            edges.push(ResourceEdge {
                                from: node_ref.clone(),
                                to: ResourceRef::new("Deployment", ns.clone(), ro_name),
                                relation_type: RelationType::Manages,
                                label_selector: None, to_display: None,
                            });
                        }
                    }
                }
            }
        }

        edges
    }
}
