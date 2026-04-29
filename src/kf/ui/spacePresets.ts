/**
 * NSpace 预设：应用壳与工作台内横向/栅格间距的单一来源，避免各处魔法数字与不一致的 wrap/align。
 * 使用方式：`<NSpace v-bind="kfSpace.toolbarActions">...</NSpace>`
 */
export type KfSpaceBind = {
  align?: "stretch" | "baseline" | "start" | "end" | "center" | "flex-end" | "flex-start";
  justify?: "start" | "end" | "center" | "space-around" | "space-between" | "space-evenly";
  wrap?: boolean;
  vertical?: boolean;
  size?: number | string | [number | string, number | string];
};

export const kfSpace = {
  /** 应用壳顶栏：品牌 + 主导航 */
  shellTopbar: {
    align: "center" as const,
    wrap: false,
    size: 12,
  },
  /** 工作台面包屑行：标签 + 路径 */
  breadcrumb: {
    align: "center" as const,
    wrap: false,
    size: 9,
  },
  /** 页面标题行：左标题 / 右主操作 */
  pageTitle: {
    align: "center" as const,
    wrap: true,
    size: 16,
    justify: "space-between" as const,
  },
  /** 空态/加载区操作或图标+文案水平居中 */
  centered: {
    align: "center" as const,
    justify: "center" as const,
    wrap: true,
    size: 8,
  },
  /** 空态底部按钮组 */
  centeredActions: {
    align: "center" as const,
    justify: "center" as const,
    wrap: true,
    size: 8,
  },
  /** 环境条内：环境名 + 多枚 Tag */
  envBanner: {
    align: "center" as const,
    wrap: true,
    size: 6,
  },
  /** 工具条右侧：实时更新 / 刷新 / 批量操作 */
  toolbarActions: {
    align: "center" as const,
    wrap: true,
    size: 8,
  },
  /** 筛选主行：名称 + Label，均分宽度由外层 :deep 处理 */
  filterInputs: {
    align: "center" as const,
    wrap: false,
    size: 8,
  },
  /** 筛选次行：Node / IP，靠右对齐 */
  filterTail: {
    align: "center" as const,
    wrap: true,
    size: 8,
    justify: "end" as const,
  },
  /** 已选筛选 Chip 行 */
  chipRow: {
    align: "center" as const,
    wrap: true,
    size: 8,
  },
  /** 抽屉标题行：资源名 | 开关 */
  drawerTitle: {
    align: "center" as const,
    wrap: true,
    size: 12,
    justify: "space-between" as const,
  },
  /** 状态条：断连 / 错误（左文右按钮） */
  alertRow: {
    align: "center" as const,
    wrap: true,
    size: 12,
    justify: "space-between" as const,
  },
  /** 小行内图标 + 文案（连接中、加载） */
  inlineStatus: {
    align: "center" as const,
    wrap: false,
    size: 8,
  },
  /** 日志/终端顶栏：左信息 | 右工具 */
  contextBar: {
    align: "center" as const,
    wrap: true,
    size: 10,
    justify: "space-between" as const,
  },
  /** 日志顶栏左侧：类型 + 标题 + 元信息 */
  contextBarMain: {
    align: "center" as const,
    wrap: true,
    size: 10,
  },
  /** 日志对比模式下单个 pane 标题行 */
  logPaneHead: {
    align: "center" as const,
    wrap: true,
    size: 10,
  },
  /** 终端侧栏：环境选择 + 打开 */
  quickOpen: {
    align: "center" as const,
    wrap: false,
    size: 8,
  },
  /** 终端上下文条内左侧成组（Tag + 选择器） */
  terminalContextMain: {
    align: "center" as const,
    wrap: true,
    size: 10,
  },
  homeEnvRow: {
    align: "baseline" as const,
    wrap: false,
    size: 10,
  },
  /** 设置：数字 + 保存、表单项横排 */
  settingInline: {
    align: "center" as const,
    wrap: true,
    size: 10,
  },
  /** 设置：纵向分组（GPU 规则块等） */
  settingStack: {
    vertical: true,
    size: 8,
  },
  settingStackLoose: {
    vertical: true,
    size: 10,
  },
  /** 设置：调试「格式/顺序」标签 + 按钮组 */
  optionGroup: {
    vertical: true,
    size: 8,
  },
  /** 设置：代码主题行（标签 + NSelect） */
  editorThemeRow: {
    align: "center" as const,
    wrap: true,
    size: 12,
  },
  /** 设置：仅换行包裹多枚按钮 */
  buttonGroup: {
    wrap: true,
  },
  /** 设置：GPU 规则底部操作 */
  settingActions: {
    wrap: true,
    size: 10,
  },
  /** 编排中心顶栏 */
  orchestratorToolbar: {
    align: "center" as const,
    wrap: true,
    size: 12,
  },
  /** 资源 / 应用包切换 */
  viewSwitch: {
    align: "center" as const,
    size: 6,
  },
  /** 编排侧栏元信息操作 */
  metaActions: {
    align: "center" as const,
    wrap: true,
    size: 8,
  },
  /** 编排空编辑器操作区 */
  editorEmptyActions: {
    align: "center" as const,
    justify: "center" as const,
    wrap: true,
    size: 10,
  },
  /** 应用包对话等紧凑按钮组 */
  compactActions: {
    wrap: true,
    size: 8,
  },
} as const satisfies Record<string, KfSpaceBind>;

export type KfSpacePreset = keyof typeof kfSpace;
