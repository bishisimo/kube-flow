import { ref } from "vue";
import { strongholdUnlock } from "../api/credential";
import { useStrongholdStatusStore } from "./strongholdStatus";

export interface StrongholdUnlockRequest {
  title?: string;
  description?: string;
  onConfirmed?: () => void;
}

const pending = ref<StrongholdUnlockRequest | null>(null);
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
    error.value = e instanceof Error ? e.message : String(e);
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
