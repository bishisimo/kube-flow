//! MCP App Gateway 线协议：一行一个 JSON 请求/响应。

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayRequest {
    pub id: String,
    pub token: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    /// 调用方声明已获批准（通常仅 App 内部在确认后置 true）。
    #[serde(default)]
    pub approved: bool,
    /// 确认时同时发放 SessionGrant（非破坏类）。
    #[serde(default)]
    pub grant_session: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayResponse {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<GatewayErrorBody>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayErrorBody {
    pub code: String,
    pub message: String,
}

impl GatewayResponse {
    pub fn success(id: impl Into<String>, result: Value) -> Self {
        Self {
            id: id.into(),
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn failure(id: impl Into<String>, code: &str, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ok: false,
            result: None,
            error: Some(GatewayErrorBody {
                code: code.into(),
                message: message.into(),
            }),
        }
    }
}

/// 发往前端的确认请求事件载荷。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequestPayload {
    pub request_id: String,
    pub env_id: String,
    pub capability: String,
    pub method: String,
    pub summary: String,
    pub force: bool,
    pub can_grant_session: bool,
}

pub const APPROVAL_EVENT: &str = "mcp-approval-request";
