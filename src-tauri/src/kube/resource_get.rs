//! 资源详情获取：按 kind/name/namespace 拉取单个资源并序列化为 YAML。
//! 与 resources 模块并列，职责分离：list 在 resources，get 在此。

use crate::kube::resource_dynamic::get_resource_yaml_by_kind;
use crate::kube::resources::{ns_or_default, ResourceError};
use kube::api::Api;
use kube::Client;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::autoscaling::v2::HorizontalPodAutoscaler;
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::{
    ConfigMap, Endpoints, LimitRange, Namespace, Node, PersistentVolume, PersistentVolumeClaim,
    Pod, ResourceQuota, Secret, Service, ServiceAccount,
};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::api::networking::v1::{Ingress, IngressClass, NetworkPolicy};
use k8s_openapi::api::policy::v1::PodDisruptionBudget;
use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding, Role, RoleBinding};
use k8s_openapi::api::scheduling::v1::PriorityClass;
use k8s_openapi::api::storage::v1::StorageClass;

/// 获取单个资源并返回 JSON 值，供关联跳转等逻辑解析。
/// kind 为 K8s API Kind（如 Pod、Deployment）；集群级资源 namespace 可为空。
pub async fn get_resource_value(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<serde_json::Value, ResourceError> {
    match kind {
        "Namespace" => get_and_serialize(Api::<Namespace>::all(client.clone()), name).await,
        "Node" => get_and_serialize(Api::<Node>::all(client.clone()), name).await,
        "PersistentVolume" => get_and_serialize(Api::<PersistentVolume>::all(client.clone()), name).await,
        "StorageClass" => get_and_serialize(Api::<StorageClass>::all(client.clone()), name).await,
        "ClusterRole" => get_and_serialize(Api::<ClusterRole>::all(client.clone()), name).await,
        "ClusterRoleBinding" => get_and_serialize(Api::<ClusterRoleBinding>::all(client.clone()), name).await,
        "Pod" => get_ns(Api::<Pod>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "Deployment" => get_ns(Api::<Deployment>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "Service" => get_ns(Api::<Service>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "StatefulSet" => get_ns(Api::<StatefulSet>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "DaemonSet" => get_ns(Api::<DaemonSet>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "ConfigMap" => get_ns(Api::<ConfigMap>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "Secret" => get_ns(Api::<Secret>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "ServiceAccount" => get_ns(Api::<ServiceAccount>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "PersistentVolumeClaim" => get_ns(Api::<PersistentVolumeClaim>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "Endpoints" => get_ns(Api::<Endpoints>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "EndpointSlice" => get_ns(Api::<EndpointSlice>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "Role" => get_ns(Api::<Role>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "RoleBinding" => get_ns(Api::<RoleBinding>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "ReplicaSet" => get_ns(Api::<ReplicaSet>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "Job" => get_ns(Api::<Job>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "CronJob" => get_ns(Api::<CronJob>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "Ingress" => get_ns(Api::<Ingress>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "IngressClass" => get_and_serialize(Api::<IngressClass>::all(client.clone()), name).await,
        "NetworkPolicy" => get_ns(Api::<NetworkPolicy>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "ResourceQuota" => get_ns(Api::<ResourceQuota>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "LimitRange" => get_ns(Api::<LimitRange>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "PriorityClass" => get_and_serialize(Api::<PriorityClass>::all(client.clone()), name).await,
        "HorizontalPodAutoscaler" => get_ns(Api::<HorizontalPodAutoscaler>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        "PodDisruptionBudget" => get_ns(Api::<PodDisruptionBudget>::namespaced(client.clone(), ns_or_default(namespace)), name).await,
        // 未知 kind：委托给 Discovery 动态路由
        _ => return Err(ResourceError::UnsupportedKind(kind.to_string())),
    }
}

/// 获取单个资源并返回 YAML 字符串。
/// 已知 kind 走静态类型路由；未知 kind 通过 Discovery 动态解析。
pub async fn get_resource_yaml(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<String, ResourceError> {
    match get_resource_value(client, kind, name, namespace).await {
        Ok(obj) => serde_yaml::to_string(&obj).map_err(|e| ResourceError::Serialize(e.to_string())),
        Err(ResourceError::UnsupportedKind(_)) => {
            get_resource_yaml_by_kind(client, kind, name, namespace).await
        }
        Err(e) => Err(e),
    }
}

async fn get_and_serialize<K>(api: Api<K>, name: &str) -> Result<serde_json::Value, ResourceError>
where
    K: Clone + std::fmt::Debug + kube::Resource<DynamicType = ()> + serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    let obj = api.get(name).await.map_err(ResourceError::Kube)?;
    serde_json::to_value(obj).map_err(|e| ResourceError::Serialize(e.to_string()))
}

async fn get_ns<K>(api: Api<K>, name: &str) -> Result<serde_json::Value, ResourceError>
where
    K: Clone + std::fmt::Debug + kube::Resource<DynamicType = ()> + serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    get_and_serialize(api, name).await
}
