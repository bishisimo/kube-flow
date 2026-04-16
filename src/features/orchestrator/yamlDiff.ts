import * as jsYaml from "js-yaml";

export interface DiffRow {
  type: "context" | "added" | "removed" | "modified";
  leftLineNo: number | null;
  rightLineNo: number | null;
  leftText: string;
  rightText: string;
}

interface TokenOp {
  op: "=" | "-" | "+";
  text: string;
}

export function buildDiffRows(oldText: string, newText: string): DiffRow[] {
  const a = oldText.replace(/\r\n/g, "\n").split("\n");
  const b = newText.replace(/\r\n/g, "\n").split("\n");
  const dp: number[][] = Array.from({ length: a.length + 1 }, () => Array(b.length + 1).fill(0));
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const ops: Array<{ op: "=" | "-" | "+"; text: string }> = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      ops.push({ op: "=", text: a[i] });
      i++;
      j++;
      continue;
    }
    if (dp[i + 1][j] >= dp[i][j + 1]) {
      ops.push({ op: "-", text: a[i] });
      i++;
    } else {
      ops.push({ op: "+", text: b[j] });
      j++;
    }
  }
  while (i < a.length) ops.push({ op: "-", text: a[i++] });
  while (j < b.length) ops.push({ op: "+", text: b[j++] });

  const rows: DiffRow[] = [];
  let leftNo = 1;
  let rightNo = 1;
  let p = 0;
  while (p < ops.length) {
    const cur = ops[p];
    if (cur.op === "=") {
      rows.push({
        type: "context",
        leftLineNo: leftNo++,
        rightLineNo: rightNo++,
        leftText: cur.text,
        rightText: cur.text,
      });
      p++;
      continue;
    }

    const delBuf: string[] = [];
    const addBuf: string[] = [];
    while (p < ops.length && ops[p].op !== "=") {
      if (ops[p].op === "-") delBuf.push(ops[p].text);
      if (ops[p].op === "+") addBuf.push(ops[p].text);
      p++;
    }
    const modifiedCount = Math.min(delBuf.length, addBuf.length);
    for (let k = 0; k < modifiedCount; k++) {
      rows.push({
        type: "modified",
        leftLineNo: leftNo++,
        rightLineNo: rightNo++,
        leftText: delBuf[k],
        rightText: addBuf[k],
      });
    }
    for (let k = modifiedCount; k < delBuf.length; k++) {
      rows.push({
        type: "removed",
        leftLineNo: leftNo++,
        rightLineNo: null,
        leftText: delBuf[k],
        rightText: "",
      });
    }
    for (let k = modifiedCount; k < addBuf.length; k++) {
      rows.push({
        type: "added",
        leftLineNo: null,
        rightLineNo: rightNo++,
        leftText: "",
        rightText: addBuf[k],
      });
    }
  }
  return rows;
}

export function normalizeYamlForDiff(yaml: string): string {
  try {
    const parsed = jsYaml.load(yaml);
    if (!parsed || typeof parsed !== "object") return yaml;
    const obj = deepClone(parsed as Record<string, unknown>);
    const meta =
      obj.metadata && typeof obj.metadata === "object"
        ? (obj.metadata as Record<string, unknown>)
        : null;
    if (meta) {
      delete meta.uid;
      delete meta.resourceVersion;
      delete meta.managedFields;
      delete meta.creationTimestamp;
    }
    delete obj.status;
    return jsYaml.dump(obj, { lineWidth: -1, sortKeys: true });
  } catch {
    return yaml;
  }
}

export function formatCodeCell(row: DiffRow, side: "left" | "right"): string {
  const raw = side === "left" ? row.leftText : row.rightText;
  if (!raw) return "";
  if (row.type !== "modified") return escapeHtml(raw);
  return renderInlineDiff(row.leftText, row.rightText, side);
}

function deepClone<T>(value: T): T {
  if (Array.isArray(value)) {
    return value.map((item) => deepClone(item)) as T;
  }
  if (value && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
      out[k] = deepClone(v);
    }
    return out as T;
  }
  return value;
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function tokenizeForInlineDiff(text: string): string[] {
  return text.match(/(\s+|[^\s]+)/g) ?? [];
}

function buildTokenOps(left: string, right: string): TokenOp[] {
  const a = tokenizeForInlineDiff(left);
  const b = tokenizeForInlineDiff(right);
  const dp: number[][] = Array.from({ length: a.length + 1 }, () => Array(b.length + 1).fill(0));
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }

  const ops: TokenOp[] = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      ops.push({ op: "=", text: a[i] });
      i++;
      j++;
      continue;
    }
    if (dp[i + 1][j] >= dp[i][j + 1]) {
      ops.push({ op: "-", text: a[i] });
      i++;
    } else {
      ops.push({ op: "+", text: b[j] });
      j++;
    }
  }
  while (i < a.length) ops.push({ op: "-", text: a[i++] });
  while (j < b.length) ops.push({ op: "+", text: b[j++] });
  return ops;
}

function renderInlineDiff(left: string, right: string, side: "left" | "right"): string {
  const ops = buildTokenOps(left, right);
  const parts: string[] = [];
  for (const op of ops) {
    const text = escapeHtml(op.text);
    if (op.op === "=") {
      parts.push(text);
      continue;
    }
    if (side === "left" && op.op === "-") {
      parts.push(`<span class="inline-removed">${text}</span>`);
      continue;
    }
    if (side === "right" && op.op === "+") {
      parts.push(`<span class="inline-added">${text}</span>`);
      continue;
    }
  }
  return parts.join("");
}
