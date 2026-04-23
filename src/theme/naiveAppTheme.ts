import type { GlobalThemeOverrides } from "naive-ui";

/** 浅色下 Workbench 列表表格：科技灰表头、白底表体（无斑马纹）。 */
const workbenchDataTable: NonNullable<GlobalThemeOverrides["DataTable"]> = {
  borderColor: "rgba(68, 64, 60, 0.12)",
  thColor: "#f8fafc",
  thColorHover: "#f1f5f9",
  tdColorHover: "rgba(254, 243, 199, 0.22)",
  tdColorStriped: "#ffffff",
  thTextColor: "#475569",
  tdTextColor: "#1c1917",
  thFontWeight: "650",
  borderRadius: "12px",
  tdPaddingSmall: "8px 12px",
  thPaddingSmall: "8px 12px",
};

/**
 * 应用壳 Naive 全局 token：主色、密度、圆角、工作台 DataTable、面包屑等。
 * 与 `NConfigProvider` 的 `theme`（null / `darkTheme`）配合；浅色时追加 DataTable 覆盖。
 */
export function buildNaiveThemeOverrides(isDark: boolean): GlobalThemeOverrides {
  const commonLight = {
    primaryColor: "#2563eb",
    primaryColorHover: "#1d4ed8",
    primaryColorPressed: "#1e40af",
  };
  const commonDark = {
    primaryColor: "#3b82f6",
    primaryColorHover: "#2563eb",
    primaryColorPressed: "#1d4ed8",
  };
  const shared: GlobalThemeOverrides = {
    common: {
      ...(isDark ? commonDark : commonLight),
      borderRadius: "10px",
      fontSize: "13px",
      fontSizeSmall: "12px",
      fontSizeMedium: "13px",
      fontSizeLarge: "15px",
      heightMedium: "34px",
      lineHeight: "1.45",
    },
    Button: {
      borderRadiusMedium: "10px",
      heightMedium: "32px",
      paddingMedium: "0 14px",
      fontSizeMedium: "13px",
    },
    Input: {
      borderRadius: "10px",
      heightMedium: "34px",
      fontSizeMedium: "13px",
    },
    Select: {
      peers: {
        InternalSelection: {
          borderRadius: "10px",
          heightMedium: "34px",
          fontSizeMedium: "13px",
          heightSmall: "1.85rem",
          fontSizeSmall: "13px",
        },
      },
    },
    Card: {
      borderRadius: "14px",
      paddingMedium: "18px",
    },
    Checkbox: {
      sizeMedium: "16px",
    },
    /** 仅 WorkbenchBreadcrumb 使用，与 `--wb-*` 变量语义对齐，减少 :deep 补丁。 */
    Breadcrumb: isDark
      ? {
          fontSize: "13px",
          separatorColor: "rgba(148, 163, 184, 0.45)",
          itemTextColor: "#94a3b8",
          itemTextColorHover: "#e2e8f0",
          itemTextColorPressed: "#e2e8f0",
          itemTextColorActive: "#f1f5f9",
          itemBorderRadius: "999px",
          itemColorHover: "rgba(59, 130, 246, 0.14)",
          itemColorPressed: "rgba(59, 130, 246, 0.2)",
        }
      : {
          fontSize: "13px",
          separatorColor: "#94a3b8",
          itemTextColor: "#64748b",
          itemTextColorHover: "#1d4ed8",
          itemTextColorPressed: "#1e40af",
          itemTextColorActive: "#0f172a",
          itemBorderRadius: "999px",
          itemColorHover: "rgba(255, 255, 255, 0.95)",
          itemColorPressed: "rgba(239, 246, 255, 0.95)",
        },
  };
  if (isDark) {
    return shared;
  }
  return { ...shared, DataTable: workbenchDataTable };
}
