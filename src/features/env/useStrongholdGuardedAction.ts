import type { Ref } from "vue";
import { extractErrorMessage } from "../../utils/errorMessage";
import { useStrongholdAuthStore } from "../../stores/strongholdAuth";

export interface StrongholdGuardOpts {
  /** 弹出解锁框时显示的标题 */
  title: string;
  /** 弹出解锁框时显示的描述 */
  description: string;
  /** Stronghold 已锁定时写入 msgRef 的提示文字 */
  lockedText: string;
}

/**
 * 处理凭证操作抛出的错误：
 * - 若是 Stronghold 已锁定：触发解锁弹窗并设置 msgRef，返回 true
 * - 否则：将错误文本写入 msgRef，返回 false
 *
 * 用法：在 catch 块中调用，传入 retry 回调（通常是再次调用当前函数）。
 */
export function useStrongholdGuardedAction() {
  const strongholdAuth = useStrongholdAuthStore();

  async function handleError(
    e: unknown,
    retry: () => void,
    msgRef: Ref<{ type: "ok" | "err"; text: string } | null>,
    opts: StrongholdGuardOpts
  ): Promise<boolean> {
    const message = extractErrorMessage(e);
    const isLocked = await strongholdAuth.checkAndHandle(message, retry, {
      title: opts.title,
      description: opts.description,
    });
    if (isLocked) {
      msgRef.value = { type: "err", text: opts.lockedText };
      return true;
    }
    msgRef.value = { type: "err", text: message };
    return false;
  }

  return { handleError };
}
