import { ref } from "vue";

/**
 * 工作台资源列表区域的 loading、错误与环境切换提示文案。
 * 环境切换时 `viewSessionId`、抽屉/菜单等仍由视图层自行重置。
 */
export function useWorkbenchListUi() {
  const listLoading = ref(false);
  const listError = ref<string | null>(null);
  const envSwitching = ref(false);
  const envSwitchingName = ref("");

  function dismissError() {
    listError.value = null;
  }

  /** 进入环境切换或清空当前环境时更新列表区状态；`displayName` 仅在 `nextEnvId` 非空时使用 */
  function beginListSwitch(nextEnvId: string | null, displayName: string) {
    listError.value = null;
    listLoading.value = !!nextEnvId;
    envSwitching.value = !!nextEnvId;
    envSwitchingName.value = nextEnvId ? displayName : "";
  }

  return {
    listLoading,
    listError,
    envSwitching,
    envSwitchingName,
    dismissError,
    beginListSwitch,
  };
}
