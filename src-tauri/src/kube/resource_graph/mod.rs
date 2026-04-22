//! 资源关联图：以任意资源为根，通过 BFS 构建完整的多跳关联图。
//! 用于编排资源收集、拓扑展示、关联跳转三种场景。

pub mod extractor;
pub mod registry;
pub mod extractors {
    pub mod workload_mounts;
    pub mod owner_ref;
    pub mod selector;
    pub mod service_selector;
    pub mod service_account;
    pub mod pvc_bindings;
    pub mod ingress_backend;
    pub mod service_reverse;
    pub mod hpa_ref;
    pub mod rbac_refs;
    pub mod sa_bindings_reverse;
}

use crate::kube::resource_get::get_resource_value;
use crate::kube::resources::ResourceError;
use extractor::RelationExtractor;
use futures::future::join_all;
use kube::Client;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};

// ── 数据结构 ──────────────────────────────────────────────────────────────────

/// 资源的唯一标识。集群级资源 namespace 为 None。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ResourceRef {
    pub kind: String,
    pub namespace: Option<String>,
    pub name: String,
}

impl ResourceRef {
    pub fn new(kind: impl Into<String>, namespace: Option<String>, name: impl Into<String>) -> Self {
        Self { kind: kind.into(), namespace, name: name.into() }
    }
}

/// 图中的节点。
#[derive(Debug, Clone, Serialize)]
pub struct ResourceNode {
    pub resource_ref: ResourceRef,
    pub depth: u32,
    /// true = 具名资源；false = 聚合节点（如 "Pods (3)"，由 label selector 展开）
    pub is_concrete: bool,
    /// 聚合节点的 label selector，供前端跳转列表使用
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_selector: Option<String>,
}

/// 关联类型枚举。
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    // A类：YAML 声明引用
    Volume,
    EnvFrom,
    EnvValue,
    ImagePullSecret,
    ServiceAccountRef,
    // B类：K8s 管理关系
    OwnerRef,
    Manages,
    // C类：Label Selector 绑定
    Selector,
    ServiceSelector,
    HpaTarget,
    // D类：名称精确引用
    BoundVolume,
    StorageClass,
    IngressBackend,
    RoleRef,
    // E类：反向派生
    Routes,
    ScaledBy,
}

/// 图中的有向边。
#[derive(Debug, Clone, Serialize)]
pub struct ResourceEdge {
    pub from: ResourceRef,
    pub to: ResourceRef,
    pub relation_type: RelationType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_selector: Option<String>,
}

/// 完整的资源关联图。
#[derive(Debug, Serialize)]
pub struct ResourceGraph {
    pub root: ResourceRef,
    /// BFS 顺序排列，root 在第一位
    pub nodes: Vec<ResourceNode>,
    pub edges: Vec<ResourceEdge>,
}

// ── 叶节点定义 ────────────────────────────────────────────────────────────────

fn is_leaf_kind(kind: &str) -> bool {
    matches!(kind, "PersistentVolume" | "StorageClass" | "Node" | "IngressClass")
}

// ── 工具函数（供 extractors 使用） ────────────────────────────────────────────

/// 将 spec.selector（LabelSelector）转为 K8s API 接受的字符串。
pub fn selector_to_string(sel: &serde_json::Value) -> Option<String> {
    let obj = sel.as_object()?;
    let mut parts: Vec<String> = Vec::new();

    if let Some(ml) = obj.get("matchLabels").and_then(|v| v.as_object()) {
        for (k, v) in ml {
            parts.push(format!("{}={}", k, v.as_str().unwrap_or("")));
        }
    }
    if let Some(me) = obj.get("matchExpressions").and_then(|v| v.as_array()) {
        for expr in me {
            let obj2 = match expr.as_object() { Some(o) => o, None => continue };
            let key = match expr.get("key").and_then(|v| v.as_str()) { Some(k) => k, None => continue };
            let op = obj2.get("operator").and_then(|v| v.as_str()).unwrap_or("");
            let vals = expr.get("values").and_then(|v| v.as_array());
            match op {
                "In" => {
                    if let Some(arr) = vals {
                        let vs: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                        if !vs.is_empty() { parts.push(format!("{} in ({})", key, vs.join(","))); }
                    }
                }
                "NotIn" => {
                    if let Some(arr) = vals {
                        let vs: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                        if !vs.is_empty() { parts.push(format!("{} notin ({})", key, vs.join(","))); }
                    }
                }
                "Exists" => parts.push(key.to_string()),
                "DoesNotExist" => parts.push(format!("!{}", key)),
                _ => {}
            }
        }
    }
    if parts.is_empty() { None } else { Some(parts.join(",")) }
}

/// 将简单 map（如 Service.spec.selector）转为 label selector 字符串。
pub fn simple_map_to_selector(map: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    if map.is_empty() { return None; }
    let parts: Vec<String> = map.iter()
        .filter_map(|(k, v)| v.as_str().map(|s| format!("{}={}", k, s)))
        .collect();
    if parts.is_empty() { None } else { Some(parts.join(",")) }
}

// ── BFS 引擎 ──────────────────────────────────────────────────────────────────

/// 构建资源关联图。
pub async fn build_graph(
    client: &Client,
    root_kind: &str,
    root_name: &str,
    root_namespace: Option<&str>,
    extractors: &[Box<dyn RelationExtractor>],
    max_depth: u32,
) -> Result<ResourceGraph, ResourceError> {
    let root_ref = ResourceRef::new(root_kind, root_namespace.map(String::from), root_name);

    // 节点表：key = ResourceRef，value = ResourceNode（保序用 order_vec）
    let mut node_map: HashMap<ResourceRef, ResourceNode> = HashMap::new();
    let mut node_order: Vec<ResourceRef> = Vec::new();

    let mut edges: Vec<ResourceEdge> = Vec::new();
    // 去重 key: (from, to, relation_type)
    let mut edge_set: HashSet<(ResourceRef, ResourceRef, String)> = HashSet::new();

    let mut queue: VecDeque<(ResourceRef, u32)> = VecDeque::new();
    let mut visited: HashSet<ResourceRef> = HashSet::new();

    queue.push_back((root_ref.clone(), 0));

    while let Some((ref_, depth)) = queue.pop_front() {
        if visited.contains(&ref_) {
            continue;
        }
        visited.insert(ref_.clone());

        let is_concrete = !ref_.name.is_empty();

        if !is_concrete {
            // 聚合节点已由调用方预插入节点表，无需 fetch
            continue;
        }

        // fetch JSON value
        let value = match get_resource_value(client, &ref_.kind, &ref_.name, ref_.namespace.as_deref()).await {
            Ok(v) => v,
            Err(_) => {
                if !node_map.contains_key(&ref_) {
                    node_order.push(ref_.clone());
                    node_map.insert(ref_.clone(), ResourceNode {
                        resource_ref: ref_.clone(),
                        depth,
                        is_concrete: true,
                        label_selector: None,
                    });
                }
                continue;
            }
        };

        if !node_map.contains_key(&ref_) {
            node_order.push(ref_.clone());
            node_map.insert(ref_.clone(), ResourceNode {
                resource_ref: ref_.clone(),
                depth,
                is_concrete: true,
                label_selector: None,
            });
        }

        if is_leaf_kind(&ref_.kind) || depth >= max_depth {
            continue;
        }

        let ns = ref_.namespace.as_deref();
        let matching_extractors: Vec<&Box<dyn RelationExtractor>> = extractors
            .iter()
            .filter(|e| e.source_kinds().contains(&ref_.kind.as_str()))
            .collect();

        let mut new_edges: Vec<ResourceEdge> = Vec::new();

        for ext in &matching_extractors {
            new_edges.extend(ext.extract_static(&ref_, &value));
        }

        let dynamic_futures: Vec<_> = matching_extractors
            .iter()
            .map(|ext| ext.extract_dynamic(&ref_, &value, client, ns))
            .collect();
        for result in join_all(dynamic_futures).await {
            new_edges.extend(result);
        }

        for edge in new_edges {
            let dedup_key = (
                edge.from.clone(),
                edge.to.clone(),
                format!("{:?}", edge.relation_type),
            );
            if !edge_set.contains(&dedup_key) {
                edge_set.insert(dedup_key);
                let target = edge.to.clone();
                let target_label_selector = edge.label_selector.clone();
                let target_is_concrete = !target.name.is_empty();
                edges.push(edge);

                if !visited.contains(&target) && !node_map.contains_key(&target) {
                    node_order.push(target.clone());
                    node_map.insert(target.clone(), ResourceNode {
                        resource_ref: target.clone(),
                        depth: depth + 1,
                        is_concrete: target_is_concrete,
                        label_selector: target_label_selector,
                    });
                    queue.push_back((target, depth + 1));
                }
            }
        }
    }

    // 按 BFS 顺序排列，root 在首位
    let root_node = node_map.remove(&root_ref);
    let mut nodes: Vec<ResourceNode> = Vec::with_capacity(node_order.len());
    if let Some(n) = root_node {
        nodes.push(n);
    }
    for r in node_order {
        if r != root_ref {
            if let Some(n) = node_map.remove(&r) {
                nodes.push(n);
            }
        }
    }

    Ok(ResourceGraph { root: root_ref, nodes, edges })
}
