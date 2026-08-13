//! kube-flow MCP stdio Server：实现 MCP tools/list 与 tools/call。

use super::audit;
use super::capabilities::Capability;
use super::gate::{self, GateRequest};
use super::ipc_client::{self, gateway_seems_online};
use super::policy::McpPolicy;
use super::read_executor;
use super::token;
use super::write_executor::{capability_for_method, meta_from_yaml};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(default)]
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

/// 在当前线程阻塞运行 MCP stdio 循环（供 bin 入口调用）。
pub fn run_stdio_blocking() -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let err = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": e.to_string() }
                });
                writeln!(stdout, "{}", err).map_err(|e| e.to_string())?;
                stdout.flush().map_err(|e| e.to_string())?;
                continue;
            }
        };

        // 通知无需响应
        if req.id.is_none() && req.method.starts_with("notifications/") {
            continue;
        }

        let id = req.id.clone().unwrap_or(Value::Null);
        let result = rt.block_on(dispatch_mcp(&req.method, req.params));
        let resp = match result {
            Ok(value) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": value
            }),
            Err(msg) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32000, "message": msg }
            }),
        };
        writeln!(stdout, "{}", resp).map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 处理单条 JSON-RPC 请求体；通知类返回 `None`（无需响应）。
pub async fn handle_jsonrpc_body(body: &str) -> Result<Option<Value>, String> {
    let req: JsonRpcRequest = serde_json::from_str(body).map_err(|e| e.to_string())?;
    if req.id.is_none() && req.method.starts_with("notifications/") {
        return Ok(None);
    }
    let id = req.id.clone().unwrap_or(Value::Null);
    match dispatch_mcp(&req.method, req.params).await {
        Ok(value) => Ok(Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": value
        }))),
        Err(msg) => Ok(Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32000, "message": msg }
        }))),
    }
}

/// MCP 方法分发（stdio 与 HTTP 共用）。
pub async fn dispatch_mcp(method: &str, params: Value) -> Result<Value, String> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "kube-flow", "version": env!("CARGO_PKG_VERSION") }
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_defs() })),
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing tool name".to_string())?;
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
            call_tool(name, arguments).await
        }
        _ => Err(format!("method not found: {method}")),
    }
}

fn tool_defs() -> Vec<Value> {
    vec![
        tool(
            "list_environments",
            "List MCP-visible kube-flow environments (whitelist only).",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "list_resources",
            "List Kubernetes resources of a kind in an environment.",
            json!({
                "type": "object",
                "properties": {
                    "env_id": { "type": "string" },
                    "kind": { "type": "string" },
                    "namespace": { "type": "string" },
                    "label_selector": { "type": "string" }
                },
                "required": ["env_id", "kind"]
            }),
        ),
        tool(
            "get_resource_yaml",
            "Get a resource as YAML.",
            json!({
                "type": "object",
                "properties": {
                    "env_id": { "type": "string" },
                    "kind": { "type": "string" },
                    "name": { "type": "string" },
                    "namespace": { "type": "string" }
                },
                "required": ["env_id", "kind", "name"]
            }),
        ),
        tool(
            "describe_resource",
            "Describe a resource (markdown).",
            json!({
                "type": "object",
                "properties": {
                    "env_id": { "type": "string" },
                    "kind": { "type": "string" },
                    "name": { "type": "string" },
                    "namespace": { "type": "string" }
                },
                "required": ["env_id", "kind", "name"]
            }),
        ),
        tool(
            "get_logs",
            "Fetch pod container logs (tailed).",
            json!({
                "type": "object",
                "properties": {
                    "env_id": { "type": "string" },
                    "namespace": { "type": "string" },
                    "pod": { "type": "string" },
                    "container": { "type": "string" },
                    "tail_lines": { "type": "integer" }
                },
                "required": ["env_id", "namespace", "pod"]
            }),
        ),
        tool(
            "apply_resource_yaml",
            "Apply YAML to an existing resource (requires App Gateway + approval).",
            json!({
                "type": "object",
                "properties": {
                    "env_id": { "type": "string" },
                    "yaml": { "type": "string" }
                },
                "required": ["env_id", "yaml"]
            }),
        ),
        tool(
            "patch_resource",
            "Strategic merge patch a resource (requires App Gateway + approval).",
            json!({
                "type": "object",
                "properties": {
                    "env_id": { "type": "string" },
                    "kind": { "type": "string" },
                    "name": { "type": "string" },
                    "namespace": { "type": "string" },
                    "patch": { "type": "object" }
                },
                "required": ["env_id", "kind", "name", "patch"]
            }),
        ),
        tool(
            "workload_action",
            "stop | resume | restart a workload (requires App Gateway + approval).",
            json!({
                "type": "object",
                "properties": {
                    "env_id": { "type": "string" },
                    "action": { "type": "string" },
                    "kind": { "type": "string" },
                    "name": { "type": "string" },
                    "namespace": { "type": "string" }
                },
                "required": ["env_id", "action", "kind", "name"]
            }),
        ),
        tool(
            "create_or_deploy",
            "Create or deploy a resource from YAML (requires App Gateway + approval).",
            json!({
                "type": "object",
                "properties": {
                    "env_id": { "type": "string" },
                    "yaml": { "type": "string" }
                },
                "required": ["env_id", "yaml"]
            }),
        ),
        tool(
            "delete_resource",
            "Delete a resource (requires App Gateway + forced approval).",
            json!({
                "type": "object",
                "properties": {
                    "env_id": { "type": "string" },
                    "kind": { "type": "string" },
                    "name": { "type": "string" },
                    "namespace": { "type": "string" }
                },
                "required": ["env_id", "kind", "name"]
            }),
        ),
        tool(
            "pod_exec",
            "Run a non-interactive command in a pod (requires App Gateway + forced approval).",
            json!({
                "type": "object",
                "properties": {
                    "env_id": { "type": "string" },
                    "namespace": { "type": "string" },
                    "pod": { "type": "string" },
                    "container": { "type": "string" },
                    "command": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["env_id", "namespace", "pod", "command"]
            }),
        ),
        tool(
            "pod_file_transfer",
            "Upload/download a file to/from a pod (requires App Gateway + forced approval).",
            json!({
                "type": "object",
                "properties": {
                    "env_id": { "type": "string" },
                    "direction": { "type": "string", "enum": ["upload", "download"] },
                    "namespace": { "type": "string" },
                    "pod": { "type": "string" },
                    "container": { "type": "string" },
                    "local_path": { "type": "string" },
                    "remote_path": { "type": "string" },
                    "overwrite": { "type": "boolean" }
                },
                "required": ["env_id", "direction", "namespace", "pod", "local_path", "remote_path"]
            }),
        ),
    ]
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

async fn call_tool(name: &str, arguments: Value) -> Result<Value, String> {
    let policy = McpPolicy::load_default().map_err(|e| e.to_string())?;
    let Some(cap) = capability_for_method(name) else {
        return Err(format!("unknown tool: {name}"));
    };

    if name == "list_environments" {
        if !policy.mcp.enabled {
            return Err("MCP 未启用".into());
        }
        let ids: Vec<String> = policy
            .bindings
            .iter()
            .filter(|b| b.enabled)
            .map(|b| b.env_id.clone())
            .collect();
        // env.list 可本地完成
        match read_executor::list_environment_summaries(&ids) {
            Ok(list) => {
                audit::record(None, name, cap.as_str(), true, None, false, "stdio");
                return Ok(tool_text(json!({ "environments": list })));
            }
            Err(e) => {
                audit::record(None, name, cap.as_str(), false, Some(&e), false, "stdio");
                return Err(e);
            }
        }
    }

    let env_id = arguments
        .get("env_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let (kind, namespace) = scope_from_args(name, &arguments);

    let gate_req = GateRequest {
        env_id: env_id.as_deref(),
        capability: cap,
        namespace: namespace.as_deref(),
        kind: kind.as_deref(),
        via_stdio: true,
        approved: false,
    };

    // 写能力：stdio 门控会要求 App；转发 Gateway
    if cap.requires_app_channel() {
        return forward_to_gateway(name, arguments, env_id.as_deref(), cap).await;
    }

    if let Err(e) = gate::evaluate(&policy, &gate_req) {
        // 只读但本地无法执行（如 SSH）时，尝试 Gateway
        if gateway_seems_online() {
            return forward_to_gateway(name, arguments, env_id.as_deref(), cap).await;
        }
        audit::record(
            env_id.as_deref(),
            name,
            cap.as_str(),
            false,
            Some(&e.to_string()),
            false,
            "stdio",
        );
        return Err(e.to_string());
    }

    // 本地只读执行
    let result = execute_local_read(name, &arguments).await;
    match &result {
        Ok(_) => audit::record(env_id.as_deref(), name, cap.as_str(), true, None, false, "stdio"),
        Err(e) => {
            // SSH 等失败时回退 Gateway
            if gateway_seems_online() {
                return forward_to_gateway(name, arguments, env_id.as_deref(), cap).await;
            }
            audit::record(
                env_id.as_deref(),
                name,
                cap.as_str(),
                false,
                Some(e),
                false,
                "stdio",
            );
        }
    }
    result.map(tool_text)
}

async fn execute_local_read(name: &str, args: &Value) -> Result<Value, String> {
    let env_id = args
        .get("env_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing env_id".to_string())?;
    let env = read_executor::load_environment(env_id)?;
    let client = read_executor::client_for_local_env(&env).await?;
    match name {
        "list_resources" => {
            let kind = args
                .get("kind")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing kind".to_string())?;
            let ns = args.get("namespace").and_then(|v| v.as_str());
            let label = args.get("label_selector").and_then(|v| v.as_str());
            read_executor::list_resources_json(&client, kind, ns, label).await
        }
        "get_resource_yaml" => {
            let kind = args
                .get("kind")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing kind".to_string())?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing name".to_string())?;
            let ns = args.get("namespace").and_then(|v| v.as_str());
            let yaml = read_executor::get_yaml(&client, kind, name, ns).await?;
            Ok(json!({ "yaml": yaml }))
        }
        "describe_resource" => {
            let kind = args
                .get("kind")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing kind".to_string())?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing name".to_string())?;
            let ns = args.get("namespace").and_then(|v| v.as_str());
            let markdown = read_executor::describe(&client, kind, name, ns).await?;
            Ok(json!({ "markdown": markdown }))
        }
        "get_logs" => {
            let ns = args
                .get("namespace")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing namespace".to_string())?;
            let pod = args
                .get("pod")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing pod".to_string())?;
            let container = args.get("container").and_then(|v| v.as_str());
            let tail = args.get("tail_lines").and_then(|v| v.as_i64());
            let logs = read_executor::logs(&client, ns, pod, container, tail).await?;
            Ok(json!({ "logs": logs }))
        }
        _ => Err(format!("not a local read tool: {name}")),
    }
}

async fn forward_to_gateway(
    name: &str,
    arguments: Value,
    env_id: Option<&str>,
    cap: Capability,
) -> Result<Value, String> {
    if !gateway_seems_online() {
        audit::record(
            env_id,
            name,
            cap.as_str(),
            false,
            Some("AppOffline"),
            false,
            "stdio",
        );
        return Err(
            "AppOffline: 请启动 kube-flow 并启用 MCP Gateway 后再执行写/SSH 操作".into(),
        );
    }
    let token = token::read_token().or_else(|_| token::ensure_token())?;
    match ipc_client::call_gateway(&token, name, arguments).await {
        Ok(v) => {
            audit::record(env_id, name, cap.as_str(), true, None, true, "stdio->gateway");
            Ok(tool_text(v))
        }
        Err(e) => {
            audit::record(
                env_id,
                name,
                cap.as_str(),
                false,
                Some(&e),
                false,
                "stdio->gateway",
            );
            Err(e)
        }
    }
}

fn scope_from_args(name: &str, args: &Value) -> (Option<String>, Option<String>) {
    if matches!(name, "apply_resource_yaml" | "create_or_deploy") {
        if let Some(yaml) = args.get("yaml").and_then(|v| v.as_str()) {
            let (kind, ns, _) = meta_from_yaml(yaml);
            return (kind, ns);
        }
    }
    let kind = args
        .get("kind")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let ns = args
        .get("namespace")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let _ = Uuid::new_v4(); // keep uuid linked for future correlation ids
    (kind, ns)
}

fn tool_text(v: Value) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())
        }]
    })
}
