//! 资源下发：支持工作台的 replace，以及编排中心的 create+replace / apply。

use crate::config::ResourceDeployStrategy;
use crate::kube::resources::ResourceError;
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
use kube::api::{Api, Patch, PatchParams, PostParams, Resource};
use kube::Client;

fn ns_or_default(ns: Option<&str>) -> &str {
    ns.unwrap_or("default")
}

fn sanitize_apply_value(mut obj: serde_json::Value) -> serde_json::Value {
    if let Some(root) = obj.as_object_mut() {
        if let Some(meta) = root.get_mut("metadata").and_then(|v| v.as_object_mut()) {
            meta.remove("managedFields");
            meta.remove("resourceVersion");
            meta.remove("uid");
            meta.remove("creationTimestamp");
            meta.remove("generation");
        }
        root.remove("status");
    }
    obj
}

fn parse_resource_identity(
    yaml: &str,
) -> Result<(serde_json::Value, String, String, Option<String>), ResourceError> {
    let obj: serde_json::Value = serde_yaml::from_str(yaml)
        .map_err(|e| ResourceError::Serialize(format!("yaml parse: {}", e)))?;
    let kind = obj
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ResourceError::Serialize("missing kind".to_string()))?
        .to_string();

    let meta = obj
        .get("metadata")
        .and_then(|v| v.as_object())
        .ok_or_else(|| ResourceError::Serialize("missing metadata".to_string()))?;

    let name = meta
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ResourceError::Serialize("missing metadata.name".to_string()))?
        .to_string();

    let namespace = meta
        .get("namespace")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok((obj, kind, name, namespace))
}

async fn replace_existing_resource<T>(
    api: Api<T>,
    name: &str,
    mut desired: T,
) -> Result<(), ResourceError>
where
    T: Clone + std::fmt::Debug + serde::de::DeserializeOwned + serde::Serialize + Resource<DynamicType = ()>,
{
    let existing = api.get(name).await.map_err(ResourceError::Kube)?;
    desired.meta_mut().resource_version = existing.meta().resource_version.clone();
    api.replace(name, &PostParams::default(), &desired)
        .await
        .map_err(ResourceError::Kube)?;
    Ok(())
}

async fn create_or_replace_resource<T>(
    api: Api<T>,
    name: &str,
    desired: T,
) -> Result<(), ResourceError>
where
    T: Clone + std::fmt::Debug + serde::de::DeserializeOwned + serde::Serialize + Resource<DynamicType = ()>,
{
    if api.get_opt(name).await.map_err(ResourceError::Kube)?.is_some() {
        replace_existing_resource(api, name, desired).await
    } else {
        api.create(&PostParams::default(), &desired)
            .await
            .map_err(ResourceError::Kube)?;
        Ok(())
    }
}

async fn server_side_apply_resource<T>(
    api: Api<T>,
    name: &str,
    desired: T,
) -> Result<(), ResourceError>
where
    T: Clone + std::fmt::Debug + serde::de::DeserializeOwned + serde::Serialize + Resource<DynamicType = ()>,
{
    let pp = PatchParams::apply("kube-flow").force();
    api.patch(name, &pp, &Patch::Apply(&desired))
        .await
        .map_err(ResourceError::Kube)?;
    Ok(())
}

macro_rules! dispatch_resource {
    ($client:expr, $kind:expr, $obj:expr, $name:expr, $namespace:expr, |$typed:ident, $api:ident| $body:block) => {{
        match $kind.as_str() {
            "Namespace" => {
                let $typed: Namespace =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<Namespace>::all($client.clone());
                $body
            }
            "Node" => {
                let $typed: Node =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<Node>::all($client.clone());
                $body
            }
            "PersistentVolume" => {
                let $typed: PersistentVolume =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<PersistentVolume>::all($client.clone());
                $body
            }
            "StorageClass" => {
                let $typed: StorageClass =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<StorageClass>::all($client.clone());
                $body
            }
            "ClusterRole" => {
                let $typed: ClusterRole =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<ClusterRole>::all($client.clone());
                $body
            }
            "ClusterRoleBinding" => {
                let $typed: ClusterRoleBinding =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<ClusterRoleBinding>::all($client.clone());
                $body
            }
            "Pod" => {
                let $typed: Pod =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<Pod>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "Deployment" => {
                let $typed: Deployment =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<Deployment>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "Service" => {
                let $typed: Service =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<Service>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "StatefulSet" => {
                let $typed: StatefulSet =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<StatefulSet>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "DaemonSet" => {
                let $typed: DaemonSet =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<DaemonSet>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "ConfigMap" => {
                let $typed: ConfigMap =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<ConfigMap>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "Secret" => {
                let $typed: Secret =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<Secret>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "ServiceAccount" => {
                let $typed: ServiceAccount =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<ServiceAccount>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "PersistentVolumeClaim" => {
                let $typed: PersistentVolumeClaim =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api =
                    Api::<PersistentVolumeClaim>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "Endpoints" => {
                let $typed: Endpoints =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<Endpoints>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "EndpointSlice" => {
                let $typed: EndpointSlice =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<EndpointSlice>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "Role" => {
                let $typed: Role =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<Role>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "RoleBinding" => {
                let $typed: RoleBinding =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<RoleBinding>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "ReplicaSet" => {
                let $typed: ReplicaSet =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<ReplicaSet>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "Job" => {
                let $typed: Job =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<Job>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "CronJob" => {
                let $typed: CronJob =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<CronJob>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "Ingress" => {
                let $typed: Ingress =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<Ingress>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "IngressClass" => {
                let $typed: IngressClass =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<IngressClass>::all($client.clone());
                $body
            }
            "NetworkPolicy" => {
                let $typed: NetworkPolicy =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<NetworkPolicy>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "ResourceQuota" => {
                let $typed: ResourceQuota =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<ResourceQuota>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "LimitRange" => {
                let $typed: LimitRange =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<LimitRange>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "PriorityClass" => {
                let $typed: PriorityClass =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api = Api::<PriorityClass>::all($client.clone());
                $body
            }
            "HorizontalPodAutoscaler" => {
                let $typed: HorizontalPodAutoscaler =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api =
                    Api::<HorizontalPodAutoscaler>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            "PodDisruptionBudget" => {
                let $typed: PodDisruptionBudget =
                    serde_json::from_value($obj.clone()).map_err(|e| ResourceError::Serialize(e.to_string()))?;
                let $api =
                    Api::<PodDisruptionBudget>::namespaced($client.clone(), ns_or_default($namespace.as_deref()));
                $body
            }
            _ => return Err(ResourceError::UnsupportedKind($kind.to_string())),
        }
    }};
}

/// 工作台资源编辑：要求对象已存在，按 replace 语义覆盖。
pub async fn apply_resource_yaml(client: &Client, yaml: &str) -> Result<(), ResourceError> {
    let (obj, kind, name, namespace) = parse_resource_identity(yaml)?;
    let obj = sanitize_apply_value(obj);

    dispatch_resource!(client, kind, obj, &name, namespace, |typed, api| {
        replace_existing_resource(api, &name, typed).await
    })
}

/// 编排中心资源下发：按策略使用 create+replace 或 server-side apply。
pub async fn deploy_resource_yaml(
    client: &Client,
    yaml: &str,
    strategy: ResourceDeployStrategy,
) -> Result<(), ResourceError> {
    let (obj, kind, name, namespace) = parse_resource_identity(yaml)?;
    let obj = sanitize_apply_value(obj);

    dispatch_resource!(client, kind, obj, &name, namespace, |typed, api| {
        match strategy {
            ResourceDeployStrategy::CreateReplace => create_or_replace_resource(api, &name, typed).await,
            ResourceDeployStrategy::Apply => server_side_apply_resource(api, &name, typed).await,
        }
    })
}
