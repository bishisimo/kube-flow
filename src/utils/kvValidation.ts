import * as jsYaml from "js-yaml";

export interface KeyValueRow {
  key: string;
  value: string;
}

/** 检测单行是否含有控制字符或行尾空白（侧栏 badge 用）。 */
export function hasCharIssue(value: string, whitespaceEnabled: boolean): boolean {
  if (!whitespaceEnabled || !value) return false;
  for (const ch of value) {
    const code = ch.charCodeAt(0);
    if ((code < 0x20 && code !== 10 && code !== 9) || code === 0x7f) return true;
  }
  // 行尾空白
  if (/[ \t]+$/m.test(value)) return true;
  return false;
}

/** 根据 key 后缀推断格式提示。 */
export function getFormatHint(key: string): string {
  const ext = key.split(".").pop()?.toLowerCase() ?? "";
  if (["yaml", "yml"].includes(ext)) return "yaml";
  if (["json", "json5"].includes(ext)) return "json";
  return "";
}

/** 校验 value 是否符合 key 后缀所暗示的格式；返回不符合的 key 列表。空值视为通过。 */
export function validateFormatBySuffix(rows: KeyValueRow[]): string[] {
  const invalid: string[] = [];
  for (const r of rows) {
    const k = r.key.trim();
    if (!k) continue;
    const fmt = getFormatHint(k);
    const v = r.value.trim();
    if (!v) continue;
    if (fmt === "yaml") {
      try {
        jsYaml.load(v);
      } catch {
        invalid.push(k);
      }
    } else if (fmt === "json") {
      try {
        JSON.parse(v);
      } catch {
        invalid.push(k);
      }
    }
  }
  return invalid;
}

/** 校验 value 中是否含有非常规控制字符或行尾空白；返回有问题的 key 列表。 */
export function validateControlChars(rows: KeyValueRow[], whitespaceEnabled: boolean): string[] {
  const problematic: string[] = [];
  for (const r of rows) {
    if (!r.key.trim() || !r.value) continue;
    if (hasCharIssue(r.value, whitespaceEnabled)) {
      problematic.push(r.key.trim());
    }
  }
  return problematic;
}

export function dumpInlineScalar(value: string): string {
  return jsYaml.dump(value, { lineWidth: -1 }).trim();
}

export function renderStringEntry(key: string, value: string, indent = "  "): string[] {
  const renderedKey = dumpInlineScalar(key);
  if (!value.includes("\n")) {
    return [`${indent}${renderedKey}: ${dumpInlineScalar(value)}`];
  }
  const lines = value.split("\n");
  const hasTrailingNewline = value.endsWith("\n");
  const header = `${indent}${renderedKey}: |${hasTrailingNewline ? "" : "-"}`;
  return [header, ...lines.map((line) => `${indent}  ${line}`)];
}

export function renderSection(name: string, entries: KeyValueRow[]): string[] {
  const validEntries = entries.filter((r) => r.key.trim());
  if (validEntries.length === 0) return [];
  const lines = [`${name}:`];
  for (const row of validEntries) {
    lines.push(...renderStringEntry(row.key.trim(), row.value));
  }
  return lines;
}
