//! Service ← Ingress 反向关联：给定 Service，找出引用了它的 Ingress。

use crate::kube::resource_graph::{extractor::RelationExtractor, RelationType, ResourceEdge, ResourceRef};
use crate::kube::resources::list_ingresses;
use async_trait::async_trait;
use kube::Client;

pub struct ServiceReverseExtractor;

fn ingress_references_service(ingress_value: &serde_json::Value, svc_name: &str) -> bool {
    let spec = match ingress_value.get("spec") { Some(s) => s, None => return false };

    if let Some(s) = spec.get("defaultBackend").and_then(|b| b.get("service")).and_then(|s| s.get("name")).and_then(|v| v.as_str()) {
        if s == svc_name { return true; }
    }

    if let Some(rules) = spec.get("rules").and_then(|v| v.as_array()) {
        for rule in rules {
            if let Some(paths) = rule.get("http").and_then(|h| h.get("paths")).and_then(|v| v.as_array()) {
                for path in paths {
                    if let Some(s) = path.get("backend").and_then(|b| b.get("service")).and_then(|s| s.get("name")).and_then(|v| v.as_str()) {
                        if s == svc_name { return true; }
                    }
                }
            }
        }
    }
    false
}

#[async_trait]
impl RelationExtractor for ServiceReverseExtractor {
    fn source_kinds(&self) -> &[&str] {
        &["Service"]
    }

    fn extract_static(&self, _node_ref: &ResourceRef, _value: &serde_json::Value) -> Vec<ResourceEdge> {
        vec![]
    }

    async fn extract_dynamic(
        &self,
        node_ref: &ResourceRef,
        _value: &serde_json::Value,
        client: &Client,
        namespace: Option<&str>,
    ) -> Vec<ResourceEdge> {
        let ingresses = match list_ingresses(client, namespace, None).await {
            Ok(v) => v,
            Err(_) => return vec![],
        };

        let mut edges = Vec::new();
        for ing in ingresses {
            // list_ingresses 返回摘要，需要 get 完整对象
            if let Ok(ing_value) = crate::kube::resource_get::get_resource_value(
                client, "Ingress", &ing.name, namespace
            ).await {
                if ingress_references_service(&ing_value, &node_ref.name) {
                    edges.push(ResourceEdge {
                        from: node_ref.clone(),
                        to: ResourceRef::new("Ingress", node_ref.namespace.clone(), &ing.name),
                        relation_type: RelationType::Routes,
                        label_selector: None,
                    });
                }
            }
        }
        edges
    }
}
