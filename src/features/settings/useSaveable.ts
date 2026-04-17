import { ref } from "vue";
import { extractErrorMessage } from "../../utils/errorMessage";

/**
 * 统一设置保存逻辑的 composable。
 *
 * 用法：
 *   const { saving, message, runSave } = useSaveable();
 *   async function saveFoo(val: string) {
 *     await runSave(() => apiFoo(val));
 *   }
 *
 * @param autoClear 成功后是否在 2s 后自动清除 message（默认 true）
 */
export function useSaveable(autoClear = true) {
  const saving = ref(false);
  const message = ref<string | null>(null);

  async function runSave(fn: () => Promise<void>): Promise<void> {
    saving.value = true;
    message.value = null;
    try {
      await fn();
      message.value = "已保存";
      if (autoClear) setTimeout(() => (message.value = null), 2000);
    } catch (e) {
      message.value = extractErrorMessage(e);
    } finally {
      saving.value = false;
    }
  }

  return { saving, message, runSave };
}
