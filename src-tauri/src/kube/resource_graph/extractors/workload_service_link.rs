//! Deployment / StatefulSet / DaemonSet → 与 Pod 模板的 label 被 Service.spec.selector 所匹配的 Service。

use crate::kube::resource_graph::{
    extractor::RelationExtractor, RelationType, ResourceEdge, ResourceRef,
};
use async_trait::async_trait;
use k8s_openapi::api::core::v1::Service;
use kube::api::{Api, ListParams};
use kube::Client;
use serde_json::Value;

pub struct WorkloadServiceLinkExtractor;

/// 与 Endpoints/selector 比较时使用的 Pod 标签：模板 labels 覆盖 workload selector.matchLabels（与已调度 Pod 一致）。
fn effective_pod_template_labels(value: &Value) -> Option<serde_json::Map<String, Value>> {
    let mut m = serde_json::Map::new();
    if let Some(sel) = value
        .get("spec")
        .and_then(|s| s.get("selector"))
        .and_then(|s| s.get("matchLabels"))
        .and_then(|l| l.as_object())
    {
        for (k, v) in sel {
            m.insert(k.clone(), v.clone());
        }
    }
    if let Some(tpl) = value
        .get("spec")
        .and_then(|s| s.get("template"))
        .and_then(|t| t.get("metadata"))
        .and_then(|meta| meta.get("labels"))
        .and_then(|l| l.as_object())
    {
        for (k, v) in tpl {
            m.insert(k.clone(), v.clone());
        }
    }
    if m.is_empty() {
        None
    } else {
        Some(m)
    }
}

/// 非空 selector 且为 Pod 模板 labels 的子集时，该 Service 会匹配此工作负载产生的 Pod。
fn service_selects_pods_in_template(
    svc: &Service,
    template_labels: &serde_json::Map<String, Value>,
) -> bool {
    let sel = match svc.spec.as_ref().and_then(|s| s.selector.as_ref()) {
        Some(s) if !s.is_empty() => s,
        _ => return false,
    };
    for (k, want) in sel.iter() {
        let got = match template_labels.get(k).and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return false,
        };
        if got != want.as_str() {
            return false;
        }
    }
    true
}

#[async_trait]
impl RelationExtractor for WorkloadServiceLinkExtractor {
    fn source_kinds(&self) -> &[&str] {
        &["Deployment", "StatefulSet", "DaemonSet"]
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
        let template_labels = match effective_pod_template_labels(value) {
            Some(m) => m,
            None => return vec![],
        };
        let ns = match namespace {
            Some(n) => n,
            None => return vec![],
        };

        let api: Api<Service> = Api::namespaced(client.clone(), ns);
        let list = match api.list(&ListParams::default()).await {
            Ok(l) => l,
            Err(_) => return vec![],
        };

        let mut edges = Vec::new();
        for svc in list.items {
            if !service_selects_pods_in_template(&svc, &template_labels) {
                continue;
            }
            let name = match &svc.metadata.name {
                Some(n) if !n.is_empty() => n.clone(),
                _ => continue,
            };
            edges.push(ResourceEdge {
                from: node_ref.clone(),
                to: ResourceRef::new("Service", node_ref.namespace.clone(), &name),
                relation_type: RelationType::ServicePodMatch,
                label_selector: None,
                to_display: None,
            });
        }
        edges
    }
}
