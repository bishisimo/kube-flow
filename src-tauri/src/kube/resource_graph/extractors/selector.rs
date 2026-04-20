//! Workload spec.selector → Pod 聚合节点。
//! 适用于 Deployment / StatefulSet / DaemonSet。

use crate::kube::resource_graph::{
    extractor::RelationExtractor, selector_to_string, RelationType, ResourceEdge, ResourceRef,
};
use crate::kube::resources::list_pods;
use async_trait::async_trait;
use kube::Client;

pub struct WorkloadSelectorExtractor;

#[async_trait]
impl RelationExtractor for WorkloadSelectorExtractor {
    fn source_kinds(&self) -> &[&str] {
        &["Deployment", "StatefulSet", "DaemonSet"]
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
        let sel = match value.get("spec").and_then(|v| v.get("selector")) {
            Some(s) => s,
            None => return vec![],
        };
        let ls = match selector_to_string(sel) {
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
            relation_type: RelationType::Selector,
            label_selector: Some(ls),
        }]
    }
}
