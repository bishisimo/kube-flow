/**
 * 命令面板类型定义。
 *
 * 概念模型：
 * - L1：CommandItem 作为"单条命令"，直接出现在候选列表（如 tab 切换、全局动作）
 * - L2：TokenSpec 声明一类 token（如 `@ns`），按 symbol/key/value 语法组合输入
 * - L3：Executor 读取已 committed 的 Token 数组 + freeText，产出可执行计划（ExecutorPlan）
 *
 * UI 数据源依 draft 状态切换：
 * - mode = "free"   → CommandItem 候选
 * - mode = "keying" → TokenSpec.key 候选
 * - mode = "valuing" → TokenSpec.values() 候选
 */
import type { VNodeChild } from "vue";

/* ---------------- L1：命令项 ---------------- */

export type CommandCategory = "nav" | "action" | "session";

export interface CommandItem {
  id: string;
  title: string;
  subtitle?: string;
  hint?: string;
  section?: string;
  keywords?: string[];
  category: CommandCategory;
  icon?: string | (() => VNodeChild);
  availableWhen?: () => boolean;
  run: () => void | Promise<void>;
  weight?: number;
  context?: string;
}

export interface ScoredCommand {
  item: CommandItem;
  score: number;
  matchedIndices: number[];
  matchedField: "title" | "subtitle" | "keywords" | null;
}

export type CommandProvider = () => CommandItem[];

/* ---------------- L2：Token 语法 ---------------- */

export type TokenSymbol = "@" | "#" | ">";

export interface Token {
  symbol: TokenSymbol;
  key: string;
  value: string;
}

export interface TokenValueCandidate {
  /** 最终写入 Token.value 的字符串。 */
  value: string;
  title: string;
  subtitle?: string;
  hint?: string;
  icon?: string;
  /** 模糊匹配评分时的额外关键字。 */
  keywords?: string[];
  /** 在 valuing 面板中覆盖 spec.label 作为分组标题，用于区分展示（如"已打开"/"可打开"）。 */
  section?: string;
}

/** 一类 token 的声明（如"@ns = 命名空间"）。 */
export interface TokenSpec {
  symbol: TokenSymbol;
  key: string;
  /** 在 keying 候选中显示的标签，如"命名空间"。 */
  label: string;
  /** 简短说明，展示在候选副行。 */
  hint?: string;
  icon?: string;
  /** 仅在此上下文（tab id）生效；留空表示所有上下文。 */
  context?: string;
  /** 返回 value 候选；query 为用户在 value 位已输入的字符串。 */
  values: (query: string) => TokenValueCandidate[];
  /**
   * 是否自由文本（如 #name / #label）。
   * freeText = true 时允许 value 无候选匹配也可 commit（直接用户原文）。
   */
  freeText?: boolean;
  /** keying 排序权重。 */
  weight?: number;
}

/* ---------------- L3：执行计划 ---------------- */

export interface ExecutorContext {
  tokens: Token[];
  /** 未被 token 吃掉的自由文本（来自 draft 的 freeText 部分或首次 commit 前的文本）。 */
  freeText: string;
  /** 当前 tab 标识（用于上下文敏感的 executor）。 */
  context: string | null;
}

export interface ExecutorPlan {
  /** 预览栏显示，如"在 kube-system 列 Pods · 按 nginx 过滤"。 */
  title: string;
  /** 副标题，进一步说明。 */
  subtitle?: string;
  icon?: string;
  run: () => void | Promise<void>;
}

export interface Executor {
  id: string;
  /** 仅在此上下文生效；留空表示所有上下文。 */
  context?: string;
  /** 数字越大优先级越高。相同分数按 id 字典序稳定排序。 */
  priority: number;
  /** 返回 null 表示不匹配当前 tokens。 */
  match: (ctx: ExecutorContext) => ExecutorPlan | null;
}

/* ---------------- Draft 解析产物 ---------------- */

export type DraftMode = "free" | "keying" | "valuing";

export interface Draft {
  mode: DraftMode;
  /** keying / valuing 下必存。 */
  symbol?: TokenSymbol;
  /** keying 下是"正在输入的 key 字符"；valuing 下是"已确定的 key"。 */
  keyBuffer?: string;
  /** valuing 下的 value 原始字符串。 */
  value?: string;
  /** free 下的原文；其它模式下等同 raw draft。 */
  rawText: string;
}
