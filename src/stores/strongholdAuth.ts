import { computed, ref } from "vue";
import { strongholdUnlock } from "../api/credential";
import { extractErrorMessage } from "../utils/errorMessage";
import { useStrongholdStatusStore } from "./strongholdStatus";

export interface StrongholdUnlockRequest {
  title?: string;
  description?: string;
  onConfirmed?: () => void;
}

const pending = ref<StrongholdUnlockRequest | null>(null);

/**
 * 仍为打开状态的父级 NModal / NDrawer 上绑定 `:trap-focus`：
 * Stronghold 解锁层通过 Teleport 挂在 body，若父层保持焦点陷阱，无法在解锁输入框中输入。
 */
export const strongholdAdjacentModalTrapFocusEnabled = computed(() => pending.value === null);
const loading = ref(false);
const error = ref<string | null>(null);
const { setStrongholdStatus } = useStrongholdStatusStore();

function isStrongholdLockedError(message: string): boolean {
  return message.includes("Stronghold 已锁定");
}

async function checkAndHandle(
  errorMessage: string,
  onConfirmed?: () => void,
  request?: Omit<StrongholdUnlockRequest, "onConfirmed">
): Promise<boolean> {
  if (!isStrongholdLockedError(errorMessage)) return false;
  pending.value = {
    title: request?.title,
    description: request?.description,
    onConfirmed,
  };
  loading.value = false;
  error.value = null;
  return true;
}

async function confirm(password: string) {
  if (!pending.value) return;
  loading.value = true;
  error.value = null;
  try {
    await strongholdUnlock(password);
    setStrongholdStatus("unlocked");
    const onConfirmed = pending.value.onConfirmed;
    pending.value = null;
    loading.value = false;
    onConfirmed?.();
  } catch (e) {
    error.value = extractErrorMessage(e);
    loading.value = false;
  }
}

function cancel() {
  pending.value = null;
  loading.value = false;
  error.value = null;
}

export function useStrongholdAuthStore() {
  return {
    pending,
    loading,
    error,
    checkAndHandle,
    confirm,
    cancel,
  };
}
