/**
 * 环境 = 连接（本地或 SSH），每个连接下可有多个 context。
 */
import { invoke } from "@tauri-apps/api/core";

export interface EnvironmentContext {
  context_name: string;
  display_name?: string | null;
  default_namespace?: string | null;
  cluster_name?: string | null;
}

export interface Environment {
  id: string;
  source: "local_kubeconfig" | "ssh_tunnel";
  display_name: string;
  tags: string[];
  is_favorite: boolean;
  sort_order: number;
  kubeconfig_path?: string | null;
  ssh_tunnel_id?: string | null;
  contexts: EnvironmentContext[];
  current_context?: string | null;
  last_used_at?: string | null;
  /** SSH 环境空闲保护：仅 true 时启用；未配置或 false 时不启用 */
  ssh_idle_protection?: boolean | null;
}

export interface KubeContextInfo {
  context_name: string;
  cluster_name: string;
  namespace?: string | null;
}

export interface SshTunnel {
  id: string;
  name: string;
  ssh_host: string;
  remote_kubeconfig_path: string;
  local_port?: number | null;
  remote_port?: number | null;
  has_saved_credential?: boolean;
}

/** 当前生效的 context 名（current_context 或第一个） */
export function effectiveContext(env: Environment): string | null {
  if (env.current_context && env.contexts.some((c) => c.context_name === env.current_context)) {
    return env.current_context;
  }
  return env.contexts[0]?.context_name ?? null;
}

/** 当前 context 的默认 namespace */
export function defaultNamespace(env: Environment): string | null {
  const name = effectiveContext(env);
  if (!name) return null;
  const ctx = env.contexts.find((c) => c.context_name === name);
  return ctx?.default_namespace ?? null;
}

export function envList(): Promise<Environment[]> {
  return invoke("env_list");
}

export function envAdd(env: Environment): Promise<void> {
  return invoke("env_add", { env });
}

export function envUpdate(env: Environment): Promise<void> {
  return invoke("env_update", { env });
}

export function envDelete(id: string): Promise<void> {
  return invoke("env_delete", { id });
}

export function envTouch(id: string): Promise<void> {
  return invoke("env_touch", { id });
}

export function envSetCurrentContext(envId: string, contextName: string): Promise<void> {
  return invoke("env_set_current_context", { args: { envId, contextName } });
}

export function envListContextsFromKubeconfig(kubeconfigPath: string): Promise<KubeContextInfo[]> {
  return invoke("env_list_contexts_from_kubeconfig", { kubeconfigPath });
}

export function envListSshTunnels(): Promise<SshTunnel[]> {
  return invoke("env_list_ssh_tunnels");
}

/** 列出 ~/.ssh/config 中的 Host，供创建 SSH 环境时选择。 */
export function envListSshConfigHosts(): Promise<string[]> {
  return invoke("env_list_ssh_config_hosts");
}

export function envCreateLocal(
  displayName: string,
  kubeconfigPath: string,
  selectedContexts: KubeContextInfo[],
  tags: string[] = []
): Promise<Environment> {
  return invoke("env_create_local", {
    args: { displayName, kubeconfigPath, selectedContexts, tags },
  });
}

export function envCreateSsh(
  displayName: string,
  sshTunnelId: string,
  contexts: EnvironmentContext[],
  sshIdleProtection?: boolean | null
): Promise<Environment> {
  return invoke("env_create_ssh", {
    args: { displayName, sshTunnelId, contexts, sshIdleProtection: sshIdleProtection ?? null },
  });
}

/** 使用 ~/.ssh/config 中的 Host 创建 SSH 环境；若无对应隧道配置会自动写入。localPort 为空则自动分配。 */
export function envCreateSshWithHost(
  displayName: string,
  sshHost: string,
  remoteKubeconfigPath: string,
  localPort: number | null,
  contexts: EnvironmentContext[],
  tags: string[] = [],
  sshIdleProtection?: boolean | null
): Promise<Environment> {
  return invoke("env_create_ssh_with_host", {
    args: {
      displayName,
      sshHost,
      remoteKubeconfigPath,
      localPort,
      contexts,
      tags,
      sshIdleProtection: sshIdleProtection ?? null,
    },
  });
}

/** 确保存在对应 Host 的隧道配置并返回 tunnel id；供编辑 SSH 环境切换 Host 时使用。localPort 为空则自动分配。 */
export function envEnsureSshTunnelForHost(
  sshHost: string,
  remoteKubeconfigPath: string,
  localPort?: number | null
): Promise<string> {
  return invoke("env_ensure_ssh_tunnel_for_host", {
    args: {
      sshHost,
      remoteKubeconfigPath,
      localPort: localPort ?? null,
    },
  });
}
