//! MCP 原子能力与档位预设。Capability 是权限真源；Preset 只是快捷模板。

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// MCP 可授予的原子能力（不含本机 shell/文件/凭证）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    EnvList,
    ResourceList,
    ResourceGet,
    LogsRead,
    ResourceApply,
    ResourcePatch,
    WorkloadLifecycle,
    ResourceCreate,
    ResourceDelete,
    PodExec,
    PodFiles,
}

impl Capability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EnvList => "env.list",
            Self::ResourceList => "resource.list",
            Self::ResourceGet => "resource.get",
            Self::LogsRead => "logs.read",
            Self::ResourceApply => "resource.apply",
            Self::ResourcePatch => "resource.patch",
            Self::WorkloadLifecycle => "workload.lifecycle",
            Self::ResourceCreate => "resource.create",
            Self::ResourceDelete => "resource.delete",
            Self::PodExec => "pod.exec",
            Self::PodFiles => "pod.files",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "env.list" => Some(Self::EnvList),
            "resource.list" => Some(Self::ResourceList),
            "resource.get" => Some(Self::ResourceGet),
            "logs.read" => Some(Self::LogsRead),
            "resource.apply" => Some(Self::ResourceApply),
            "resource.patch" => Some(Self::ResourcePatch),
            "workload.lifecycle" => Some(Self::WorkloadLifecycle),
            "resource.create" => Some(Self::ResourceCreate),
            "resource.delete" => Some(Self::ResourceDelete),
            "pod.exec" => Some(Self::PodExec),
            "pod.files" => Some(Self::PodFiles),
            _ => None,
        }
    }

    /// 可在独立 stdio MCP 进程中执行（只读）。
    pub fn is_stdio_channel(self) -> bool {
        matches!(
            self,
            Self::EnvList | Self::ResourceList | Self::ResourceGet | Self::LogsRead
        )
    }

    /// 破坏类：强制每次确认，不吃 SessionGrant。
    pub fn requires_forced_approval(self) -> bool {
        matches!(
            self,
            Self::ResourceDelete | Self::PodExec | Self::PodFiles
        )
    }

    /// 写操作（含破坏类）：必须经 App Gateway。
    pub fn requires_app_channel(self) -> bool {
        !self.is_stdio_channel()
    }
}

/// 档位预设。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    #[default]
    Readonly,
    ReadWrite,
    Full,
    Custom,
}

impl Preset {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Readonly => "readonly",
            Self::ReadWrite => "read_write",
            Self::Full => "full",
            Self::Custom => "custom",
        }
    }

    pub fn capabilities(self) -> BTreeSet<Capability> {
        use Capability::*;
        match self {
            Self::Readonly => [EnvList, ResourceList, ResourceGet, LogsRead]
                .into_iter()
                .collect(),
            Self::ReadWrite => {
                let mut caps = Self::Readonly.capabilities();
                caps.extend([ResourceApply, ResourcePatch, WorkloadLifecycle]);
                caps
            }
            Self::Full => {
                let mut caps = Self::ReadWrite.capabilities();
                caps.extend([ResourceCreate, ResourceDelete, PodExec, PodFiles]);
                caps
            }
            Self::Custom => BTreeSet::new(),
        }
    }
}

/// 合并预设与显式 grant/deny，得到最终能力集。
pub fn resolve_capabilities(
    preset: Preset,
    extra: &[Capability],
    deny: &[Capability],
) -> BTreeSet<Capability> {
    let mut caps = preset.capabilities();
    for c in extra {
        caps.insert(*c);
    }
    for c in deny {
        caps.remove(c);
    }
    caps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readonly_excludes_write() {
        let caps = Preset::Readonly.capabilities();
        assert!(caps.contains(&Capability::ResourceGet));
        assert!(!caps.contains(&Capability::ResourceApply));
        assert!(!caps.contains(&Capability::ResourceDelete));
    }

    #[test]
    fn deny_removes_from_full() {
        let caps = resolve_capabilities(
            Preset::Full,
            &[],
            &[Capability::ResourceDelete, Capability::PodExec],
        );
        assert!(!caps.contains(&Capability::ResourceDelete));
        assert!(!caps.contains(&Capability::PodExec));
        assert!(caps.contains(&Capability::ResourceApply));
    }
}
