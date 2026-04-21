//! Ingress.spec.rules[].http.paths[].backend.service → Service（静态声明引用）。
//! 同时包含 spec.defaultBackend.service。

use crate::kube::resource_graph::{extractor::RelationExtractor, RelationType, ResourceEdge, ResourceRef};
use async_trait::async_trait;

pub struct IngressBackendExtractor;

fn extract_service_name(backend: &serde_json::Value) -> Option<&str> {
    backend.get("service")
        .and_then(|s| s.get("name"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
}

#[async_trait]
impl RelationExtractor for IngressBackendExtractor {
    fn source_kinds(&self) -> &[&str] {
        &["Ingress"]
    }

    fn extract_static(&self, node_ref: &ResourceRef, value: &serde_json::Value) -> Vec<ResourceEdge> {
        let mut edges = Vec::new();
        let spec = match value.get("spec") {
            Some(s) => s,
            None => return edges,
        };

        let ns = node_ref.namespace.clone();

        // spec.defaultBackend
        if let Some(svc_name) = spec.get("defaultBackend").and_then(extract_service_name) {
            edges.push(ResourceEdge {
                from: node_ref.clone(),
                to: ResourceRef::new("Service", ns.clone(), svc_name),
                relation_type: RelationType::IngressBackend,
                label_selector: None,
            });
        }

        // spec.rules[].http.paths[].backend
        if let Some(rules) = spec.get("rules").and_then(|v| v.as_array()) {
            for rule in rules {
                if let Some(paths) = rule.get("http").and_then(|h| h.get("paths")).and_then(|v| v.as_array()) {
                    for path in paths {
                        if let Some(svc_name) = path.get("backend").and_then(extract_service_name) {
                            edges.push(ResourceEdge {
                                from: node_ref.clone(),
                                to: ResourceRef::new("Service", ns.clone(), svc_name),
                                relation_type: RelationType::IngressBackend,
                                label_selector: None,
                            });
                        }
                    }
                }
            }
        }

        edges
    }
}
