//! 集群 API 发现驱动的资源别名索引（shortNames / plural / kind / singular），按环境缓存。
//! 与 kubectl 类似：刷新后可在本地解析别名，避免每次解析都打 discovery。
//!
//! `search`：单字符仅匹配扩展资源的 shortNames；≥2 字符可匹配 kind / plural / singular / shortNames / group / apiVersion。
//! 结果按相关度排序并截断（默认 Top 10）。

use kube::core::gvk::GroupVersion;
use kube::Client;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use tokio::sync::Mutex;

/// 解析得到的资源定位信息，供前端展示或后续 DynamicObject 列表使用。
#[derive(Clone, Debug, Serialize, PartialEq, Eq, Hash)]
pub struct ResolvedAliasTarget {
    pub group: String,
    pub version: String,
    pub api_version: String,
    pub kind: String,
    pub plural: String,
    pub namespaced: bool,
    /// apiserver 声明的短名，供展示与单字符别名匹配。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub short_names: Vec<String>,
    /// APIResource.singular_name，可能为空字符串。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub singular: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResourceAliasRefreshResult {
    pub resource_count: usize,
    pub alias_key_count: usize,
}

#[derive(Clone, Debug)]
struct CatalogEntry {
    target: ResolvedAliasTarget,
    short_names: Vec<String>,
    singular: Option<String>,
}

struct EnvAliasCache {
    map: HashMap<String, Vec<ResolvedAliasTarget>>,
    catalog: Vec<CatalogEntry>,
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

    /// 遍历 /api 与 /apis 下各版本的 APIResourceList，建立别名索引与搜索目录。
    pub async fn refresh(&self, env_id: &str, client: &Client) -> Result<ResourceAliasRefreshResult, kube::Error> {
        let (map, catalog, resource_count) = build_alias_map(client).await?;
        let alias_key_count = map.len();
        self.inner.lock().await.insert(
            env_id.to_string(),
            EnvAliasCache { map, catalog },
        );
        Ok(ResourceAliasRefreshResult {
            resource_count,
            alias_key_count,
        })
    }

    /// 按别名精确查询；`preferred_group` 非空时优先筛到该 API 组（仍保留其它候选若筛空）。
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

    /// 按查询串搜索扩展资源类型，返回最多 `limit` 条（相关度降序）。
    pub async fn search(&self, env_id: &str, query: &str, limit: usize) -> Result<Vec<ResolvedAliasTarget>, String> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(vec![]);
        }
        let guard = self.inner.lock().await;
        let cache = guard
            .get(env_id)
            .ok_or_else(|| "资源别名缓存未就绪，请先点击「刷新发现缓存」".to_string())?;
        let qc = q.to_lowercase();
        let char_count = q.chars().count();
        let lim = limit.max(1);

        let mut scored: Vec<(i32, ResolvedAliasTarget)> = Vec::new();
        for entry in &cache.catalog {
            if let Some(s) = score_match(char_count, &qc, entry) {
                let mut t = entry.target.clone();
                if !entry.short_names.is_empty() {
                    t.short_names = entry.short_names.clone();
                }
                t.singular = entry.singular.clone();
                scored.push((s, t));
            }
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.kind.cmp(&b.1.kind)));
        let mut seen: HashSet<(String, String, String)> = HashSet::new();
        let mut out: Vec<ResolvedAliasTarget> = Vec::new();
        for (_, t) in scored {
            let k = (t.api_version.clone(), t.kind.clone(), t.plural.clone());
            if seen.insert(k) && out.len() < lim {
                out.push(t);
            }
        }
        Ok(out)
    }
}

/// 单字符：仅 shortNames；≥2：kind / plural / singular / shortNames / group / apiVersion（子串匹配 + kind 子序列加分）。
fn score_match(char_count: usize, qc: &str, entry: &CatalogEntry) -> Option<i32> {
    if char_count == 1 {
        for sn in &entry.short_names {
            let sl = sn.to_lowercase();
            if sl.starts_with(qc) {
                return Some(100);
            }
        }
        return None;
    }

    let kind_l = entry.target.kind.to_lowercase();
    let plural_l = entry.target.plural.to_lowercase();
    let group_l = entry.target.group.to_lowercase();
    let av_l = entry.target.api_version.to_lowercase();
    let mut best: i32 = 0;

    if kind_l.contains(qc) {
        let mut s = 120;
        if kind_l.starts_with(qc) {
            s += 40;
        }
        best = best.max(s);
    } else if subsequence_match(qc, &kind_l) {
        best = best.max(90);
    }

    if plural_l.contains(qc) {
        best = best.max(100);
    }
    if let Some(sg) = entry.singular.as_ref().map(|s| s.to_lowercase()) {
        if sg.contains(qc) {
            best = best.max(95);
        }
    }
    for sn in &entry.short_names {
        let sl = sn.to_lowercase();
        if sl.contains(qc) {
            best = best.max(88);
        }
    }
    if group_l.contains(qc) {
        best = best.max(60);
    }
    if av_l.contains(qc) {
        best = best.max(55);
    }

    if best > 0 { Some(best) } else { None }
}

fn subsequence_match(q: &str, text: &str) -> bool {
    let mut qi = q.chars().peekable();
    let mut ti = text.chars().peekable();
    while let Some(&qc) = qi.peek() {
        let mut found = false;
        while let Some(&tc) = ti.peek() {
            ti.next();
            if tc == qc {
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
        qi.next();
    }
    true
}

fn push_target(map: &mut HashMap<String, Vec<ResolvedAliasTarget>>, key: &str, t: ResolvedAliasTarget) {
    let key = key.to_lowercase();
    let v = map.entry(key).or_default();
    if !v.iter().any(|x| x == &t) {
        v.push(t);
    }
}

fn catalog_merge(
    catalog_map: &mut HashMap<String, CatalogEntry>,
    target: ResolvedAliasTarget,
    singular: Option<String>,
    short_names: &[String],
) {
    let key = format!("{}\x1f{}\x1f{}", target.api_version, target.kind, target.plural);
    let entry = catalog_map.entry(key).or_insert_with(|| CatalogEntry {
        target: target.clone(),
        short_names: Vec::new(),
        singular: singular.clone(),
    });
    entry.target = target;
    if entry.singular.is_none() {
        entry.singular = singular;
    }
    let mut seen: HashSet<String> = entry.short_names.iter().map(|s| s.to_lowercase()).collect();
    for s in short_names {
        let sl = s.to_lowercase();
        if !s.is_empty() && seen.insert(sl) {
            entry.short_names.push(s.clone());
        }
    }
}

fn index_resource(
    map: &mut HashMap<String, Vec<ResolvedAliasTarget>>,
    catalog_map: &mut HashMap<String, CatalogEntry>,
    r: &k8s_openapi::apimachinery::pkg::apis::meta::v1::APIResource,
    group_version: &str,
) -> bool {
    let gv: GroupVersion = match group_version.parse() {
        Ok(g) => g,
        Err(_) => return false,
    };
    let group = r.group.clone().unwrap_or_else(|| gv.group.clone());
    let version = r.version.clone().unwrap_or_else(|| gv.version.clone());
    let api_version = gv.api_version();
    let mut shorts: Vec<String> = Vec::new();
    if let Some(ref sn) = r.short_names {
        for s in sn {
            if !s.is_empty() {
                shorts.push(s.clone());
            }
        }
    }
    let singular = if r.singular_name.is_empty() {
        None
    } else {
        Some(r.singular_name.clone())
    };

    let target = ResolvedAliasTarget {
        group: group.clone(),
        version: version.clone(),
        api_version,
        kind: r.kind.clone(),
        plural: r.name.clone(),
        namespaced: r.namespaced,
        short_names: Vec::new(),
        singular: None,
    };

    catalog_merge(catalog_map, target.clone(), singular.clone(), &shorts);

    push_target(map, &r.kind, target.clone());
    push_target(map, &r.name, target.clone());
    if let Some(ref sg) = singular {
        push_target(map, sg, target.clone());
    }
    for s in &shorts {
        push_target(map, s, target.clone());
    }
    true
}

async fn build_alias_map(
    client: &Client,
) -> Result<(HashMap<String, Vec<ResolvedAliasTarget>>, Vec<CatalogEntry>, usize), kube::Error> {
    let mut map: HashMap<String, Vec<ResolvedAliasTarget>> = HashMap::new();
    let mut catalog_map: HashMap<String, CatalogEntry> = HashMap::new();
    let mut resource_count = 0usize;

    let api_groups = client.list_api_groups().await?;
    for g in api_groups.groups {
        for ver in &g.versions {
            let list = client.list_api_group_resources(&ver.group_version).await?;
            for r in &list.resources {
                if r.name.contains('/') {
                    continue;
                }
                if index_resource(&mut map, &mut catalog_map, r, &list.group_version) {
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
            if index_resource(&mut map, &mut catalog_map, r, &list.group_version) {
                resource_count += 1;
            }
        }
    }

    let mut catalog: Vec<CatalogEntry> = catalog_map.into_values().collect();
    catalog.sort_by(|a, b| {
        a.target
            .group
            .cmp(&b.target.group)
            .then_with(|| a.target.kind.cmp(&b.target.kind))
    });
    Ok((map, catalog, resource_count))
}
