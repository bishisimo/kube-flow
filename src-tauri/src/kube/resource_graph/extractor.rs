//! RelationExtractor trait：每种关联关系的提取接口。

use super::{ResourceEdge, ResourceRef};
use async_trait::async_trait;
use kube::Client;

#[async_trait]
pub trait RelationExtractor: Send + Sync {
    /// 声明本提取器处理哪些 source kind。
    fn source_kinds(&self) -> &[&str];

    /// 从已加载的 JSON 值中同步提取边，无需 API 调用。
    fn extract_static(
        &self,
        node_ref: &ResourceRef,
        value: &serde_json::Value,
    ) -> Vec<ResourceEdge>;

    /// 需要额外 API list 调用的动态提取（如反向查找）。默认空实现。
    async fn extract_dynamic(
        &self,
        node_ref: &ResourceRef,
        value: &serde_json::Value,
        client: &Client,
        namespace: Option<&str>,
    ) -> Vec<ResourceEdge> {
        let _ = (node_ref, value, client, namespace);
        vec![]
    }
}
