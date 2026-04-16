export type StrongholdAuthLike = {
  checkAndHandle: (
    errorMessage: string,
    onConfirmed?: () => void,
    request?: { title?: string; description?: string }
  ) => Promise<boolean>;
};

export type SshAuthLike = {
  checkAndHandle: (errorMessage: string, onConfirmed?: () => void) => Promise<boolean>;
};

type HandleAuthRetryOptions = {
  message: string;
  envId: string;
  retry: () => void;
  strongholdAuth: StrongholdAuthLike;
  sshAuth: SshAuthLike;
  setConnecting: (envId: string) => void;
};

/**
 * 处理连接认证重试流程：优先 Stronghold 解锁，再处理 SSH 认证输入。
 * 若已接管错误并等待用户输入，返回 true。
 */
export async function handleAuthRetry(options: HandleAuthRetryOptions): Promise<boolean> {
  const isStrongholdRequired = await options.strongholdAuth.checkAndHandle(options.message, options.retry, {
    title: "解锁环境凭证",
    description: "当前环境连接需要访问已保存凭证，请先输入 Stronghold 主密码解锁。",
  });
  if (isStrongholdRequired) {
    options.setConnecting(options.envId);
    return true;
  }
  const isAuthRequired = await options.sshAuth.checkAndHandle(options.message, options.retry);
  if (isAuthRequired) {
    options.setConnecting(options.envId);
    return true;
  }
  return false;
}
