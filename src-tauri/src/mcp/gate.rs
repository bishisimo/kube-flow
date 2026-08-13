//! CapabilityGate：deny-by-default 权限评估。

use super::capabilities::{resolve_capabilities, Capability};
use super::policy::{McpEnvBinding, McpPolicy};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 门控失败原因（结构化，便于 MCP / IPC 返回）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum GateDeny {
    McpDisabled,
    EnvNotVisible { env_id: String },
    CapabilityDenied { capability: String },
    NamespaceDenied { namespace: String },
    KindDenied { kind: String },
    SecretDenied,
    AppChannelRequired { capability: String },
    ApprovalRequired { capability: String },
    ApprovalDenied,
    AppOffline,
    InvalidToken,
}

impl std::fmt::Display for GateDeny {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::McpDisabled => write!(f, "MCP 未启用"),
            Self::EnvNotVisible { env_id } => {
                write!(f, "环境不可见或不在白名单: {env_id}")
            }
            Self::CapabilityDenied { capability } => {
                write!(f, "缺少能力: {capability}")
            }
            Self::NamespaceDenied { namespace } => {
                write!(f, "namespace 不在允许范围: {namespace}")
            }
            Self::KindDenied { kind } => write!(f, "kind 不在允许范围: {kind}"),
            Self::SecretDenied => write!(f, "Secret 访问未授权"),
            Self::AppChannelRequired { capability } => {
                write!(f, "能力 {capability} 需要 kube-flow App Gateway")
            }
            Self::ApprovalRequired { capability } => {
                write!(f, "能力 {capability} 需要人工确认")
            }
            Self::ApprovalDenied => write!(f, "用户拒绝了该操作"),
            Self::AppOffline => write!(f, "kube-flow App Gateway 未在线"),
            Self::InvalidToken => write!(f, "MCP token 无效"),
        }
    }
}

/// 一次操作的上下文。
#[derive(Debug, Clone)]
pub struct GateRequest<'a> {
    pub env_id: Option<&'a str>,
    pub capability: Capability,
    pub namespace: Option<&'a str>,
    pub kind: Option<&'a str>,
    /// 当前执行通道是否为独立 stdio（非 App Gateway）。
    pub via_stdio: bool,
    /// 是否已通过确认或持有有效 SessionGrant。
    pub approved: bool,
}

/// 评估结果：通过时附带绑定信息。
#[derive(Debug, Clone)]
pub struct GateAllow<'a> {
    pub binding: Option<&'a McpEnvBinding>,
    pub capabilities: BTreeSet<Capability>,
}

/// 命名空间是否命中绑定规则（空列表 = 全部允许；支持 `prefix*`）。
pub fn namespace_allowed(patterns: &[String], namespace: Option<&str>) -> bool {
    if patterns.is_empty() {
        return true;
    }
    let Some(ns) = namespace else {
        // 集群级资源无 namespace：仅当未限制 ns 时允许；有限制则拒绝无 ns 请求中的 namespaced 假设由调用方处理。
        // 这里：有 namespaces 限制时，无 namespace 的集群级资源仍允许（kind 再过滤）。
        return true;
    };
    patterns.iter().any(|p| match_glob_prefix(p, ns))
}

fn match_glob_prefix(pattern: &str, value: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        value.starts_with(prefix)
    } else {
        pattern == value
    }
}

pub fn kind_allowed(allow: &[String], kind: Option<&str>) -> bool {
    if allow.is_empty() {
        return true;
    }
    let Some(k) = kind else {
        return true;
    };
    allow.iter().any(|a| a.eq_ignore_ascii_case(k))
}

pub fn is_secret_kind(kind: Option<&str>) -> bool {
    kind.map(|k| k.eq_ignore_ascii_case("Secret"))
        .unwrap_or(false)
}

/// 评估请求；通过返回 Allow，否则返回 Deny。
pub fn evaluate<'a>(policy: &'a McpPolicy, req: &GateRequest<'_>) -> Result<GateAllow<'a>, GateDeny> {
    if !policy.mcp.enabled {
        return Err(GateDeny::McpDisabled);
    }

    // env.list 可不带具体 env；其余操作必须带 env_id。
    if req.capability == Capability::EnvList && req.env_id.is_none() {
        return Ok(GateAllow {
            binding: None,
            capabilities: [Capability::EnvList].into_iter().collect(),
        });
    }

    let env_id = req.env_id.ok_or_else(|| GateDeny::EnvNotVisible {
        env_id: "(missing)".into(),
    })?;
    let binding = policy
        .binding_for(env_id)
        .ok_or_else(|| GateDeny::EnvNotVisible {
            env_id: env_id.to_string(),
        })?;

    let caps = resolve_capabilities(
        binding.preset,
        &binding.parsed_extra_capabilities(),
        &binding.parsed_deny_capabilities(),
    );
    if !caps.contains(&req.capability) {
        return Err(GateDeny::CapabilityDenied {
            capability: req.capability.as_str().to_string(),
        });
    }

    if !namespace_allowed(&binding.namespaces, req.namespace) {
        return Err(GateDeny::NamespaceDenied {
            namespace: req.namespace.unwrap_or("").to_string(),
        });
    }
    if !kind_allowed(&binding.kinds_allow, req.kind) {
        return Err(GateDeny::KindDenied {
            kind: req.kind.unwrap_or("").to_string(),
        });
    }

    if is_secret_kind(req.kind) && !binding.allow_secrets {
        return Err(GateDeny::SecretDenied);
    }

    if req.via_stdio && req.capability.requires_app_channel() {
        return Err(GateDeny::AppChannelRequired {
            capability: req.capability.as_str().to_string(),
        });
    }

    let needs_approval = if req.capability.requires_forced_approval() {
        true
    } else if req.capability.requires_app_channel() {
        binding.require_approval
    } else {
        false
    };
    if needs_approval && !req.approved {
        return Err(GateDeny::ApprovalRequired {
            capability: req.capability.as_str().to_string(),
        });
    }

    Ok(GateAllow {
        binding: Some(binding),
        capabilities: caps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::policy::{McpEnvBinding, McpGlobalConfig, McpPolicy};
    use crate::mcp::capabilities::Preset;

    fn policy_with_binding(preset: Preset) -> McpPolicy {
        McpPolicy {
            mcp: McpGlobalConfig {
                enabled: true,
                ..Default::default()
            },
            bindings: vec![McpEnvBinding {
                env_id: "e1".into(),
                enabled: true,
                preset,
                capabilities: vec![],
                deny_capabilities: vec![],
                namespaces: vec!["default".into(), "app-*".into()],
                kinds_allow: vec![],
                require_approval: true,
                allow_secrets: false,
            }],
        }
    }

    #[test]
    fn denies_disabled() {
        let mut p = policy_with_binding(Preset::Readonly);
        p.mcp.enabled = false;
        let err = evaluate(
            &p,
            &GateRequest {
                env_id: Some("e1"),
                capability: Capability::ResourceGet,
                namespace: Some("default"),
                kind: Some("Pod"),
                via_stdio: true,
                approved: false,
            },
        )
        .unwrap_err();
        assert_eq!(err, GateDeny::McpDisabled);
    }

    #[test]
    fn namespace_glob() {
        assert!(namespace_allowed(
            &["app-*".into()],
            Some("app-prod")
        ));
        assert!(!namespace_allowed(
            &["app-*".into()],
            Some("kube-system")
        ));
    }

    #[test]
    fn secret_denied_by_default() {
        let p = policy_with_binding(Preset::ReadWrite);
        let err = evaluate(
            &p,
            &GateRequest {
                env_id: Some("e1"),
                capability: Capability::ResourceGet,
                namespace: Some("default"),
                kind: Some("Secret"),
                via_stdio: true,
                approved: false,
            },
        )
        .unwrap_err();
        assert_eq!(err, GateDeny::SecretDenied);
    }

    #[test]
    fn write_via_stdio_denied() {
        let p = policy_with_binding(Preset::ReadWrite);
        let err = evaluate(
            &p,
            &GateRequest {
                env_id: Some("e1"),
                capability: Capability::ResourceApply,
                namespace: Some("default"),
                kind: Some("Deployment"),
                via_stdio: true,
                approved: true,
            },
        )
        .unwrap_err();
        assert!(matches!(err, GateDeny::AppChannelRequired { .. }));
    }
}
