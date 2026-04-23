//! ServiceAccount 反向 RBAC 关联：
//! - ServiceAccount ← RoleBinding（动态，list RoleBinding，按 subjects 过滤）
//! - ServiceAccount ← ClusterRoleBinding（动态，list ClusterRoleBinding，按 subjects 过滤）
//! 产生的边由 BFS 加入图后，rbac_refs.rs 会继续展开 RoleBinding → Role/ClusterRole。

use crate::kube::resource_graph::{extractor::RelationExtractor, RelationType, ResourceEdge, ResourceRef};
use crate::kube::resources::{list_role_bindings, list_cluster_role_bindings};
use async_trait::async_trait;
use kube::Client;

pub struct SaBindingsReverseExtractor;

#[async_trait]
impl RelationExtractor for SaBindingsReverseExtractor {
    fn source_kinds(&self) -> &[&str] {
        &["ServiceAccount"]
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
        let mut edges = Vec::new();
        let sa_name = &node_ref.name;
        let ns = node_ref.namespace.as_deref().or(namespace).unwrap_or("default");

        if let Ok(rbs) = list_role_bindings(client, Some(ns), None).await {
            for rb in rbs {
                let matched = rb.subjects_list.as_ref().map_or(false, |subs| {
                    subs.iter().any(|s| {
                        s.kind == "ServiceAccount"
                            && s.name == *sa_name
                            && s.namespace.as_deref().unwrap_or(ns) == ns
                    })
                });
                if matched {
                    edges.push(ResourceEdge {
                        from: ResourceRef::new("RoleBinding", Some(ns.to_string()), &rb.name),
                        to: node_ref.clone(),
                        relation_type: RelationType::RoleRef,
                        label_selector: None, to_display: None,
                    });
                }
            }
        }

        if let Ok(crbs) = list_cluster_role_bindings(client, None).await {
            for crb in crbs {
                let matched = crb.subjects_list.as_ref().map_or(false, |subs| {
                    subs.iter().any(|s| {
                        s.kind == "ServiceAccount"
                            && s.name == *sa_name
                            && s.namespace.as_deref().unwrap_or(ns) == ns
                    })
                });
                if matched {
                    edges.push(ResourceEdge {
                        from: ResourceRef::new("ClusterRoleBinding", None, &crb.name),
                        to: node_ref.clone(),
                        relation_type: RelationType::RoleRef,
                        label_selector: None, to_display: None,
                    });
                }
            }
        }

        edges
    }
}
