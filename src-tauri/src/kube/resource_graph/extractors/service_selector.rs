//! Service.spec.selector → Pod 聚合节点。

use crate::kube::resource_graph::{
    extractor::RelationExtractor, simple_map_to_selector, RelationType, ResourceEdge, ResourceRef,
};
use crate::kube::resources::list_pods;
use async_trait::async_trait;
use kube::Client;

pub struct ServiceSelectorExtractor;

#[async_trait]
impl RelationExtractor for ServiceSelectorExtractor {
    fn source_kinds(&self) -> &[&str] {
        &["Service"]
    }

    fn extract_static(&self, _node_ref: &ResourceRef, _value: &serde_json::Value) -> Vec<ResourceEdge> {
        vec![]
    }

    async fn extract_dynamic(
        &self,
        node_ref: &ResourceRef,
        value: &serde_json::Value,
        client: &Client,
        namespace: Option<&str>,
    ) -> Vec<ResourceEdge> {
        let sel_map = match value.get("spec")
            .and_then(|v| v.get("selector"))
            .and_then(|v| v.as_object())
        {
            Some(m) => m,
            None => return vec![],
        };
        let ls = match simple_map_to_selector(sel_map) {
            Some(s) => s,
            None => return vec![],
        };

        let count = list_pods(client, namespace, Some(&ls)).await.map(|v| v.len()).unwrap_or(0);

        vec![ResourceEdge {
            from: node_ref.clone(),
            to: ResourceRef {
                kind: "Pod".to_string(),
                namespace: node_ref.namespace.clone(),
                name: format!("Pods ({})", count),
            },
            relation_type: RelationType::ServiceSelector,
            label_selector: Some(ls),
        }]
    }
}
