//! 集群基础资源：Namespace、Node。
//! 包含所有 CPU/内存/GPU 容量解析与格式化工具，供 resource_watch 复用。

use super::{build_list_params, format_creation_time, ResourceError};
use k8s_openapi::api::core::v1::{Namespace, Node, Pod};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::api::{Api, ListParams};
use kube::Client;
use serde::Serialize;
use std::collections::HashSet;

// ── 数据结构 ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct NamespaceItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taint_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_total: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_total: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_total: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_requests: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_requests: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_requests: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

// ── CPU/内存/GPU 解析工具 ──────────────────────────────────────────────────

fn parse_cpu_millis(s: &str) -> i64 {
    let s = s.trim();
    if s.is_empty() {
        return 0;
    }
    if let Some(rest) = s.strip_suffix('m') {
        return rest.parse::<i64>().unwrap_or(0);
    }
    s.parse::<f64>().map(|n| (n * 1000.0) as i64).unwrap_or(0)
}

fn parse_mem_bytes(s: &str) -> i64 {
    let s = s.trim();
    if s.is_empty() {
        return 0;
    }
    let (num, unit) = if let Some(rest) = s.strip_suffix("Ki") {
        (rest.parse::<f64>().unwrap_or(0.0), 1024i64)
    } else if let Some(rest) = s.strip_suffix("Mi") {
        (rest.parse::<f64>().unwrap_or(0.0), 1024 * 1024)
    } else if let Some(rest) = s.strip_suffix("Gi") {
        (rest.parse::<f64>().unwrap_or(0.0), 1024 * 1024 * 1024)
    } else if let Some(rest) = s.strip_suffix("Ti") {
        (rest.parse::<f64>().unwrap_or(0.0), 1024_i64.pow(4))
    } else if let Some(rest) = s.strip_suffix("Pi") {
        (rest.parse::<f64>().unwrap_or(0.0), 1024_i64.pow(5))
    } else if let Some(rest) = s.strip_suffix('K') {
        (rest.parse::<f64>().unwrap_or(0.0), 1000)
    } else if let Some(rest) = s.strip_suffix('M') {
        (rest.parse::<f64>().unwrap_or(0.0), 1000 * 1000)
    } else if let Some(rest) = s.strip_suffix('G') {
        (rest.parse::<f64>().unwrap_or(0.0), 1000 * 1000 * 1000)
    } else if let Some(rest) = s.strip_suffix('T') {
        (rest.parse::<f64>().unwrap_or(0.0), 1000_i64.pow(4))
    } else if let Some(rest) = s.strip_suffix('P') {
        (rest.parse::<f64>().unwrap_or(0.0), 1000_i64.pow(5))
    } else if let Ok(n) = s.parse::<i64>() {
        return n;
    } else {
        return 0;
    };
    (num * unit as f64) as i64
}

pub(crate) fn quantity_cpu_millis(q: Option<&Quantity>) -> i64 {
    q.map(|v| parse_cpu_millis(&v.0)).unwrap_or(0)
}

pub(crate) fn quantity_mem_bytes(q: Option<&Quantity>) -> i64 {
    q.map(|v| parse_mem_bytes(&v.0)).unwrap_or(0)
}

pub(crate) fn quantity_scalar_units(q: Option<&Quantity>) -> i64 {
    q.and_then(|v| v.0.trim().parse::<i64>().ok()).unwrap_or(0)
}

pub(crate) fn format_cpu(millis: i64) -> String {
    if millis == 0 {
        "0".to_string()
    } else if millis % 1000 == 0 {
        format!("{}", millis / 1000)
    } else {
        format!("{}m", millis)
    }
}

pub(crate) fn format_cpu_total(millis: i64) -> String {
    if millis <= 0 {
        return "0".to_string();
    }
    let cores = (millis as f64) / 1000.0;
    if (cores.fract()).abs() < f64::EPSILON {
        format!("{}", cores as i64)
    } else {
        let text = format!("{cores:.3}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

pub(crate) fn format_mem(bytes: i64) -> String {
    if bytes == 0 {
        "0".to_string()
    } else if bytes >= 1024_i64.pow(4) {
        format!("{}Ti", bytes / 1024_i64.pow(4))
    } else if bytes >= 1024 * 1024 * 1024 {
        format!("{}Gi", bytes / (1024 * 1024 * 1024))
    } else if bytes >= 1024 * 1024 {
        format!("{}Mi", bytes / (1024 * 1024))
    } else if bytes >= 1024 {
        format!("{}Ki", bytes / 1024)
    } else {
        bytes.to_string()
    }
}

fn format_request_ratio(used: i64, total: i64, formatter: fn(i64) -> String) -> Option<String> {
    if total <= 0 {
        return None;
    }
    let percent = ((used as f64 / total as f64) * 100.0).round() as i64;
    Some(format!("{} / {} ({}%)", formatter(used), formatter(total), percent.clamp(0, 999)))
}

fn is_gpu_resource_name(name: &str, gpu_resource_names: &HashSet<String>) -> bool {
    let name = name.trim().to_lowercase();
    gpu_resource_names.iter().any(|pattern| {
        if let Some(suffix) = pattern.strip_prefix('*') {
            name.ends_with(suffix)
        } else {
            name == *pattern
        }
    })
}

pub(crate) fn format_gpu(units: i64) -> String {
    units.to_string()
}

// ── list 函数 ──────────────────────────────────────────────────────────────

/// 列出集群 Namespace（集群级资源）。
pub async fn list_namespaces(
    client: &Client,
    label_selector: Option<&str>,
) -> Result<Vec<NamespaceItem>, ResourceError> {
    let api: Api<Namespace> = Api::all(client.clone());
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|n| NamespaceItem {
            name: n.metadata.name.unwrap_or_default(),
            creation_time: format_creation_time(n.metadata.creation_timestamp.as_ref()),
        })
        .collect();
    Ok(items)
}

/// 列出集群 Node（集群级资源）。
pub async fn list_nodes(
    client: &Client,
    label_selector: Option<&str>,
    gpu_resource_names: &[String],
) -> Result<Vec<NodeItem>, ResourceError> {
    let api: Api<Node> = Api::all(client.clone());
    let pods_api: Api<Pod> = Api::all(client.clone());
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let pod_list = pods_api.list(&ListParams::default()).await.map_err(ResourceError::Kube)?;
    let gpu_resource_names: HashSet<String> = gpu_resource_names
        .iter()
        .map(|name| name.trim().to_lowercase())
        .filter(|name| !name.is_empty())
        .collect();
    let items = list
        .items
        .into_iter()
        .map(|n| {
            let node_name = n.metadata.name.clone().unwrap_or_default();
            let status = n.status.as_ref().and_then(|s| {
                s.conditions.as_ref().and_then(|conds| {
                    conds.iter()
                        .find(|c| c.type_ == "Ready")
                        .map(|c| if c.status == "True" { "Ready".to_string() } else { "NotReady".to_string() })
                })
            });
            let internal_ip = n.status.as_ref().and_then(|s| {
                s.addresses.as_ref().and_then(|addrs| {
                    addrs.iter()
                        .find(|a| a.type_ == "InternalIP")
                        .map(|a| a.address.clone())
                })
            });
            let taint_count = n
                .spec
                .as_ref()
                .and_then(|spec| spec.taints.as_ref())
                .map(|taints| taints.len() as u32)
                .unwrap_or(0);
            let alloc_cpu = n
                .status
                .as_ref()
                .and_then(|s| s.allocatable.as_ref())
                .map(|m| quantity_cpu_millis(m.get("cpu")))
                .unwrap_or(0);
            let alloc_mem = n
                .status
                .as_ref()
                .and_then(|s| s.allocatable.as_ref())
                .map(|m| quantity_mem_bytes(m.get("memory")))
                .unwrap_or(0);
            let alloc_gpu = n
                .status
                .as_ref()
                .and_then(|s| s.allocatable.as_ref())
                .map(|m| {
                    m.iter()
                        .filter(|(name, _)| is_gpu_resource_name(name, &gpu_resource_names))
                        .map(|(_, quantity)| quantity_scalar_units(Some(quantity)))
                        .sum::<i64>()
                })
                .unwrap_or(0);
            let (cpu_req_sum, mem_req_sum, gpu_req_sum) = pod_list.items.iter().fold((0i64, 0i64, 0i64), |acc, pod| {
                let assigned = pod
                    .spec
                    .as_ref()
                    .and_then(|s| s.node_name.as_deref())
                    .map(|name| name == node_name)
                    .unwrap_or(false);
                if !assigned {
                    return acc;
                }
                let phase = pod.status.as_ref().and_then(|s| s.phase.as_deref()).unwrap_or("");
                if matches!(phase, "Succeeded" | "Failed") {
                    return acc;
                }
                let mut cpu = 0i64;
                let mut mem = 0i64;
                let mut gpu = 0i64;
                if let Some(spec) = &pod.spec {
                    for container in &spec.containers {
                        if let Some(resources) = &container.resources {
                            if let Some(req) = &resources.requests {
                                cpu += quantity_cpu_millis(req.get("cpu"));
                                mem += quantity_mem_bytes(req.get("memory"));
                            }
                            if let Some(limits) = &resources.limits {
                                gpu += limits
                                    .iter()
                                    .filter(|(name, _)| is_gpu_resource_name(name, &gpu_resource_names))
                                    .map(|(_, quantity)| quantity_scalar_units(Some(quantity)))
                                    .sum::<i64>();
                            }
                        }
                    }
                    if let Some(overhead) = &spec.overhead {
                        cpu += quantity_cpu_millis(overhead.get("cpu"));
                        mem += quantity_mem_bytes(overhead.get("memory"));
                    }
                }
                (acc.0 + cpu, acc.1 + mem, acc.2 + gpu)
            });
            NodeItem {
                name: node_name,
                status,
                taint_count: Some(taint_count),
                internal_ip,
                cpu_total: (alloc_cpu > 0).then(|| format_cpu_total(alloc_cpu)),
                memory_total: (alloc_mem > 0).then(|| format_mem(alloc_mem)),
                gpu_total: (alloc_gpu > 0).then(|| format_gpu(alloc_gpu)),
                cpu_requests: format_request_ratio(cpu_req_sum, alloc_cpu, format_cpu),
                memory_requests: format_request_ratio(mem_req_sum, alloc_mem, format_mem),
                gpu_requests: format_request_ratio(gpu_req_sum, alloc_gpu, format_gpu),
                creation_time: format_creation_time(n.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}
