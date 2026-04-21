//! 网络资源：Service, Endpoints, EndpointSlice, Ingress, IngressClass, NetworkPolicy。

use super::{
    build_list_params, format_creation_time, list_simple_namespaced, ResourceError,
    SimpleNamespacedItem,
};
use k8s_openapi::api::core::v1::{Endpoints, Service};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::api::networking::v1::{Ingress, IngressClass, NetworkPolicy};
use kube::api::{Api, ListParams};
use kube::Client;
use serde::Serialize;

// ── 数据结构 ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ServiceItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_ip: Option<String>,
    /// 端口摘要；NodePort 时为 "port:nodePort/protocol"，如 "80:30080/TCP, 443:30443/TCP"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EndpointsItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subsets: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EndpointSliceItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoints: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IngressItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IngressClassItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkPolicyItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

impl SimpleNamespacedItem for NetworkPolicyItem {
    fn from_meta(meta: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta, default_ns: &str) -> Self {
        Self {
            name: meta.name.unwrap_or_default(),
            namespace: meta.namespace.unwrap_or_else(|| default_ns.to_string()),
            creation_time: format_creation_time(meta.creation_timestamp.as_ref()),
        }
    }
}

// ── list 函数 ──────────────────────────────────────────────────────────────

/// 列出指定 namespace 的 Services。
pub async fn list_services(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<ServiceItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<Service> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|s| {
            let spec = s.spec.as_ref();
            let svc_type = spec.and_then(|sp| sp.type_.as_deref());
            let is_nodeport = svc_type == Some("NodePort");
            let ports_str = spec.and_then(|sp| {
                let ports = sp.ports.as_deref()?;
                if ports.is_empty() {
                    return None;
                }
                let parts: Vec<String> = ports
                    .iter()
                    .map(|p| {
                        let proto = p.protocol.as_deref().unwrap_or("TCP");
                        if is_nodeport {
                            if let Some(np) = p.node_port {
                                format!("{}:{}/{}", p.port, np, proto)
                            } else {
                                format!("{}/{}", p.port, proto)
                            }
                        } else {
                            format!("{}/{}", p.port, proto)
                        }
                    })
                    .collect();
                Some(parts.join(", "))
            });
            ServiceItem {
                name: s.metadata.name.unwrap_or_default(),
                namespace: s.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                service_type: spec.and_then(|sp| sp.type_.clone()),
                cluster_ip: s.spec.and_then(|spec| spec.cluster_ip),
                ports: ports_str,
                creation_time: format_creation_time(s.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出 selector 与 workload 匹配的 Services。
pub async fn list_services_matching_workload_selector(
    client: &Client,
    namespace: &str,
    workload_selector: &serde_json::Value,
) -> Result<Vec<String>, ResourceError> {
    let workload_labels = workload_selector
        .get("matchLabels")
        .and_then(|v| v.as_object())
        .filter(|m| !m.is_empty());
    let Some(workload_labels) = workload_labels else {
        return Ok(Vec::new());
    };
    let api: Api<Service> = Api::namespaced(client.clone(), namespace);
    let list = api.list(&ListParams::default()).await.map_err(ResourceError::Kube)?;
    let mut names = Vec::new();
    for svc in list.items {
        let Some(spec) = svc.spec.as_ref() else { continue };
        let Some(sel) = spec.selector.as_ref() else { continue };
        if sel.is_empty() {
            continue;
        }
        let all_match = sel.iter().all(|(k, v)| {
            workload_labels
                .get(k)
                .and_then(|lv| lv.as_str())
                .map(|lv| lv == v)
                .unwrap_or(false)
        });
        if all_match {
            if let Some(name) = svc.metadata.name.as_deref() {
                names.push(name.to_string());
            }
        }
    }
    Ok(names)
}

/// 列出 selector 与 Pod labels 匹配的 Services。
pub async fn list_services_matching_pod_labels(
    client: &Client,
    namespace: &str,
    pod_labels: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<String>, ResourceError> {
    if pod_labels.is_empty() {
        return Ok(Vec::new());
    }
    let api: Api<Service> = Api::namespaced(client.clone(), namespace);
    let list = api.list(&ListParams::default()).await.map_err(ResourceError::Kube)?;
    let mut names = Vec::new();
    for svc in list.items {
        let Some(spec) = svc.spec.as_ref() else { continue };
        let Some(sel) = spec.selector.as_ref() else { continue };
        if sel.is_empty() {
            continue;
        }
        let all_match = sel.iter().all(|(k, v)| {
            pod_labels
                .get(k)
                .and_then(|lv| lv.as_str())
                .map(|lv| lv == v)
                .unwrap_or(false)
        });
        if all_match {
            if let Some(name) = svc.metadata.name.as_deref() {
                names.push(name.to_string());
            }
        }
    }
    Ok(names)
}

/// 列出指定 namespace 的 Endpoints。
pub async fn list_endpoints(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<EndpointsItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<Endpoints> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|e| {
            let subsets = e.subsets.as_ref().map(|s| s.len() as u32);
            EndpointsItem {
                name: e.metadata.name.unwrap_or_default(),
                namespace: e.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                subsets,
                creation_time: format_creation_time(e.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 EndpointSlices。
pub async fn list_endpoint_slices(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<EndpointSliceItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<EndpointSlice> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|e| {
            let endpoints = Some(e.endpoints.len() as u32);
            EndpointSliceItem {
                name: e.metadata.name.unwrap_or_default(),
                namespace: e.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                address_type: Some(e.address_type.clone()),
                endpoints,
                creation_time: format_creation_time(e.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 Ingresses。
pub async fn list_ingresses(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<IngressItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<Ingress> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|i| {
            let class = i.spec.as_ref().and_then(|s| s.ingress_class_name.clone());
            let hosts = i
                .spec
                .as_ref()
                .and_then(|s| s.rules.as_ref())
                .map(|r| r.iter().filter_map(|rule| rule.host.clone()).collect::<Vec<_>>().join(", "))
                .filter(|s| !s.is_empty());
            IngressItem {
                name: i.metadata.name.unwrap_or_default(),
                namespace: i.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                class,
                hosts,
                creation_time: format_creation_time(i.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出集群级 IngressClasses。
pub async fn list_ingress_classes(
    client: &Client,
    label_selector: Option<&str>,
) -> Result<Vec<IngressClassItem>, ResourceError> {
    let api: Api<IngressClass> = Api::all(client.clone());
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|i| IngressClassItem {
            name: i.metadata.name.unwrap_or_default(),
            controller: i.spec.and_then(|s| s.controller),
            creation_time: format_creation_time(i.metadata.creation_timestamp.as_ref()),
        })
        .collect();
    Ok(items)
}

list_simple_namespaced!(list_network_policies, NetworkPolicy, NetworkPolicyItem);
