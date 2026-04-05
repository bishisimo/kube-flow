/**
 * SSH 认证状态 store：当 Tauri 命令返回 SSH_AUTH_REQUIRED 错误时，
 * 激活全局密码输入弹窗；用户输入密码后写入内存缓存，调用方可重试。
 */
import { ref } from "vue";
import { credentialCacheOnly } from "../api/credential";
import { envListSshTunnels } from "../api/env";
import { useStrongholdAuthStore } from "./strongholdAuth";
import { useStrongholdStatusStore } from "./strongholdStatus";

export interface SshAuthRequest {
  tunnelId: string;
  tunnelName: string;
  sshHost: string;
  /** 确认密码后调用此回调（可选，用于自动重试） */
  onConfirmed?: () => void;
}

/** 错误模式，与 Rust 侧 TunnelError::AuthRequired 格式一致 */
const AUTH_REQUIRED_RE = /SSH_AUTH_REQUIRED:([^\s]+)/;

const pending = ref<SshAuthRequest | null>(null);
const strongholdAuth = useStrongholdAuthStore();
const { refreshStrongholdStatus } = useStrongholdStatusStore();

/**
 * 检查错误字符串是否包含 SSH 认证错误，若是则激活弹窗并返回 true。
 * @param error  Tauri 命令返回的错误字符串
 * @param onConfirmed  用户确认密码后的回调（用于触发重试）
 */
async function checkAndHandle(error: string, onConfirmed?: () => void): Promise<boolean> {
  const match = AUTH_REQUIRED_RE.exec(error);
  if (!match) return false;

  const tunnelId = match[1];
  let tunnelName = tunnelId;
  let sshHost = tunnelId;
  try {
    const tunnels = await envListSshTunnels();
    const found = tunnels.find((t) => t.id === tunnelId);
    if (found) {
      tunnelName = found.name;
      sshHost = found.ssh_host;
      if (found.has_saved_credential) {
        const status = await refreshStrongholdStatus().catch(() => null);
        if (status === "locked") {
          await strongholdAuth.checkAndHandle(
            "Stronghold 已锁定",
            onConfirmed,
            {
              title: "解锁环境凭证",
              description: "当前环境已配置保存密码，请先输入 Stronghold 主密码解锁。",
            }
          );
          return true;
        }
      }
    }
  } catch {
    // 查找失败时降级使用 tunnelId
  }

  pending.value = { tunnelId, tunnelName, sshHost, onConfirmed };
  return true;
}

/** 用户在弹窗中确认密码 */
async function confirm(password: string) {
  if (!pending.value) return;
  const { tunnelId, onConfirmed } = pending.value;
  await credentialCacheOnly(tunnelId, password);
  pending.value = null;
  onConfirmed?.();
}

/** 用户取消弹窗 */
function cancel() {
  pending.value = null;
}

export function useSshAuthStore() {
  return { pending, checkAndHandle, confirm, cancel };
}
