/**
 * Draft 字符串解析：把用户未 commit 的输入片段解析为结构化的 Draft。
 *
 * 规则：
 * - 空字符串 / 非符号开头 → free 模式，整段作为 freeText
 * - `>` 开头 → valuing：`>` 后整段为动作筛选串（无二级 key），keyBuffer 固定为 ""
 * - `@` `#` 开头 + 尚无空格 → keying 模式，剩余字符是 key buffer
 * - `@` / `#`：符号 + key + 空格 + 可选 value → valuing 模式
 *
 * 注意 Draft 始终只承载"最后一个正在编辑的 token"。已 commit 的 token 走 tokens[]。
 */
import type { Draft, Token, TokenSymbol } from "./types";

const TOKEN_SYMBOLS: readonly TokenSymbol[] = ["@", "#", ">"] as const;

export function isTokenSymbol(ch: string): ch is TokenSymbol {
  return ch === "@" || ch === "#" || ch === ">";
}

export function parseDraft(draft: string): Draft {
  if (!draft) {
    return { mode: "free", rawText: "", value: "" };
  }
  const first = draft[0];
  if (first === ">") {
    const rest = draft.slice(1);
    return { mode: "valuing", symbol: ">", keyBuffer: "", value: rest, rawText: draft };
  }
  if (!isTokenSymbol(first)) {
    return { mode: "free", rawText: draft, value: draft };
  }
  const rest = draft.slice(1);
  const spaceIdx = rest.indexOf(" ");
  if (spaceIdx < 0) {
    return { mode: "keying", symbol: first, keyBuffer: rest, rawText: draft };
  }
  const key = rest.slice(0, spaceIdx);
  const value = rest.slice(spaceIdx + 1);
  return { mode: "valuing", symbol: first, keyBuffer: key, value, rawText: draft };
}

/** 将 draft + 选中的 key 候选合并，用于"选 key 后自动续写一个空格进入 valuing"。 */
export function writeKeySelection(symbol: TokenSymbol, key: string): string {
  return `${symbol}${key} `;
}

/** 将 draft + 选中的 value 候选合并为一个完整 chip 文本，用于 commit。 */
export function commitDraftToToken(
  draft: Draft,
  pickedValue: string | null,
  label?: string,
): Token | null {
  if (draft.mode !== "valuing" || draft.symbol === undefined || draft.keyBuffer === undefined) return null;
  const value = pickedValue ?? (draft.value ?? "").trim();
  if (!value) return null;
  return { symbol: draft.symbol, key: draft.keyBuffer, value: { raw: value, label: label ?? value } };
}

/** 序列化 Token 数组用于调试 / 展示。 */
export function stringifyTokens(tokens: Token[]): string {
  return tokens
    .map((t) =>
      t.symbol === ">" && t.key === "" ? `${t.symbol}${t.value.raw}` : `${t.symbol}${t.key}=${t.value.raw}`,
    )
    .join(" ");
}

export { TOKEN_SYMBOLS };
