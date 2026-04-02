//! 工作负载表格用 Pod 态势聚合：按 Pod 归类为 运/等/成/败/异，并汇总最近容器结束时间。

use super::format_creation_time;
use k8s_openapi::api::core::v1::{ContainerStatus, Pod};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, Time};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkloadPodRollup {
    /// Running 且全部业务容器 Ready
    pub running_ready: i32,
    /// Pending，或 Running 但未全部 Ready 且无「异」类原因
    pub pending: i32,
    pub succeeded: i32,
    pub failed: i32,
    /// CrashLoop、镜像拉取/创建容器错误、Unknown 等
    pub abnormal: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_container_restart: Option<String>,
}

#[derive(Clone, Copy)]
enum RollupBucket {
    RunningReady,
    Pending,
    Succeeded,
    Failed,
    Abnormal,
}

fn waiting_reason_abnormal(reason: Option<&str>) -> bool {
    matches!(
        reason.unwrap_or(""),
        "CrashLoopBackOff"
            | "ImagePullBackOff"
            | "ErrImagePull"
            | "InvalidImageName"
            | "CreateContainerConfigError"
            | "CreateContainerError"
            | "RunContainerError"
    )
}

fn pod_has_abnormal_container_state(pod: &Pod) -> bool {
    let Some(status) = pod.status.as_ref() else {
        return false;
    };
    let check = |statuses: Option<&Vec<ContainerStatus>>| {
        statuses.is_some_and(|ss| {
            ss.iter().any(|cs| {
                cs.state
                    .as_ref()
                    .and_then(|st| st.waiting.as_ref())
                    .is_some_and(|w| waiting_reason_abnormal(w.reason.as_deref()))
            })
        })
    };
    check(status.container_statuses.as_ref()) || check(status.init_container_statuses.as_ref())
}

fn all_workload_containers_ready(pod: &Pod) -> bool {
    let spec_n = pod.spec.as_ref().map(|s| s.containers.len()).unwrap_or(0);
    if spec_n == 0 {
        return false;
    }
    let Some(statuses) = pod.status.as_ref().and_then(|s| s.container_statuses.as_ref()) else {
        return false;
    };
    if statuses.len() < spec_n {
        return false;
    }
    statuses.iter().filter(|cs| cs.ready).count() == spec_n
}

fn classify_pod(pod: &Pod) -> RollupBucket {
    let phase = pod
        .status
        .as_ref()
        .and_then(|s| s.phase.as_deref())
        .unwrap_or("");

    if phase == "Failed" {
        return RollupBucket::Failed;
    }
    if phase == "Succeeded" {
        return RollupBucket::Succeeded;
    }
    if phase == "Unknown" {
        return RollupBucket::Abnormal;
    }
    if pod_has_abnormal_container_state(pod) {
        return RollupBucket::Abnormal;
    }
    if phase == "Pending" {
        return RollupBucket::Pending;
    }
    if phase == "Running" {
        if all_workload_containers_ready(pod) {
            return RollupBucket::RunningReady;
        }
        return RollupBucket::Pending;
    }
    RollupBucket::Pending
}

fn pod_labels_match_selector(labels: &BTreeMap<String, String>, sel: &LabelSelector) -> bool {
    let has_ml = sel.match_labels.as_ref().is_some_and(|m| !m.is_empty());
    let has_me = sel.match_expressions.as_ref().is_some_and(|e| !e.is_empty());
    if !has_ml && !has_me {
        return false;
    }
    if let Some(ref ml) = sel.match_labels {
        for (k, v) in ml {
            if labels.get(k).map(String::as_str) != Some(v.as_str()) {
                return false;
            }
        }
    }
    if let Some(ref exprs) = sel.match_expressions {
        for expr in exprs {
            let key = expr.key.as_str();
            let op = expr.operator.as_str();
            let vals: Vec<&str> = expr.values.iter().flatten().map(|s| s.as_str()).collect();
            match op {
                "In" => {
                    let lv = labels.get(key).map(|s| s.as_str());
                    if !vals.iter().any(|v| Some(*v) == lv) {
                        return false;
                    }
                }
                "NotIn" => {
                    let lv = labels.get(key).map(|s| s.as_str());
                    if vals.iter().any(|v| Some(*v) == lv) {
                        return false;
                    }
                }
                "Exists" => {
                    if !labels.contains_key(key) {
                        return false;
                    }
                }
                "DoesNotExist" => {
                    if labels.contains_key(key) {
                        return false;
                    }
                }
                _ => {}
            }
        }
    }
    true
}

fn bump_max_finished(
    statuses: Option<&Vec<ContainerStatus>>,
    best: &mut Option<Time>,
) {
    let Some(statuses) = statuses else { return };
    for cs in statuses {
        if let Some(ft) = cs
            .last_state
            .as_ref()
            .and_then(|st| st.terminated.as_ref())
            .and_then(|term| term.finished_at.as_ref())
        {
            *best = Some(match best {
                None => ft.clone(),
                Some(cur) => {
                    if ft.0 > cur.0 {
                        ft.clone()
                    } else {
                        cur.clone()
                    }
                }
            });
        }
    }
}

fn pod_max_finished_time(pod: &Pod) -> Option<Time> {
    let status = pod.status.as_ref()?;
    let mut best: Option<Time> = None;
    bump_max_finished(status.container_statuses.as_ref(), &mut best);
    bump_max_finished(status.init_container_statuses.as_ref(), &mut best);
    best
}

/// 将同一 namespace 下、命中 workload selector 的 Pod 聚合为表格用态势。
pub fn compute_workload_pod_rollup(pods: &[Pod], workload_ns: &str, selector: &LabelSelector) -> WorkloadPodRollup {
    let mut r = WorkloadPodRollup::default();
    let mut global_best: Option<Time> = None;

    for p in pods {
        let pns = p.metadata.namespace.as_deref().unwrap_or("");
        if pns != workload_ns {
            continue;
        }
        let labels = match p.metadata.labels.as_ref() {
            Some(l) if !l.is_empty() => l,
            _ => continue,
        };
        if !pod_labels_match_selector(labels, selector) {
            continue;
        }

        match classify_pod(p) {
            RollupBucket::RunningReady => r.running_ready += 1,
            RollupBucket::Pending => r.pending += 1,
            RollupBucket::Succeeded => r.succeeded += 1,
            RollupBucket::Failed => r.failed += 1,
            RollupBucket::Abnormal => r.abnormal += 1,
        }

        if let Some(t) = pod_max_finished_time(p) {
            global_best = Some(match global_best {
                None => t,
                Some(cur) => {
                    if t.0 > cur.0 {
                        t
                    } else {
                        cur
                    }
                }
            });
        }
    }

    r.last_container_restart = global_best.as_ref().and_then(|t| format_creation_time(Some(t)));
    r
}
