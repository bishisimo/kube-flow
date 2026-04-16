//! 批处理资源：Job, CronJob。

use super::{build_list_params, format_creation_time, ResourceError};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use kube::api::Api;
use kube::Client;
use serde::Serialize;

// ── 数据结构 ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct JobItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CronJobItem {
    pub name: String,
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_schedule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,
}

// ── list 函数 ──────────────────────────────────────────────────────────────

/// 列出指定 namespace 的 Jobs。
pub async fn list_jobs(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<JobItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<Job> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|j| {
            let status = j.status.as_ref();
            let succeeded = status.and_then(|s| s.succeeded);
            let failed = status.and_then(|s| s.failed);
            let active = status.and_then(|s| s.active);
            let spec = j.spec.as_ref();
            let completions = spec.and_then(|s| s.completions);
            let completions_str = match (completions, succeeded, failed, active) {
                (Some(c), Some(s), Some(f), Some(a)) => Some(format!("{}/{}", s + f + a, c)),
                (Some(c), s, f, a) => Some(format!("{}/{}", s.unwrap_or(0) + f.unwrap_or(0) + a.unwrap_or(0), c)),
                _ => Some("1/1".to_string()),
            };
            let start = status.and_then(|s| s.start_time.as_ref());
            let completion = status.and_then(|s| s.completion_time.as_ref());
            let duration = match (start, completion) {
                (Some(st), Some(ct)) => {
                    let d = ct.0.signed_duration_since(st.0);
                    Some(format!("{}s", d.num_seconds().max(0)))
                }
                _ => None,
            };
            JobItem {
                name: j.metadata.name.unwrap_or_default(),
                namespace: j.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                completions: completions_str,
                duration,
                creation_time: format_creation_time(j.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}

/// 列出指定 namespace 的 CronJobs。
pub async fn list_cron_jobs(
    client: &Client,
    namespace: Option<&str>,
    label_selector: Option<&str>,
) -> Result<Vec<CronJobItem>, ResourceError> {
    let ns = namespace.unwrap_or("default");
    let api: Api<CronJob> = Api::namespaced(client.clone(), ns);
    let list = api.list(&build_list_params(label_selector)).await.map_err(ResourceError::Kube)?;
    let items = list
        .items
        .into_iter()
        .map(|c| {
            let schedule = c.spec.as_ref().map(|s| s.schedule.clone());
            let last_schedule = c.status.as_ref().and_then(|s| s.last_successful_time.as_ref().or(s.last_schedule_time.as_ref()));
            let last_schedule_str = last_schedule.and_then(|t| format_creation_time(Some(t)));
            CronJobItem {
                name: c.metadata.name.unwrap_or_default(),
                namespace: c.metadata.namespace.unwrap_or_else(|| ns.to_string()),
                schedule,
                last_schedule: last_schedule_str,
                creation_time: format_creation_time(c.metadata.creation_timestamp.as_ref()),
            }
        })
        .collect();
    Ok(items)
}
