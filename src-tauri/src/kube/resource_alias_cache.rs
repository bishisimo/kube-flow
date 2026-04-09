//! 集群 API 发现驱动的资源别名索引（shortNames / plural / kind / singular），按环境缓存。
//! 与 kubectl 类似：刷新后可在本地解析别名，避免每次解析都打 discovery。

use kube::core::gvk::GroupVersion;
use kube::Client;
use serde::Serialize;
use std::collections::HashMap;
use tokio::sync::Mutex;

/// 解析得到的资源定位信息，供前端展示或后续 DynamicObject 列表使用。
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ResolvedAliasTarget {
    pub group: String,
    pub version: String,
    pub api_version: String,
    pub kind: String,
    pub plural: String,
    pub namespaced: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResourceAliasRefreshResult {
    pub resource_count: usize,
    pub alias_key_count: usize,
}

struct EnvAliasCache {
    map: HashMap<String, Vec<ResolvedAliasTarget>>,
}

/// 按环境 id 持有别名索引；与 KubeClient 生命周期独立，移除客户端时可丢弃缓存。
pub struct ResourceAliasCacheStore {
    inner: Mutex<HashMap<String, EnvAliasCache>>,
}

impl ResourceAliasCacheStore {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub async fn remove_env(&self, env_id: &str) {
        self.inner.lock().await.remove(env_id);
    }

    /// 遍历 /api 与 /apis 下各版本的 APIResourceList，建立别名索引。
    pub async fn refresh(&self, env_id: &str, client: &Client) -> Result<ResourceAliasRefreshResult, kube::Error> {
        let (map, resource_count) = build_alias_map(client).await?;
        let alias_key_count = map.len();
        self.inner.lock().await.insert(
            env_id.to_string(),
            EnvAliasCache { map },
        );
        Ok(ResourceAliasRefreshResult {
            resource_count,
            alias_key_count,
        })
    }

    /// 按别名查询；`preferred_group` 非空时优先筛到该 API 组（仍保留其它候选若筛空）。
    pub async fn resolve(
        &self,
        env_id: &str,
        query: &str,
        preferred_group: Option<&str>,
    ) -> Result<Vec<ResolvedAliasTarget>, String> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Err("查询为空".into());
        }
        let guard = self.inner.lock().await;
        let cache = guard
            .get(env_id)
            .ok_or_else(|| "资源别名缓存未就绪，请先点击「刷新发现缓存」".to_string())?;
        let mut candidates = cache.map.get(&q).cloned().unwrap_or_default();
        if candidates.is_empty() {
            return Ok(vec![]);
        }
        if let Some(pg) = preferred_group.map(str::trim).filter(|s| !s.is_empty()) {
            let filtered: Vec<_> = candidates.iter().filter(|c| c.group == pg).cloned().collect();
            if !filtered.is_empty() {
                candidates = filtered;
            }
        }
        Ok(candidates)
    }
}

fn push_target(map: &mut HashMap<String, Vec<ResolvedAliasTarget>>, key: &str, t: ResolvedAliasTarget) {
    let key = key.to_lowercase();
    let v = map.entry(key).or_default();
    if !v.iter().any(|x| x == &t) {
        v.push(t);
    }
}

fn index_resource(
    map: &mut HashMap<String, Vec<ResolvedAliasTarget>>,
    r: &k8s_openapi::apimachinery::pkg::apis::meta::v1::APIResource,
    group_version: &str,
) -> bool {
    let gv: GroupVersion = match group_version.parse() {
        Ok(g) => g,
        Err(_) => return false,
    };
    let group = r
        .group
        .clone()
        .unwrap_or_else(|| gv.group.clone());
    let version = r
        .version
        .clone()
        .unwrap_or_else(|| gv.version.clone());
    let api_version = gv.api_version();
    let target = ResolvedAliasTarget {
        group: group.clone(),
        version: version.clone(),
        api_version,
        kind: r.kind.clone(),
        plural: r.name.clone(),
        namespaced: r.namespaced,
    };

    push_target(map, &r.kind, target.clone());
    push_target(map, &r.name, target.clone());
    if !r.singular_name.is_empty() {
        push_target(map, &r.singular_name, target.clone());
    }
    if let Some(shorts) = r.short_names.as_ref() {
        for s in shorts {
            if !s.is_empty() {
                push_target(map, s, target.clone());
            }
        }
    }
    true
}

async fn build_alias_map(client: &Client) -> Result<(HashMap<String, Vec<ResolvedAliasTarget>>, usize), kube::Error> {
    let mut map: HashMap<String, Vec<ResolvedAliasTarget>> = HashMap::new();
    let mut resource_count = 0usize;

    let api_groups = client.list_api_groups().await?;
    for g in api_groups.groups {
        for ver in &g.versions {
            let list = client.list_api_group_resources(&ver.group_version).await?;
            for r in &list.resources {
                if r.name.contains('/') {
                    continue;
                }
                if index_resource(&mut map, r, &list.group_version) {
                    resource_count += 1;
                }
            }
        }
    }

    let corevers = client.list_core_api_versions().await?;
    for v in corevers.versions {
        let list = client.list_core_api_resources(&v).await?;
        for r in &list.resources {
            if r.name.contains('/') {
                continue;
            }
            if index_resource(&mut map, r, &list.group_version) {
                resource_count += 1;
            }
        }
    }

    Ok((map, resource_count))
}
