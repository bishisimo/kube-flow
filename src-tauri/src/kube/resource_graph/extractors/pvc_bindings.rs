//! PVC 关联：
//! - PVC → PV（spec.volumeName）
//! - PVC → StorageClass（spec.storageClassName）
//! - StatefulSet volumeClaimTemplates → PVC（动态，需要 list PVC）
//! - Workload ← PVC 反向（workload 挂载了哪些 PVC 由 WorkloadMountsExtractor 处理）

use crate::kube::resource_graph::{extractor::RelationExtractor, RelationType, ResourceEdge, ResourceRef};
use crate::kube::resources::list_persistent_volume_claims;
use async_trait::async_trait;
use kube::Client;

pub struct PvcBindingsExtractor;

fn statefulset_volume_claim_template_names(value: &serde_json::Value) -> Vec<String> {
    let templates = value
        .get("spec")
        .and_then(|v| v.get("volumeClaimTemplates"))
        .and_then(|v| v.as_array());
    let Some(templates) = templates else { return vec![] };
    templates.iter()
        .filter_map(|t| t.get("metadata").and_then(|m| m.get("name")).and_then(|v| v.as_str()))
        .filter(|n| !n.is_empty())
        .map(String::from)
        .collect()
}

#[async_trait]
impl RelationExtractor for PvcBindingsExtractor {
    fn source_kinds(&self) -> &[&str] {
        &["PersistentVolumeClaim", "StatefulSet"]
    }

    fn extract_static(&self, node_ref: &ResourceRef, value: &serde_json::Value) -> Vec<ResourceEdge> {
        if node_ref.kind != "PersistentVolumeClaim" { return vec![]; }

        let mut edges = Vec::new();
        let spec = match value.get("spec").and_then(|v| v.as_object()) {
            Some(s) => s,
            None => return edges,
        };

        if let Some(pv) = spec.get("volumeName").and_then(|v| v.as_str()) {
            if !pv.is_empty() {
                edges.push(ResourceEdge {
                    from: node_ref.clone(),
                    to: ResourceRef::new("PersistentVolume", None, pv),
                    relation_type: RelationType::BoundVolume,
                    label_selector: None,
                });
            }
        }
        if let Some(sc) = spec.get("storageClassName").and_then(|v| v.as_str()) {
            if !sc.is_empty() {
                edges.push(ResourceEdge {
                    from: node_ref.clone(),
                    to: ResourceRef::new("StorageClass", None, sc),
                    relation_type: RelationType::StorageClass,
                    label_selector: None,
                });
            }
        }

        edges
    }

    /// StatefulSet: volumeClaimTemplates → 已生成的 PVC（需要 list）
    async fn extract_dynamic(
        &self,
        node_ref: &ResourceRef,
        value: &serde_json::Value,
        client: &Client,
        namespace: Option<&str>,
    ) -> Vec<ResourceEdge> {
        if node_ref.kind != "StatefulSet" { return vec![]; }

        let template_names = statefulset_volume_claim_template_names(value);
        if template_names.is_empty() { return vec![]; }

        let ns = namespace.unwrap_or("default");
        let pvcs = match list_persistent_volume_claims(client, Some(ns), None).await {
            Ok(v) => v,
            Err(_) => return vec![],
        };

        pvcs.iter()
            .filter(|pvc| {
                template_names.iter().any(|t| {
                    pvc.name.starts_with(&format!("{}-{}-", t, node_ref.name))
                })
            })
            .map(|pvc| ResourceEdge {
                from: node_ref.clone(),
                to: ResourceRef::new("PersistentVolumeClaim", node_ref.namespace.clone(), &pvc.name),
                relation_type: RelationType::Volume,
                label_selector: None,
            })
            .collect()
    }
}
