//! 资源删除：按 kind/name/namespace 调用 K8s Delete API。

use crate::kube::resources::ResourceError;
use kube::api::{Api, DeleteParams};
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

fn ns_or_default(ns: Option<&str>) -> &str {
    ns.unwrap_or("default")
}

/// 删除指定资源。集群级资源 namespace 为空。
pub async fn delete_resource(
    client: &Client,
    kind: &str,
    name: &str,
    namespace: Option<&str>,
) -> Result<(), ResourceError> {
    let dp = DeleteParams::default();

    let result = match kind {
        "Namespace" => Api::<Namespace>::all(client.clone()).delete(name, &dp).await.map(|_| ()),
        "Node" => Api::<Node>::all(client.clone()).delete(name, &dp).await.map(|_| ()),
        "PersistentVolume" => Api::<PersistentVolume>::all(client.clone()).delete(name, &dp).await.map(|_| ()),
        "StorageClass" => Api::<StorageClass>::all(client.clone()).delete(name, &dp).await.map(|_| ()),
        "ClusterRole" => Api::<ClusterRole>::all(client.clone()).delete(name, &dp).await.map(|_| ()),
        "ClusterRoleBinding" => Api::<ClusterRoleBinding>::all(client.clone()).delete(name, &dp).await.map(|_| ()),
        "IngressClass" => Api::<IngressClass>::all(client.clone()).delete(name, &dp).await.map(|_| ()),
        "PriorityClass" => Api::<PriorityClass>::all(client.clone()).delete(name, &dp).await.map(|_| ()),
        "Pod" => Api::<Pod>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "Deployment" => Api::<Deployment>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "Service" => Api::<Service>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "StatefulSet" => Api::<StatefulSet>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "DaemonSet" => Api::<DaemonSet>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "ConfigMap" => Api::<ConfigMap>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "Secret" => Api::<Secret>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "ServiceAccount" => Api::<ServiceAccount>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "PersistentVolumeClaim" => Api::<PersistentVolumeClaim>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "Endpoints" => Api::<Endpoints>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "EndpointSlice" => Api::<EndpointSlice>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "Role" => Api::<Role>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "RoleBinding" => Api::<RoleBinding>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "ReplicaSet" => Api::<ReplicaSet>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "Job" => Api::<Job>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "CronJob" => Api::<CronJob>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "Ingress" => Api::<Ingress>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "NetworkPolicy" => Api::<NetworkPolicy>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "ResourceQuota" => Api::<ResourceQuota>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "LimitRange" => Api::<LimitRange>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "HorizontalPodAutoscaler" => Api::<HorizontalPodAutoscaler>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        "PodDisruptionBudget" => Api::<PodDisruptionBudget>::namespaced(client.clone(), ns_or_default(namespace)).delete(name, &dp).await.map(|_| ()),
        _ => return Err(ResourceError::UnsupportedKind(kind.to_string())),
    };
    result.map_err(ResourceError::Kube)
}
