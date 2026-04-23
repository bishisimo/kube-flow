//! RBAC 关联：
//! - RoleBinding / ClusterRoleBinding → Role / ClusterRole（roleRef）
//! - RoleBinding / ClusterRoleBinding → ServiceAccount（subjects）

use crate::kube::resource_graph::{extractor::RelationExtractor, RelationType, ResourceEdge, ResourceRef};
use async_trait::async_trait;

pub struct RbacRefsExtractor;

fn extract_rolebinding_edges(node_ref: &ResourceRef, value: &serde_json::Value) -> Vec<ResourceEdge> {
    let mut edges = Vec::new();
    let ns = node_ref.namespace.clone();

    // roleRef → Role / ClusterRole
    if let Some(rr) = value.get("roleRef").and_then(|v| v.as_object()) {
        let rkind = rr.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let rname = rr.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if !rname.is_empty() {
            let target_ns = if rkind == "Role" { ns.clone() } else { None };
            edges.push(ResourceEdge {
                from: node_ref.clone(),
                to: ResourceRef::new(rkind, target_ns, rname),
                relation_type: RelationType::RoleRef,
                label_selector: None, to_display: None,
            });
        }
    }

    // subjects[] → ServiceAccount
    if let Some(subs) = value.get("subjects").and_then(|v| v.as_array()) {
        for s in subs {
            let Some(sobj) = s.as_object() else { continue };
            if sobj.get("kind").and_then(|v| v.as_str()) != Some("ServiceAccount") { continue; }
            let sname = match sobj.get("name").and_then(|v| v.as_str()) { Some(n) if !n.is_empty() => n, _ => continue };
            let sns = sobj.get("namespace").and_then(|v| v.as_str()).map(String::from).or_else(|| ns.clone());
            edges.push(ResourceEdge {
                from: node_ref.clone(),
                to: ResourceRef::new("ServiceAccount", sns, sname),
                relation_type: RelationType::RoleRef,
                label_selector: None, to_display: None,
            });
        }
    }

    edges
}

#[async_trait]
impl RelationExtractor for RbacRefsExtractor {
    fn source_kinds(&self) -> &[&str] {
        &["RoleBinding", "ClusterRoleBinding"]
    }

    fn extract_static(&self, node_ref: &ResourceRef, value: &serde_json::Value) -> Vec<ResourceEdge> {
        extract_rolebinding_edges(node_ref, value)
    }
}
