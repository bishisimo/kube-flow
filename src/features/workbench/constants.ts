/** 工作台（Main）本地存储键、UI 常量与跨资源类型的静态集合。 */

export const WORKBENCH_ENV_BAR_COLLAPSED_KEY = "kube-flow:env-bar-collapsed";
export const WORKBENCH_NS_FAVORITES_KEY = "kube-flow:ns-favorites";
export const WORKBENCH_NS_RECENT_KEY_PREFIX = "kube-flow:ns-recent:";
export const WORKBENCH_RECENT_KINDS_KEY = "kube-flow:recent-kinds";
export const WORKBENCH_FAVORITE_KINDS_KEY = "kube-flow:favorite-kinds";

export const WORKBENCH_MAX_RECENT_KINDS = 6;
/** 全命名空间列表占位，用于 watch key 等 */
export const WORKBENCH_ALL_NAMESPACES_SENTINEL = "__all__";

/** 节点分配信息轮询间隔 */
export const WORKBENCH_NODE_ALLOC_REFRESH_MS = 30_000;

/** 行操作菜单相对光标的偏移，避免菜单角与点击点完全重合 */
export const WORKBENCH_ACTION_MENU_OFFSET = 6;

/** 支持修改镜像的 API Kind 名 */
export const WORKBENCH_IMAGE_PATCH_KINDS = new Set(["Deployment", "StatefulSet", "DaemonSet"]);

/** 可打开 Shell 的工作负载 Kind */
export const WORKBENCH_SHELL_WORKLOAD_KINDS = new Set(["Pod", "Deployment", "StatefulSet", "DaemonSet"]);

/** 可打开节点终端的资源 Kind */
export const WORKBENCH_NODE_TERMINAL_RESOURCE_KINDS = new Set(["Node", "Pod"]);

/** 列表表头可排序列 key */
export const WORKBENCH_SORTABLE_KEYS = new Set([
  "name",
  "creationTime",
  "phase",
  "status",
  "replicas",
  "value",
]);
