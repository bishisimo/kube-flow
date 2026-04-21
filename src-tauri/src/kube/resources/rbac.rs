//! RBAC 资源：Role, RoleBinding, ClusterRole, ClusterRoleBinding。

use super::{
    build_list_params, format_creation_time, list_simple_cluster, list_simple_namespaced,
    ResourceError, SimpleClusterItem, SimpleNamespacedItem,
};
use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding, Role, RoleBinding, Subject};
use kube::api::Api;
use kube::Client;
use serde::Serialize;

// ── 数据结构 ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RoleItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

impl SimpleNamespacedItem for RoleItem {
    fn from_meta(meta: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta, default_ns: &str) -> Self {
        Self {
            name: meta.name.unwrap_or_default(),
            namespace: meta.namespace.unwrap_or_else(|| default_ns.to_string()),
            creation_time: format_creation_time(meta.creation_timestamp.as_ref()),
        }
    }
}

/// Subject 引用，仅 ServiceAccount 可下钻
#[derive(Debug, Clone, Serialize)]
pub struct SubjectRef {
    pub kind: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoleBindingItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_ref_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_ref_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjects: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjects_list: Option<Vec<SubjectRef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClusterRoleItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

impl SimpleClusterItem for ClusterRoleItem {
    fn from_meta_cluster(meta: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta) -> Self {
        Self {
            name: meta.name.unwrap_or_default(),
            creation_time: format_creation_time(meta.creation_timestamp.as_ref()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ClusterRoleBindingItem {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_ref_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_ref_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjects: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjects_list: Option<Vec<SubjectRef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

// ── 共用提取函数 ────────────────────────────────────────────────────────────

fn extract_role_ref_and_subjects(
    role_ref: &k8s_openapi::api::rbac::v1::RoleRef,
    subjects: &Option<Vec<Subject>>,
) -> (Option<String>, Option<String>, Option<String>, Option<u32>, Option<Vec<SubjectRef>>) {
    let role_ref_str = Some(format!("{}/{}", role_ref.kind, role_ref.name));
    let role_ref_kind = Some(role_ref.kind.clone());
    let role_ref_name = Some(role_ref.name.clone());
    let subject_count = subjects.as_ref().map(|s| s.len() as u32);
    let subjects_list: Option<Vec<SubjectRef>> = subjects.as_ref().map(|subs| {
        subs.iter()
            .filter(|s| s.kind == "ServiceAccount")
            .map(|s| SubjectRef {
                kind: s.kind.clone(),
                name: s.name.clone(),
                namespace: s.namespace.clone(),
            })
            .collect()
    });
    let subjects_list = subjects_list.filter(|v| !v.is_empty());
    (role_ref_str, role_ref_kind, role_ref_name, subject_count, subjects_list)
}

// ── list 函数 ──────────────────────────────────────────────────────────────

list_simple_namespaced!(list_roles, Role, RoleItem);
list_simple_cluster!(list_cluster_roles, ClusterRole, ClusterRoleItem);

/// 列出指定 namespace 的 RoleBindings。
pub async fn list_role_bindings(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<RoleBindingItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<RoleBinding> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|r| {
            let (role_ref, role_ref_kind, role_ref_name, subjects, subjects_list) =
                extract_role_ref_and_subjects(&r.role_ref, &r.subjects);
            RoleBindingItem {
                name: r.metadata.name.unwrap_or_default(),
                namespace: r.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                role_ref,
                role_ref_kind,
                role_ref_name,
                subjects,
                subjects_list,
                creation_time: format_creation_time(r.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出集群级 ClusterRoleBindings。
pub async fn list_cluster_role_bindings(
    client: &Client,
    label_selector: Option<&str>,
) -> Result<Vec<ClusterRoleBindingItem>, ResourceError> {
    let api: Api<ClusterRoleBinding> = Api::all(client.clone());
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|r| {
            let (role_ref, role_ref_kind, role_ref_name, subjects, subjects_list) =
                extract_role_ref_and_subjects(&r.role_ref, &r.subjects);
            ClusterRoleBindingItem {
                name: r.metadata.name.unwrap_or_default(),
                role_ref,
                role_ref_kind,
                role_ref_name,
                subjects,
                subjects_list,
                creation_time: format_creation_time(r.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}
