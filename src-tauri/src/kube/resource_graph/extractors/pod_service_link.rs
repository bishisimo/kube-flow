//! Pod → 与其 labels 被 Service.spec.selector 命中的 Service。

use crate::kube::resource_graph::{
    extractor::RelationExtractor, RelationType, ResourceEdge, ResourceRef,
};
use async_trait::async_trait;
use k8s_openapi::api::core::v1::Service;
use kube::api::{Api, ListParams};
use kube::Client;
use serde_json::Value;

pub struct PodServiceLinkExtractor;

fn pod_labels(value: &Value) -> Option<serde_json::Map<String, Value>> {
    value
        .get("metadata")
        .and_then(|m| m.get("labels"))
        .and_then(|l| l.as_object())
        .cloned()
}

fn service_selects_pod(svc: &Service, labels: &serde_json::Map<String, Value>) -> bool {
    let sel = match svc.spec.as_ref().and_then(|s| s.selector.as_ref()) {
        Some(s) if !s.is_empty() => s,
        _ => return false,
    };
    for (k, want) in sel {
        let got = match labels.get(k).and_then(|v| v.as_str()) {
            Some(v) => v,
            None => return false,
        };
        if got != want.as_str() {
            return false;
        }
    }
    true
}

#[async_trait]
impl RelationExtractor for PodServiceLinkExtractor {
    fn source_kinds(&self) -> &[&str] {
        &["Pod"]
    }

    fn extract_static(&self, _node_ref: &ResourceRef, _value: &Value) -> Vec<ResourceEdge> {
        vec![]
    }

    async fn extract_dynamic(
        &self,
        node_ref: &ResourceRef,
        value: &Value,
        client: &Client,
        namespace: Option<&str>,
    ) -> Vec<ResourceEdge> {
        let labels = match pod_labels(value) {
            Some(labels) if !labels.is_empty() => labels,
            _ => return vec![],
        };
        let ns = match namespace {
            Some(ns) => ns,
            None => return vec![],
        };

        let api: Api<Service> = Api::namespaced(client.clone(), ns);
        let list = match api.list(&ListParams::default()).await {
            Ok(list) => list,
            Err(_) => return vec![],
        };

        list.items
            .into_iter()
            .filter(|svc| service_selects_pod(svc, &labels))
            .filter_map(|svc| {
                let name = svc.metadata.name?;
                if name.is_empty() {
                    return None;
                }
                Some(ResourceEdge {
                    from: node_ref.clone(),
                    to: ResourceRef::new("Service", node_ref.namespace.clone(), name),
                    relation_type: RelationType::ServicePodMatch,
                    label_selector: None,
                    to_display: None,
                })
            })
            .collect()
    }
}
