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
    pub mod workload_service_link;
}

use crate::kube::resource_get::get_resource_value;
use crate::kube::resources::ResourceError;
use extractor::RelationExtractor;
use futures::future::join_all;
use kube::Client;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};

// ── 数据结构 ──────────────────────────────────────────────────────────────────

/// 图上的资源引用。`name` 仅当对应集群中可按 kind+namespace+name 唯一定位时非空；按 label 聚合的虚拟目标用空 `name` + `set_id`。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ResourceRef {
    pub kind: String,
    pub namespace: Option<String>,
    pub name: String,
    /// 无 API 具名、仅由 label 集合标识的边目标在图内去重用，不是集群中的资源名。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_id: Option<String>,
}

impl ResourceRef {
    pub fn new(kind: impl Into<String>, namespace: Option<String>, name: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            namespace,
            name: name.into(),
            set_id: None,
        }
    }

    /// 无集群对象名、仅由 `set_id` 在图内与 label 集合一一对应；不要传入 `get`。
    pub fn for_label_set(kind: impl Into<String>, namespace: Option<String>, set_id: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            namespace,
            name: String::new(),
            set_id: Some(set_id.into()),
        }
    }
}

/// 图中的节点。
#[derive(Debug, Clone, Serialize)]
pub struct ResourceNode {
    pub resource_ref: ResourceRef,
    pub depth: u32,
    /// 可在集群中按 `resource_ref` 的 kind+namespace+name 拉取 YAML；按 label 聚合的虚拟目标为 false
    pub is_concrete: bool,
    /// 聚合/虚拟目标：列表跳转用
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_selector: Option<String>,
    /// 人可读摘要（如 Pod 匹配数量），不当作资源名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_label: Option<String>,
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
    /// Workload 的 Pod 模板 label 满足该 Service 的 spec.selector
    ServicePodMatch,
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
    /// 与 `to` 的展示用（如 Pod 数），进入 `ResourceNode.display_label`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_display: Option<String>,
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

/// 对「同 kind + 命名空间 + label selector」的聚合边目标生成稳定、与文案无关的图内 id。
pub fn set_id_for_label_aggregate(kind: &str, namespace: Option<&str>, label_selector: &str) -> String {
    let mut h: u64 = 1469598103934665603;
    const P: u64 = 1099511628211;
    for b in kind.as_bytes() {
        h = h.wrapping_mul(P).wrapping_add(*b as u64);
    }
    h = h.wrapping_mul(P);
    if let Some(ns) = namespace {
        for b in ns.as_bytes() {
            h = h.wrapping_mul(P).wrapping_add(*b as u64);
        }
    }
    h = h.wrapping_mul(P);
    for b in label_selector.as_bytes() {
        h = h.wrapping_mul(P).wrapping_add(*b as u64);
    }
    format!("v1:{:016x}", h)
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

        let can_fetch = !ref_.name.is_empty() && ref_.set_id.is_none();
        if !can_fetch {
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
                        display_label: None,
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
                display_label: None,
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
                let to_display = edge.to_display.clone();
                let target_is_concrete = !target.name.is_empty() && target.set_id.is_none();
                edges.push(edge);

                if !visited.contains(&target) && !node_map.contains_key(&target) {
                    node_order.push(target.clone());
                    node_map.insert(target.clone(), ResourceNode {
                        resource_ref: target.clone(),
                        depth: depth + 1,
                        is_concrete: target_is_concrete,
                        label_selector: target_label_selector,
                        display_label: to_display,
                    });
                    if target_is_concrete {
                        queue.push_back((target, depth + 1));
                    }
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
