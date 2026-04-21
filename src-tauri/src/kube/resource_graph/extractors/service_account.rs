//! Workload / Pod → ServiceAccount 静态引用。

use crate::kube::resource_graph::{extractor::RelationExtractor, RelationType, ResourceEdge, ResourceRef};
use async_trait::async_trait;

pub struct ServiceAccountExtractor;

#[async_trait]
impl RelationExtractor for ServiceAccountExtractor {
    fn source_kinds(&self) -> &[&str] {
        &["Deployment", "StatefulSet", "DaemonSet", "Pod"]
    }

    fn extract_static(&self, node_ref: &ResourceRef, value: &serde_json::Value) -> Vec<ResourceEdge> {
        let pod_spec = if node_ref.kind == "Pod" {
            value.get("spec")
        } else {
            value.get("spec").and_then(|v| v.get("template")).and_then(|v| v.get("spec"))
        };

        let sa_name = match pod_spec.and_then(|s| s.get("serviceAccountName")).and_then(|v| v.as_str()) {
            Some(n) if !n.is_empty() && n != "default" => n,
            _ => return vec![],
        };

        vec![ResourceEdge {
            from: node_ref.clone(),
            to: ResourceRef::new("ServiceAccount", node_ref.namespace.clone(), sa_name),
            relation_type: RelationType::ServiceAccountRef,
            label_selector: None,
        }]
    }
}
