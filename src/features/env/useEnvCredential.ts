/**
 * 环境相关凭证面板的共享逻辑：保存、清除、存在性探测与 Stronghold 解锁守护。
 *
 * 给定一个动态的凭证 id（函数形式，随当前操作对象变化），返回可响应的状态：
 * - exists        凭证是否已保存
 * - loading       操作进行中
 * - message       成功/失败提示（供调用方显示）
 * - save / remove 写入/清除凭证；命中 Stronghold 已锁时自动弹出解锁框并回执重试
 * - refresh       重新探测是否已保存
 * - reset         重置面板状态（弹窗关闭时调用）
 */
import { ref, type Ref } from "vue";
import { credentialExists, credentialSave, credentialDelete } from "../../api/credential";
import { useStrongholdGuardedAction } from "./useStrongholdGuardedAction";

export interface EnvCredentialMessage {
  type: "ok" | "err";
  text: string;
}

export interface EnvCredentialGuardCopy {
  title: string;
  description: string;
  lockedSave: string;
  lockedDelete: string;
}

export interface UseEnvCredentialOptions {
  /** 获取当前凭证 id；返回 null 表示当前不存在有效对象（例如未选中环境） */
  credentialId: () => string | null;
  /** Stronghold 解锁提示文案 */
  guard: EnvCredentialGuardCopy;
  /** 保存/清除成功时显示的提示 */
  saveOkText?: string;
  deleteOkText?: string;
}

export interface EnvCredentialState {
  exists: Ref<boolean>;
  loading: Ref<boolean>;
  message: Ref<EnvCredentialMessage | null>;
  save: (password: string) => Promise<boolean>;
  remove: () => Promise<boolean>;
  refresh: () => Promise<void>;
  reset: () => void;
}

export function useEnvCredential(opts: UseEnvCredentialOptions): EnvCredentialState {
  const exists = ref(false);
  const loading = ref(false);
  const message = ref<EnvCredentialMessage | null>(null);
  const guardedAction = useStrongholdGuardedAction();

  function reset() {
    exists.value = false;
    loading.value = false;
    message.value = null;
  }

  async function refresh() {
    const id = opts.credentialId();
    if (!id) {
      exists.value = false;
      return;
    }
    try {
      exists.value = await credentialExists(id);
    } catch (e) {
      const handled = await guardedAction.handleError(e, () => void refresh(), message, {
        title: opts.guard.title,
        description: opts.guard.description,
        lockedText: opts.guard.lockedSave,
      });
      if (!handled) exists.value = false;
    }
  }

  async function save(password: string): Promise<boolean> {
    const id = opts.credentialId();
    if (!id || !password) return false;
    loading.value = true;
    message.value = null;
    try {
      await credentialSave(id, password);
      exists.value = true;
      message.value = { type: "ok", text: opts.saveOkText ?? "密码已保存到安全存储" };
      return true;
    } catch (e) {
      await guardedAction.handleError(e, () => void save(password), message, {
        title: opts.guard.title,
        description: opts.guard.description,
        lockedText: opts.guard.lockedSave,
      });
      return false;
    } finally {
      loading.value = false;
    }
  }

  async function remove(): Promise<boolean> {
    const id = opts.credentialId();
    if (!id) return false;
    loading.value = true;
    message.value = null;
    try {
      await credentialDelete(id);
      exists.value = false;
      message.value = { type: "ok", text: opts.deleteOkText ?? "密码已清除" };
      return true;
    } catch (e) {
      await guardedAction.handleError(e, () => void remove(), message, {
        title: opts.guard.title,
        description: opts.guard.description,
        lockedText: opts.guard.lockedDelete,
      });
      return false;
    } finally {
      loading.value = false;
    }
  }

  return { exists, loading, message, save, remove, refresh, reset };
}
