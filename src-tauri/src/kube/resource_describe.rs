//! 资源 Describe：按 kind 生成类 kubectl describe 的 Markdown 文档。
//! 表格（Non-terminated Pods、Events）内嵌于文档流，前端统一渲染 Markdown。

use crate::kube::resource_get;
use crate::kube::resources::ResourceError;
use chrono::{DateTime, Utc};
use kube::api::{Api, ListParams};
use kube::Client;
use k8s_openapi::api::core::v1::{Event, Pod};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DescribeResult {
    pub markdown: String,
}

/// 转义 Markdown 表格单元格中的 | 字符，避免破坏表格结构。
fn escape_table_cell(s: &str) -> String {
    s.replace('|', "&#124;")
}

/// 获取资源的 describe Markdown 文档。
pub async fn describe_resource(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<DescribeResult, ResourceError> {
    let yaml_str = resource_get::get_resource_yaml(client, kind, name, namespace).await?;
    let obj: serde_json::Value = serde_yaml::from_str(&yaml_str).map_err(|e| {
        ResourceError::Serialize(format!("yaml parse: {}", e))
    })?;

    let mut md = String::new();

    match kind {
        "Node" => append_node_describe(&mut md, &obj, client, name).await?,
        "Pod" => append_pod_describe(&mut md, &obj, client, name, namespace).await?,
        "Deployment" => append_deployment_describe(&mut md, &obj, client, name, namespace).await?,
        _ => {
            append_minimal_describe(&mut md, &obj, name, namespace);
            if let Some(ns) = namespace {
                let events = fetch_events(client, kind, name, ns).await.unwrap_or_default();
                append_events_table(&mut md, &events);
            }
        }
    }

    Ok(DescribeResult { markdown: md })
}

/// Node 的 describe：按 kubectl 语义块组织，表格内嵌。
async fn append_node_describe(
    md: &mut String,
    obj: &serde_json::Value,
    client: &Client,
    name: &str,
) -> Result<(), ResourceError> {
    let obj = obj.as_object().ok_or_else(|| {
        ResourceError::Serialize("invalid object".to_string())
    })?;

    let meta = obj.get("metadata").and_then(|v| v.as_object());
    let spec = obj.get("spec").and_then(|v| v.as_object());
    let status = obj.get("status").and_then(|v| v.as_object());

    md.push_str("## 基本信息\n\n");
    md.push_str("| 字段 | 值 |\n| --- | --- |\n");
    md.push_str(&format!("| Name | {} |\n", escape_table_cell(name)));
    if let Some(meta) = meta {
        if let Some(ns) = meta.get("namespace").and_then(|v| v.as_str()) {
            md.push_str(&format!("| Namespace | {} |\n", escape_table_cell(ns)));
        }
        if let Some(labels) = meta.get("labels").and_then(|v| v.as_object()) {
            if !labels.is_empty() {
                let parts: Vec<String> = labels
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
                    .collect();
                md.push_str(&format!("| Labels | {} |\n", escape_table_cell(&parts.join(", "))));
            }
        }
        if let Some(ann) = meta.get("annotations").and_then(|v| v.as_object()) {
            if !ann.is_empty() {
                let parts: Vec<String> = ann
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
                    .collect();
                md.push_str(&format!("| Annotations | {} |\n", escape_table_cell(&parts.join(", "))));
            }
        }
        if let Some(ts) = meta.get("creationTimestamp").and_then(|v| v.as_str()) {
            md.push_str(&format!("| CreationTimestamp | {} |\n", escape_table_cell(ts)));
        }
    }
    if let Some(spec) = spec {
        if let Some(taints) = spec.get("taints").and_then(|v| v.as_array()) {
            if !taints.is_empty() {
                let parts: Vec<String> = taints
                    .iter()
                    .filter_map(|t| {
                        let o = t.as_object()?;
                        let key = o.get("key").and_then(|v| v.as_str()).unwrap_or("");
                        let value = o.get("value").and_then(|v| v.as_str()).unwrap_or("");
                        let effect = o.get("effect").and_then(|v| v.as_str()).unwrap_or("");
                        Some(format!("{}={}:{}", key, value, effect))
                    })
                    .collect();
                md.push_str(&format!("| Taints | {} |\n", escape_table_cell(&parts.join(", "))));
            }
        }
    }
    md.push('\n');

    if let Some(status) = status {
        if let Some(conditions) = status.get("conditions").and_then(|v| v.as_array()) {
            md.push_str("## Conditions\n\n");
            md.push_str("| Type | Status | Reason | Message | LastHeartbeatTime | LastTransitionTime |\n");
            md.push_str("| --- | --- | --- | --- | --- | --- |\n");
            for c in conditions {
                let o = c.as_object();
                let type_ = o.and_then(|m| m.get("type")).and_then(|v| v.as_str()).unwrap_or("<none>");
                let status_val = o.and_then(|m| m.get("status")).and_then(|v| v.as_str()).unwrap_or("<none>");
                let reason = o.and_then(|m| m.get("reason")).and_then(|v| v.as_str()).unwrap_or("<none>");
                let message = o.and_then(|m| m.get("message")).and_then(|v| v.as_str()).unwrap_or("<none>");
                let last_hb = o.and_then(|m| m.get("lastHeartbeatTime")).and_then(|v| v.as_str()).unwrap_or("<none>");
                let last_trans = o.and_then(|m| m.get("lastTransitionTime")).and_then(|v| v.as_str()).unwrap_or("<none>");
                md.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    escape_table_cell(type_),
                    escape_table_cell(status_val),
                    escape_table_cell(reason),
                    escape_table_cell(message),
                    escape_table_cell(last_hb),
                    escape_table_cell(last_trans)
                ));
            }
            md.push('\n');
        }

        if let Some(addrs) = status.get("addresses").and_then(|v| v.as_array()) {
            if !addrs.is_empty() {
                md.push_str("## Addresses\n\n");
                md.push_str("| Type | Address |\n| --- | --- |\n");
                for a in addrs {
                    let o = a.as_object();
                    let type_ = o.and_then(|m| m.get("type")).and_then(|v| v.as_str()).unwrap_or("<none>");
                    let addr = o.and_then(|m| m.get("address")).and_then(|v| v.as_str()).unwrap_or("<none>");
                    md.push_str(&format!("| {} | {} |\n", escape_table_cell(type_), escape_table_cell(addr)));
                }
                md.push('\n');
            }
        }

        if let Some(cap) = status.get("capacity").and_then(|v| v.as_object()) {
            if !cap.is_empty() {
                md.push_str("## Capacity\n\n");
                md.push_str("| Resource | Quantity |\n| --- | --- |\n");
                for (k, v) in cap {
                    let q = v.as_str().map(String::from).unwrap_or_else(|| v.to_string());
                    md.push_str(&format!("| {} | {} |\n", escape_table_cell(k), escape_table_cell(&q)));
                }
                md.push('\n');
            }
        }

        if let Some(alloc) = status.get("allocatable").and_then(|v| v.as_object()) {
            if !alloc.is_empty() {
                md.push_str("## Allocatable\n\n");
                md.push_str("| Resource | Quantity |\n| --- | --- |\n");
                for (k, v) in alloc {
                    let q = v.as_str().map(String::from).unwrap_or_else(|| v.to_string());
                    md.push_str(&format!("| {} | {} |\n", escape_table_cell(k), escape_table_cell(&q)));
                }
                md.push('\n');
            }
        }

        if let Some(node_info) = status.get("nodeInfo").and_then(|v| v.as_object()) {
            md.push_str("## System Info\n\n");
            md.push_str("| Field | Value |\n| --- | --- |\n");
            for key in ["machineID", "systemUUID", "bootID", "kernelVersion", "osImage", "containerRuntimeVersion", "kubeletVersion", "architecture"] {
                if let Some(v) = node_info.get(key) {
                    let s = v.as_str().map(String::from).unwrap_or_else(|| v.to_string());
                    md.push_str(&format!("| {} | {} |\n", escape_table_cell(key), escape_table_cell(&s)));
                }
            }
            md.push('\n');
        }
    }

    let pods = fetch_node_pods(client, name).await?;
    md.push_str(&format!("## Non-terminated Pods ({} in total)\n\n", pods.len()));
    md.push_str("| Namespace | Name | CPU Requests | CPU Limits | Memory Requests | Memory Limits | Age |\n");
    md.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for p in &pods {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            escape_table_cell(&p.namespace),
            escape_table_cell(&p.name),
            escape_table_cell(&p.cpu_req),
            escape_table_cell(&p.cpu_lim),
            escape_table_cell(&p.mem_req),
            escape_table_cell(&p.mem_lim),
            escape_table_cell(&p.age)
        ));
    }
    md.push('\n');

    let events = fetch_events_for_node(client, name).await.unwrap_or_default();
    append_events_table(md, &events);

    Ok(())
}

struct NodePodRow {
    namespace: String,
    name: String,
    cpu_req: String,
    cpu_lim: String,
    mem_req: String,
    mem_lim: String,
    age: String,
}

async fn fetch_node_pods(client: &Client, node_name: &str) -> Result<Vec<NodePodRow>, ResourceError> {
    let api: Api<Pod> = Api::all(client.clone());
    let params = ListParams::default().fields(&format!("spec.nodeName={}", node_name));
    let list = api.list(&params).await.map_err(ResourceError::Kube)?;

    let mut pods: Vec<_> = list
        .items
        .into_iter()
        .filter(|p| {
            let phase = p.status.as_ref().and_then(|s| s.phase.as_deref()).unwrap_or("");
            phase != "Succeeded" && phase != "Failed"
        })
        .collect();

    pods.sort_by(|a, b| {
        let ns_a = a.metadata.namespace.as_deref().unwrap_or("");
        let ns_b = b.metadata.namespace.as_deref().unwrap_or("");
        ns_a.cmp(ns_b).then_with(|| {
            let name_a = a.metadata.name.as_deref().unwrap_or("");
            let name_b = b.metadata.name.as_deref().unwrap_or("");
            name_a.cmp(name_b)
        })
    });

    let items: Vec<NodePodRow> = pods
        .iter()
        .map(|p| {
            let (cpu_req, cpu_lim, mem_req, mem_lim) = sum_pod_resources(p);
            NodePodRow {
                namespace: p.metadata.namespace.as_deref().unwrap_or("<none>").to_string(),
                name: p.metadata.name.as_deref().unwrap_or("<none>").to_string(),
                cpu_req,
                cpu_lim,
                mem_req,
                mem_lim,
                age: pod_age(p),
            }
        })
        .collect();
    Ok(items)
}

fn sum_pod_resources(pod: &Pod) -> (String, String, String, String) {
    let mut cpu_req = 0i64;
    let mut cpu_lim = 0i64;
    let mut mem_req = 0i64;
    let mut mem_lim = 0i64;

    let containers = pod
        .spec
        .as_ref()
        .map(|s| {
            let mut c = s.containers.clone();
            if let Some(init) = &s.init_containers {
                c.extend(init.clone());
            }
            c
        })
        .unwrap_or_default();

    for c in &containers {
        if let Some(r) = &c.resources {
            if let Some(req) = &r.requests {
                cpu_req += req.get("cpu").map(|q| parse_cpu_millis(&q.0)).unwrap_or(0);
                mem_req += req.get("memory").map(|q| parse_mem_bytes(&q.0)).unwrap_or(0);
            }
            if let Some(lim) = &r.limits {
                cpu_lim += lim.get("cpu").map(|q| parse_cpu_millis(&q.0)).unwrap_or(0);
                mem_lim += lim.get("memory").map(|q| parse_mem_bytes(&q.0)).unwrap_or(0);
            }
        }
    }

    (
        format_cpu(cpu_req),
        format_cpu(cpu_lim),
        format_mem(mem_req),
        format_mem(mem_lim),
    )
}

fn parse_cpu_millis(s: &str) -> i64 {
    let s = s.trim();
    if s.is_empty() {
        return 0;
    }
    if let Some(rest) = s.strip_suffix('m') {
        if let Ok(n) = rest.parse::<i64>() {
            return n;
        }
    }
    if let Ok(n) = s.parse::<f64>() {
        return (n * 1000.0) as i64;
    }
    0
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
    } else if let Some(rest) = s.strip_suffix("K") {
        (rest.parse::<f64>().unwrap_or(0.0), 1000)
    } else if let Some(rest) = s.strip_suffix("M") {
        (rest.parse::<f64>().unwrap_or(0.0), 1000 * 1000)
    } else if let Some(rest) = s.strip_suffix("G") {
        (rest.parse::<f64>().unwrap_or(0.0), 1000 * 1000 * 1000)
    } else if let Ok(n) = s.parse::<i64>() {
        return n;
    } else {
        return 0;
    };
    (num * unit as f64) as i64
}

fn format_cpu(millis: i64) -> String {
    if millis == 0 {
        "0".to_string()
    } else if millis % 1000 == 0 {
        format!("{}", millis / 1000)
    } else {
        format!("{}m", millis)
    }
}

fn format_mem(bytes: i64) -> String {
    if bytes == 0 {
        "0".to_string()
    } else if bytes >= 1024 * 1024 * 1024 {
        format!("{}Gi", bytes / (1024 * 1024 * 1024))
    } else if bytes >= 1024 * 1024 {
        format!("{}Mi", bytes / (1024 * 1024))
    } else if bytes >= 1024 {
        format!("{}Ki", bytes / 1024)
    } else {
        format!("{}", bytes)
    }
}

fn pod_age(pod: &Pod) -> String {
    let ts = match &pod.metadata.creation_timestamp {
        Some(t) => t.0,
        None => return "<unknown>".to_string(),
    };
    let now: DateTime<Utc> = Utc::now();
    let d = now.signed_duration_since(ts);
    if d.num_days() > 0 {
        format!("{}d", d.num_days())
    } else if d.num_hours() > 0 {
        format!("{}h", d.num_hours())
    } else if d.num_minutes() > 0 {
        format!("{}m", d.num_minutes())
    } else {
        format!("{}s", d.num_seconds())
    }
}

/// Pod 的 describe：按 kubectl 语义块组织，无 spec/status 原始 dump。
async fn append_pod_describe(
    md: &mut String,
    obj: &serde_json::Value,
    client: &Client,
    name: &str,
    namespace: Option<&str>,
) -> Result<(), ResourceError> {
    let obj = obj.as_object().ok_or_else(|| ResourceError::Serialize("invalid object".to_string()))?;
    let meta = obj.get("metadata").and_then(|v| v.as_object());
    let spec = obj.get("spec").and_then(|v| v.as_object());
    let status = obj.get("status").and_then(|v| v.as_object());

    md.push_str("## 基本信息\n\n");
    md.push_str("| 字段 | 值 |\n| --- | --- |\n");
    md.push_str(&format!("| Name | {} |\n", escape_table_cell(name)));
    if let Some(meta) = meta {
        if let Some(ns) = meta.get("namespace").and_then(|v| v.as_str()) {
            md.push_str(&format!("| Namespace | {} |\n", escape_table_cell(ns)));
        }
        if let Some(prio) = meta.get("priority").and_then(|v| v.as_i64()) {
            md.push_str(&format!("| Priority | {} |\n", prio));
        }
        if let Some(labels) = meta.get("labels").and_then(|v| v.as_object()) {
            if !labels.is_empty() {
                let parts: Vec<String> = labels.iter().map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or(""))).collect();
                md.push_str(&format!("| Labels | {} |\n", escape_table_cell(&parts.join(", "))));
            }
        }
        if let Some(ann) = meta.get("annotations").and_then(|v| v.as_object()) {
            if !ann.is_empty() {
                let parts: Vec<String> = ann.iter().map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or(""))).collect();
                md.push_str(&format!("| Annotations | {} |\n", escape_table_cell(&parts.join(", "))));
            }
        }
        if let Some(ts) = meta.get("creationTimestamp").and_then(|v| v.as_str()) {
            md.push_str(&format!("| CreationTimestamp | {} |\n", escape_table_cell(ts)));
        }
    }
    if let Some(spec) = spec {
        if let Some(node) = spec.get("nodeName").and_then(|v| v.as_str()) {
            md.push_str(&format!("| Node | {} |\n", escape_table_cell(node)));
        }
    }
    if let Some(status) = status {
        if let Some(phase) = status.get("phase").and_then(|v| v.as_str()) {
            md.push_str(&format!("| Status | {} |\n", escape_table_cell(phase)));
        }
        if let Some(ip) = status.get("podIP").and_then(|v| v.as_str()) {
            md.push_str(&format!("| Pod IP | {} |\n", escape_table_cell(ip)));
        }
    }
    md.push('\n');

    if let Some(conditions) = status.as_ref().and_then(|s| s.get("conditions")).and_then(|v| v.as_array()) {
        md.push_str("## Conditions\n\n");
        md.push_str("| Type | Status | Reason | Message | LastTransitionTime |\n| --- | --- | --- | --- | --- |\n");
        for c in conditions {
            let o = c.as_object();
            let type_ = o.and_then(|m| m.get("type")).and_then(|v| v.as_str()).unwrap_or("<none>");
            let status_val = o.and_then(|m| m.get("status")).and_then(|v| v.as_str()).unwrap_or("<none>");
            let reason = o.and_then(|m| m.get("reason")).and_then(|v| v.as_str()).unwrap_or("<none>");
            let message = o.and_then(|m| m.get("message")).and_then(|v| v.as_str()).unwrap_or("<none>");
            let last_trans = o.and_then(|m| m.get("lastTransitionTime")).and_then(|v| v.as_str()).unwrap_or("<none>");
            md.push_str(&format!("| {} | {} | {} | {} | {} |\n",
                escape_table_cell(type_), escape_table_cell(status_val), escape_table_cell(reason),
                escape_table_cell(message), escape_table_cell(last_trans)));
        }
        md.push('\n');
    }

    if let Some(spec) = spec {
        if let Some(containers) = spec.get("containers").and_then(|v| v.as_array()) {
            md.push_str("## Containers\n\n");
            md.push_str("| Name | Image | Ports | State | Ready | Restarts | Limits | Requests |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n");
            let status_containers = status.and_then(|s| s.get("containerStatuses")).and_then(|v| v.as_array());
            for (i, c) in containers.iter().enumerate() {
                let o = c.as_object();
                let cname = o.and_then(|m| m.get("name")).and_then(|v| v.as_str()).unwrap_or("<none>");
                let image = o.and_then(|m| m.get("image")).and_then(|v| v.as_str()).unwrap_or("<none>");
                let ports = o.and_then(|m| m.get("ports")).and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|p| p.as_object().and_then(|m| m.get("containerPort")).and_then(|v| v.as_i64()).map(|n| n.to_string())).collect::<Vec<_>>().join(", "))
                    .unwrap_or_else(|| "<none>".to_string());
                let (state, ready, restarts) = status_containers.and_then(|sc| sc.get(i)).and_then(|cs| {
                    let cs = cs.as_object()?;
                    let state = cs.get("state").and_then(|s| {
                        let s = s.as_object()?;
                        if s.get("running").is_some() { Some("Running") }
                        else if s.get("waiting").is_some() { Some("Waiting") }
                        else if s.get("terminated").is_some() { Some("Terminated") }
                        else { None }
                    }).unwrap_or("<none>").to_string();
                    let ready = cs.get("ready").and_then(|v| v.as_bool()).map(|b| if b { "True" } else { "False" }).unwrap_or("<none>").to_string();
                    let restarts = cs.get("restartCount").and_then(|v| v.as_i64()).map(|n| n.to_string()).unwrap_or_else(|| "0".to_string());
                    Some((state, ready, restarts))
                }).unwrap_or_else(|| ("<none>".to_string(), "<none>".to_string(), "0".to_string()));
                let (limits, requests) = o.and_then(|m| m.get("resources")).map(|r| {
                    let lim = r.as_object().and_then(|m| m.get("limits")).and_then(format_resources).unwrap_or_else(|| "0".to_string());
                    let req = r.as_object().and_then(|m| m.get("requests")).and_then(format_resources).unwrap_or_else(|| "0".to_string());
                    (lim, req)
                }).unwrap_or_else(|| ("0".to_string(), "0".to_string()));
                md.push_str(&format!("| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                    escape_table_cell(cname), escape_table_cell(image), escape_table_cell(&ports),
                    escape_table_cell(&state), escape_table_cell(&ready), escape_table_cell(&restarts),
                    escape_table_cell(&limits), escape_table_cell(&requests)));
            }
            md.push('\n');
        }
    }

    let ns = namespace.unwrap_or("default");
    let events = fetch_events(client, "Pod", name, ns).await.unwrap_or_default();
    append_events_table(md, &events);
    Ok(())
}

fn format_resources(v: &serde_json::Value) -> Option<String> {
    let o = v.as_object()?;
    let parts: Vec<String> = o.iter()
        .filter_map(|(k, v)| v.as_str().map(|s| format!("{}={}", k, s)))
        .collect();
    if parts.is_empty() { None } else { Some(parts.join(", ")) }
}

/// Deployment 的 describe：按 kubectl 语义块组织，无 spec/status 原始 dump。
async fn append_deployment_describe(
    md: &mut String,
    obj: &serde_json::Value,
    client: &Client,
    name: &str,
    namespace: Option<&str>,
) -> Result<(), ResourceError> {
    let obj = obj.as_object().ok_or_else(|| ResourceError::Serialize("invalid object".to_string()))?;
    let meta = obj.get("metadata").and_then(|v| v.as_object());
    let spec = obj.get("spec").and_then(|v| v.as_object());
    let status = obj.get("status").and_then(|v| v.as_object());

    md.push_str("## 基本信息\n\n");
    md.push_str("| 字段 | 值 |\n| --- | --- |\n");
    md.push_str(&format!("| Name | {} |\n", escape_table_cell(name)));
    if let Some(meta) = meta {
        if let Some(ns) = meta.get("namespace").and_then(|v| v.as_str()) {
            md.push_str(&format!("| Namespace | {} |\n", escape_table_cell(ns)));
        }
        if let Some(labels) = meta.get("labels").and_then(|v| v.as_object()) {
            if !labels.is_empty() {
                let parts: Vec<String> = labels.iter().map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or(""))).collect();
                md.push_str(&format!("| Labels | {} |\n", escape_table_cell(&parts.join(", "))));
            }
        }
        if let Some(ts) = meta.get("creationTimestamp").and_then(|v| v.as_str()) {
            md.push_str(&format!("| CreationTimestamp | {} |\n", escape_table_cell(ts)));
        }
    }
    if let Some(spec) = spec {
        if let Some(sel) = spec.get("selector").and_then(|v| v.as_object()) {
            if let Some(match_labels) = sel.get("matchLabels").and_then(|v| v.as_object()) {
                let parts: Vec<String> = match_labels.iter().map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or(""))).collect();
                md.push_str(&format!("| Selector | {} |\n", escape_table_cell(&parts.join(", "))));
            }
        }
    }
    md.push('\n');

    md.push_str("## Replicas\n\n");
    let (desired, updated, total, available, unavailable) = if let Some(status) = status {
        let desired = spec.and_then(|s| s.get("replicas")).and_then(|v| v.as_i64()).unwrap_or(0);
        let updated = status.get("updatedReplicas").and_then(|v| v.as_i64()).unwrap_or(0);
        let total = status.get("replicas").and_then(|v| v.as_i64()).unwrap_or(0);
        let available = status.get("availableReplicas").and_then(|v| v.as_i64()).unwrap_or(0);
        let unavailable = status.get("unavailableReplicas").and_then(|v| v.as_i64()).unwrap_or(0);
        (desired, updated, total, available, unavailable)
    } else {
        (0, 0, 0, 0, 0)
    };
    md.push_str(&format!("| Desired | Updated | Total | Available | Unavailable |\n| --- | --- | --- | --- | --- |\n| {} | {} | {} | {} | {} |\n\n",
        desired, updated, total, available, unavailable));

    if let Some(spec) = spec {
        if let Some(strategy) = spec.get("strategy").and_then(|v| v.as_object()) {
            let strategy_type = strategy.get("type").and_then(|v| v.as_str()).unwrap_or("RollingUpdate");
            md.push_str("## Strategy\n\n");
            md.push_str("| Type | Max Unavailable | Max Surge |\n| --- | --- | --- |\n");
            let (max_unavail, max_surge) = strategy.get("rollingUpdate").and_then(|r| r.as_object()).map(|r| {
                let u = r.get("maxUnavailable").and_then(|v| v.as_str()).unwrap_or("<none>");
                let s = r.get("maxSurge").and_then(|v| v.as_str()).unwrap_or("<none>");
                (u, s)
            }).unwrap_or(("<none>", "<none>"));
            md.push_str(&format!("| {} | {} | {} |\n\n", escape_table_cell(strategy_type), escape_table_cell(max_unavail), escape_table_cell(max_surge)));
        }
    }

    if let Some(spec) = spec {
        if let Some(template) = spec.get("template").and_then(|v| v.as_object()) {
            md.push_str("## Pod Template\n\n");
            if let Some(labels) = template.get("metadata").and_then(|v| v.as_object()).and_then(|m| m.get("labels")).and_then(|v| v.as_object()) {
                let parts: Vec<String> = labels.iter().map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or(""))).collect();
                md.push_str(&format!("**Labels:** {}\n\n", parts.join(", ")));
            }
            if let Some(containers) = template.get("spec").and_then(|v| v.as_object()).and_then(|s| s.get("containers")).and_then(|v| v.as_array()) {
                md.push_str("| Container | Image |\n| --- | --- |\n");
                for c in containers {
                    let o = c.as_object();
                    let cname = o.and_then(|m| m.get("name")).and_then(|v| v.as_str()).unwrap_or("<none>");
                    let image = o.and_then(|m| m.get("image")).and_then(|v| v.as_str()).unwrap_or("<none>");
                    md.push_str(&format!("| {} | {} |\n", escape_table_cell(cname), escape_table_cell(image)));
                }
                md.push('\n');
            }
        }
    }

    let ns = namespace.unwrap_or("default");
    let events = fetch_events(client, "Deployment", name, ns).await.unwrap_or_default();
    append_events_table(md, &events);
    Ok(())
}

/// 通用资源：仅基本信息，不输出 spec/status 原始内容。
fn append_minimal_describe(md: &mut String, obj: &serde_json::Value, name: &str, namespace: Option<&str>) {
    let obj = match obj.as_object() {
        Some(o) => o,
        None => return,
    };

    md.push_str("## 基本信息\n\n");
    md.push_str("| 字段 | 值 |\n| --- | --- |\n");
    md.push_str(&format!("| Name | {} |\n", escape_table_cell(name)));
    if let Some(ns) = namespace {
        md.push_str(&format!("| Namespace | {} |\n", escape_table_cell(ns)));
    }
    if let Some(meta) = obj.get("metadata").and_then(|v| v.as_object()) {
        if let Some(labels) = meta.get("labels").and_then(|v| v.as_object()) {
            if !labels.is_empty() {
                let parts: Vec<String> = labels.iter().map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or(""))).collect();
                md.push_str(&format!("| Labels | {} |\n", escape_table_cell(&parts.join(", "))));
            }
        }
        if let Some(ts) = meta.get("creationTimestamp").and_then(|v| v.as_str()) {
            md.push_str(&format!("| CreationTimestamp | {} |\n", escape_table_cell(ts)));
        }
    }
    md.push('\n');
}

struct EventRow {
    type_: String,
    reason: String,
    age: String,
    from: String,
    message: String,
}

async fn fetch_events(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: &str,
) -> Result<Vec<EventRow>, ResourceError> {
    let api: Api<Event> = Api::namespaced(client.clone(), namespace);
    let list = api.list(&ListParams::default()).await.map_err(ResourceError::Kube)?;
    let mut events: Vec<_> = list
        .items
        .into_iter()
        .filter(|e| {
            e.involved_object.kind.as_deref() == Some(kind)
                && e.involved_object.name.as_deref() == Some(name)
        })
        .collect();
    events.sort_by(|a, b| {
        let ta = a.last_timestamp.as_ref().map(|t| t.0);
        let tb = b.last_timestamp.as_ref().map(|t| t.0);
        std::cmp::Ord::cmp(&ta, &tb)
    });
    Ok(events_to_rows(&events))
}

async fn fetch_events_for_node(client: &Client, name: &str) -> Result<Vec<EventRow>, ResourceError> {
    let api: Api<Event> = Api::namespaced(client.clone(), "default");
    let list = api.list(&ListParams::default()).await.map_err(ResourceError::Kube)?;
    let mut events: Vec<_> = list
        .items
        .into_iter()
        .filter(|e| {
            e.involved_object.kind.as_deref() == Some("Node")
                && e.involved_object.name.as_deref() == Some(name)
        })
        .collect();
    events.sort_by(|a, b| {
        let ta = a.last_timestamp.as_ref().map(|t| t.0);
        let tb = b.last_timestamp.as_ref().map(|t| t.0);
        std::cmp::Ord::cmp(&ta, &tb)
    });
    Ok(events_to_rows(&events))
}

fn events_to_rows(events: &[Event]) -> Vec<EventRow> {
    events
        .iter()
        .rev()
        .take(50)
        .map(|e| {
            let from = e
                .source
                .as_ref()
                .and_then(|s| s.component.as_ref().or(s.host.as_ref()))
                .map(|s| s.as_str())
                .unwrap_or("<none>");
            EventRow {
                type_: e.type_.as_deref().unwrap_or("<none>").to_string(),
                reason: e.reason.as_deref().unwrap_or("<none>").to_string(),
                age: e
                    .last_timestamp
                    .as_ref()
                    .map(|t| format!("{}", t.0))
                    .unwrap_or_else(|| "<unknown>".to_string()),
                from: from.to_string(),
                message: e.message.as_deref().unwrap_or("<none>").to_string(),
            }
        })
        .collect()
}

fn append_events_table(md: &mut String, events: &[EventRow]) {
    md.push_str("## Events\n\n");
    if events.is_empty() {
        md.push_str("*暂无事件*\n\n");
        return;
    }
    md.push_str("| Type | Reason | Age | From | Message |\n");
    md.push_str("| --- | --- | --- | --- | --- |\n");
    for e in events {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            escape_table_cell(&e.type_),
            escape_table_cell(&e.reason),
            escape_table_cell(&e.age),
            escape_table_cell(&e.from),
            escape_table_cell(&e.message)
        ));
    }
    md.push('\n');
}
