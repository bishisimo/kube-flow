/**
 * MCP 策略、本机服务与客户端配置导出的 Tauri invoke 封装。
 */
import { invoke } from "@tauri-apps/api/core";

export type McpPreset = "readonly" | "read_write" | "full" | "custom";

export interface McpGlobalConfig {
  enabled: boolean;
  listen: boolean;
  session_grant_minutes: number;
  token_path: string;
  audit_log: string;
}

export interface McpEnvBinding {
  env_id: string;
  enabled: boolean;
  preset: McpPreset;
  capabilities: string[];
  deny_capabilities: string[];
  namespaces: string[];
  kinds_allow: string[];
  require_approval: boolean;
  allow_secrets: boolean;
}

export interface McpPolicy {
  mcp: McpGlobalConfig;
  bindings: McpEnvBinding[];
}

export interface McpAuditEntry {
  ts: string;
  env_id: string | null;
  tool: string;
  capability: string;
  ok: boolean;
  error: string | null;
  approved: boolean;
  via: string;
}

export interface McpClientExport {
  id: "cursor" | "claude_code" | "codex" | string;
  label: string;
  config_path_hint: string;
  format: "json" | "toml" | string;
  snippet: string;
  cli_hint: string | null;
}

export interface McpClientsExport {
  command: string;
  args: string[];
  token: string;
  transport: string;
  http_url: string;
  clients: McpClientExport[];
}

/** @deprecated 使用 mcpExportClientConfigs */
export type McpCursorConfig = McpClientsExport;

export interface McpServiceStatus {
  gateway_running: boolean;
  http_running: boolean;
  http_port: number | null;
  http_url: string | null;
  token_ready: boolean;
  policy_enabled: boolean;
}

export interface McpApprovalRequest {
  request_id: string;
  env_id: string;
  capability: string;
  method: string;
  summary: string;
  force: boolean;
  can_grant_session: boolean;
}

export function mcpGetPolicy(): Promise<McpPolicy> {
  return invoke("mcp_get_policy");
}

export function mcpSetPolicy(policy: McpPolicy): Promise<void> {
  return invoke("mcp_set_policy", { policy });
}

export function mcpGatewayStatus(): Promise<boolean> {
  return invoke("mcp_gateway_status");
}

export function mcpGatewayStart(): Promise<void> {
  return invoke("mcp_gateway_start");
}

export function mcpGatewayStop(): Promise<void> {
  return invoke("mcp_gateway_stop");
}

export function mcpServiceStatus(): Promise<McpServiceStatus> {
  return invoke("mcp_service_status");
}

export function mcpServiceStart(): Promise<McpServiceStatus> {
  return invoke("mcp_service_start");
}

export function mcpServiceStop(): Promise<McpServiceStatus> {
  return invoke("mcp_service_stop");
}

export function mcpEnsureToken(): Promise<string> {
  return invoke("mcp_ensure_token");
}

export function mcpRegenerateToken(): Promise<string> {
  return invoke("mcp_regenerate_token");
}

export function mcpApprove(requestId: string, grantSession: boolean): Promise<void> {
  return invoke("mcp_approve", { requestId, grantSession });
}

export function mcpDeny(requestId: string): Promise<void> {
  return invoke("mcp_deny", { requestId });
}

export function mcpAuditRecent(limit = 50): Promise<McpAuditEntry[]> {
  return invoke("mcp_audit_recent", { limit });
}

export interface McpCheckItem {
  id: string;
  label: string;
  ok: boolean;
  detail: string;
}

export interface McpDiagnoseResult {
  ready: boolean;
  policy_enabled: boolean;
  gateway_running: boolean;
  gateway_reachable: boolean;
  http_running: boolean;
  http_url: string | null;
  token_ready: boolean;
  binary_path: string;
  binary_exists: boolean;
  enabled_env_count: number;
  checks: McpCheckItem[];
  howto: string[];
}

export function mcpDiagnose(binaryPath?: string): Promise<McpDiagnoseResult> {
  return invoke("mcp_diagnose", { binaryPath: binaryPath ?? null });
}

export function mcpResolveBinary(binaryPath?: string): Promise<[string, boolean]> {
  return invoke("mcp_resolve_binary", { binaryPath: binaryPath ?? null });
}

export function mcpExportClientConfigs(binaryPath: string): Promise<McpClientsExport> {
  return invoke("mcp_export_client_configs", { binaryPath });
}

/** @deprecated 使用 mcpExportClientConfigs */
export function mcpExportCursorConfig(binaryPath: string): Promise<McpClientsExport> {
  return mcpExportClientConfigs(binaryPath);
}

export const MCP_PRESET_OPTIONS: { label: string; value: McpPreset }[] = [
  { label: "只读", value: "readonly" },
  { label: "读写", value: "read_write" },
  { label: "完全", value: "full" },
  { label: "自定义", value: "custom" },
];

export const MCP_DANGER_CAPS = ["resource.delete", "pod.exec", "pod.files"] as const;
