/**
 * 全局日志视图状态：切换顺序/格式或调试日志级别时触发刷新。
 */
import { ref } from "vue";

export const logRefreshTrigger = ref(0);

export function useLogStore() {
  function triggerLogRefresh() {
    logRefreshTrigger.value++;
  }
  return { logRefreshTrigger, triggerLogRefresh };
}
