//! HPA 双向关联：
//! - HPA → Workload（静态，spec.scaleTargetRef）
//! - Workload ← HPA 反向（动态，list HPA）

use crate::kube::resource_graph::{extractor::RelationExtractor, RelationType, ResourceEdge, ResourceRef};
use crate::kube::resources::list_horizontal_pod_autoscalers;
use async_trait::async_trait;
use kube::Client;

pub struct HpaRefExtractor;

#[async_trait]
impl RelationExtractor for HpaRefExtractor {
    fn source_kinds(&self) -> &[&str] {
        &["HorizontalPodAutoscaler", "Deployment", "StatefulSet"]
    }

    fn extract_static(&self, node_ref: &ResourceRef, value: &serde_json::Value) -> Vec<ResourceEdge> {
        if node_ref.kind != "HorizontalPodAutoscaler" { return vec![]; }

        let target_ref = match value.get("spec").and_then(|s| s.get("scaleTargetRef")) {
            Some(r) => r,
            None => return vec![],
        };
        let target_kind = target_ref.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let target_name = target_ref.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if target_name.is_empty() { return vec![]; }

        vec![ResourceEdge {
            from: node_ref.clone(),
            to: ResourceRef::new(target_kind, node_ref.namespace.clone(), target_name),
            relation_type: RelationType::HpaTarget,
            label_selector: None,
        }]
    }

    async fn extract_dynamic(
        &self,
        node_ref: &ResourceRef,
        _value: &serde_json::Value,
        client: &Client,
        namespace: Option<&str>,
    ) -> Vec<ResourceEdge> {
        if !matches!(node_ref.kind.as_str(), "Deployment" | "StatefulSet") { return vec![]; }

        let hpas = match list_horizontal_pod_autoscalers(client, namespace, None).await {
            Ok(v) => v,
            Err(_) => return vec![],
        };

        hpas.iter()
            .filter(|h| {
                h.reference.as_deref()
                    .map(|r| r == format!("{}/{}", node_ref.kind, node_ref.name))
                    .unwrap_or(false)
            })
            .map(|h| ResourceEdge {
                from: node_ref.clone(),
                to: ResourceRef::new("HorizontalPodAutoscaler", node_ref.namespace.clone(), &h.name),
                relation_type: RelationType::ScaledBy,
                label_selector: None,
            })
            .collect()
    }
}
