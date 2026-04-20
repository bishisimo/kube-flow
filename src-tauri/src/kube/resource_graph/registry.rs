//! 全局 Extractor 注册表。新增关联类型只需在此处添加一行。

use super::extractor::RelationExtractor;
use super::extractors::{
    hpa_ref::HpaRefExtractor,
    ingress_backend::IngressBackendExtractor,
    owner_ref::OwnerRefExtractor,
    pvc_bindings::PvcBindingsExtractor,
    rbac_refs::RbacRefsExtractor,
    sa_bindings_reverse::SaBindingsReverseExtractor,
    selector::WorkloadSelectorExtractor,
    service_account::ServiceAccountExtractor,
    service_reverse::ServiceReverseExtractor,
    service_selector::ServiceSelectorExtractor,
    workload_mounts::WorkloadMountsExtractor,
};

/// 构建默认 Extractor 注册表，包含所有内置关联规则。
pub fn build_default_registry() -> Vec<Box<dyn RelationExtractor>> {
    vec![
        Box::new(WorkloadMountsExtractor),
        Box::new(OwnerRefExtractor),
        Box::new(WorkloadSelectorExtractor),
        Box::new(ServiceSelectorExtractor),
        Box::new(ServiceAccountExtractor),
        Box::new(PvcBindingsExtractor),
        Box::new(IngressBackendExtractor),
        Box::new(ServiceReverseExtractor),
        Box::new(HpaRefExtractor),
        Box::new(RbacRefsExtractor),
        Box::new(SaBindingsReverseExtractor),
    ]
}
