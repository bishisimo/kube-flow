//! 资源 Apply：解析 YAML 并 replace 到集群。
//! 要求 YAML 包含 resourceVersion，用于乐观并发控制。

use crate::kube::resources::ResourceError;
use kube::api::{Api, PostParams};
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

/// 解析 YAML 并 replace 到集群。
/// YAML 必须包含 metadata.resourceVersion。
pub async fn apply_resource_yaml(client: &Client, yaml: &str) -> Result<(), ResourceError> {
    let obj: serde_json::Value = serde_yaml::from_str(yaml)
        .map_err(|e| ResourceError::Serialize(format!("yaml parse: {}", e)))?;

    let kind = obj
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ResourceError::Serialize("missing kind".to_string()))?;

    let meta = obj
        .get("metadata")
        .and_then(|v| v.as_object())
        .ok_or_else(|| ResourceError::Serialize("missing metadata".to_string()))?;

    let name = meta
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ResourceError::Serialize("missing metadata.name".to_string()))?;

    let namespace = meta.get("namespace").and_then(|v| v.as_str());

    let pp = PostParams::default();

    match kind {
        "Namespace" => {
            let o: Namespace = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<Namespace>::all(client.clone()).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "Node" => {
            let o: Node = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<Node>::all(client.clone()).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "PersistentVolume" => {
            let o: PersistentVolume = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<PersistentVolume>::all(client.clone()).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "StorageClass" => {
            let o: StorageClass = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<StorageClass>::all(client.clone()).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "ClusterRole" => {
            let o: ClusterRole = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<ClusterRole>::all(client.clone()).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "ClusterRoleBinding" => {
            let o: ClusterRoleBinding = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<ClusterRoleBinding>::all(client.clone()).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "Pod" => {
            let o: Pod = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<Pod>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "Deployment" => {
            let o: Deployment = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<Deployment>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "Service" => {
            let o: Service = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<Service>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "StatefulSet" => {
            let o: StatefulSet = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<StatefulSet>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "DaemonSet" => {
            let o: DaemonSet = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<DaemonSet>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "ConfigMap" => {
            let o: ConfigMap = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<ConfigMap>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "Secret" => {
            let o: Secret = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<Secret>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "ServiceAccount" => {
            let o: ServiceAccount = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<ServiceAccount>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "PersistentVolumeClaim" => {
            let o: PersistentVolumeClaim = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<PersistentVolumeClaim>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "Endpoints" => {
            let o: Endpoints = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<Endpoints>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "EndpointSlice" => {
            let o: EndpointSlice = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<EndpointSlice>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "Role" => {
            let o: Role = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<Role>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "RoleBinding" => {
            let o: RoleBinding = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<RoleBinding>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "ReplicaSet" => {
            let o: ReplicaSet = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<ReplicaSet>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "Job" => {
            let o: Job = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<Job>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "CronJob" => {
            let o: CronJob = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<CronJob>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "Ingress" => {
            let o: Ingress = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<Ingress>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "IngressClass" => {
            let o: IngressClass = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<IngressClass>::all(client.clone()).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "NetworkPolicy" => {
            let o: NetworkPolicy = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<NetworkPolicy>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "ResourceQuota" => {
            let o: ResourceQuota = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<ResourceQuota>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "LimitRange" => {
            let o: LimitRange = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<LimitRange>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "PriorityClass" => {
            let o: PriorityClass = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<PriorityClass>::all(client.clone()).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "HorizontalPodAutoscaler" => {
            let o: HorizontalPodAutoscaler = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<HorizontalPodAutoscaler>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        "PodDisruptionBudget" => {
            let o: PodDisruptionBudget = serde_json::from_value(obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
            Api::<PodDisruptionBudget>::namespaced(client.clone(), ns_or_default(namespace)).replace(name, &pp, &o).await.map_err(ResourceError::Kube)?;
        }
        _ => return Err(ResourceError::UnsupportedKind(kind.to_string())),
    }

    Ok(())
}
