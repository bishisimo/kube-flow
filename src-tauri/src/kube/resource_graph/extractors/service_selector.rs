//! Service.spec.selector → Pod 聚合节点。

use crate::kube::resource_graph::{
    extractor::RelationExtractor, set_id_for_label_aggregate, simple_map_to_selector, RelationType,
    ResourceEdge, ResourceRef,
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
        let set_id = set_id_for_label_aggregate("Pod", node_ref.namespace.as_deref(), &ls);

        vec![ResourceEdge {
            from: node_ref.clone(),
            to: ResourceRef::for_label_set("Pod", node_ref.namespace.clone(), set_id),
            relation_type: RelationType::ServiceSelector,
            label_selector: Some(ls),
            to_display: Some(format!("Pods ({})", count)),
        }]
    }
}
