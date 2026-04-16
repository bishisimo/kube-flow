//! 策略与调度资源：ResourceQuota, LimitRange, PriorityClass, HPA, PodDisruptionBudget。

use super::{
    build_list_params, format_creation_time, list_simple_namespaced, ResourceError,
    SimpleNamespacedItem,
};
use k8s_openapi::api::autoscaling::v2::HorizontalPodAutoscaler;
use k8s_openapi::api::core::v1::{LimitRange, ResourceQuota};
use k8s_openapi::api::policy::v1::PodDisruptionBudget;
use k8s_openapi::api::scheduling::v1::PriorityClass;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::api::Api;
use kube::Client;
use serde::Serialize;

fn int_or_string_to_str(q: &IntOrString) -> String {
    match q {
        IntOrString::Int(i) => i.to_string(),
        IntOrString::String(s) => s.clone(),
    }
}

// ── 数据结构 ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ResourceQuotaItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

impl SimpleNamespacedItem for ResourceQuotaItem {
    fn from_meta(meta: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta, default_ns: &str) -> Self {
        Self {
            name: meta.name.unwrap_or_default(),
            namespace: meta.namespace.unwrap_or_else(|| default_ns.to_string()),
            creation_time: format_creation_time(meta.creation_timestamp.as_ref()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LimitRangeItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

impl SimpleNamespacedItem for LimitRangeItem {
    fn from_meta(meta: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta, default_ns: &str) -> Self {
        Self {
            name: meta.name.unwrap_or_default(),
            namespace: meta.namespace.unwrap_or_else(|| default_ns.to_string()),
            creation_time: format_creation_time(meta.creation_timestamp.as_ref()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PriorityClassItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HorizontalPodAutoscalerItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicas: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PodDisruptionBudgetItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_available: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_unavailable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_disruptions: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

// ── list 函数 ──────────────────────────────────────────────────────────────

list_simple_namespaced!(list_resource_quotas, ResourceQuota, ResourceQuotaItem);
list_simple_namespaced!(list_limit_ranges, LimitRange, LimitRangeItem);

/// 列出集群级 PriorityClasses。
pub async fn list_priority_classes(
    client: &Client,
    label_selector: Option<&str>,
) -> Result<Vec<PriorityClassItem>, ResourceError> {
    let api: Api<PriorityClass> = Api::all(client.clone());
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|p| PriorityClassItem {
            name: p.metadata.name.unwrap_or_default(),
            value: Some(p.value),
            creation_time: format_creation_time(p.metadata.creation_timestamp.as_ref()),
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 HorizontalPodAutoscalers。
pub async fn list_horizontal_pod_autoscalers(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<HorizontalPodAutoscalerItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<HorizontalPodAutoscaler> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|h| {
            let status = h.status.as_ref();
            let current = status.and_then(|s| s.current_replicas);
            let desired = status.map(|s| s.desired_replicas);
            let replicas = match (current, desired) {
                (Some(c), Some(d)) => Some(format!("{}/{}", c, d)),
                (Some(c), _) => Some(format!("{}/{}", c, c)),
                _ => None,
            };
            let reference = h.spec.as_ref().map(|s| {
                let r = &s.scale_target_ref;
                format!("{}/{}", r.kind.as_str(), r.name.as_str())
            });
            HorizontalPodAutoscalerItem {
                name: h.metadata.name.unwrap_or_default(),
                namespace: h.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                reference,
                replicas,
                creation_time: format_creation_time(h.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 PodDisruptionBudgets。
pub async fn list_pod_disruption_budgets(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<PodDisruptionBudgetItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<PodDisruptionBudget> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|p| {
            let status = p.status.as_ref();
            let allowed = status.map(|s| s.disruptions_allowed);
            let min_avail = p.spec.as_ref().and_then(|s| s.min_available.as_ref()).map(int_or_string_to_str);
            let max_unavail = p.spec.as_ref().and_then(|s| s.max_unavailable.as_ref()).map(int_or_string_to_str);
            PodDisruptionBudgetItem {
                name: p.metadata.name.unwrap_or_default(),
                namespace: p.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                min_available: min_avail,
                max_unavailable: max_unavail,
                allowed_disruptions: allowed,
                creation_time: format_creation_time(p.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}
