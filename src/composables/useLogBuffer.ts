/**
 * 日志行缓冲区管理。
 *
 * - 结构化解析（JSON / logfmt / 纯文本 fallback）
 * - 环形缓冲，超过 MAX_LINES 自动淘汰最旧行
 * - 入口处一次性解析 level / timestamp / fields，后续过滤直接使用
 */
import { ref, computed, type Ref } from "vue";

export type LogLevel = "error" | "warn" | "info" | "debug" | "unknown";

export interface LogEntry {
  /** 全局递增序号（用于虚拟列表 key 和匹配索引） */
  index: number;
  /** 原始文本 */
  raw: string;
  /** 解析后的日志级别 */
  level: LogLevel;
  /** 解析后的时间戳（ms），无法解析时为 null */
  timestamp: number | null;
  /** 原始时间戳字符串 */
  timestampRaw: string | null;
  /** JSON 日志的原始字段对象 */
  fields: Record<string, unknown> | null;
}

export interface LogBufferOptions {
  /** 最大保留行数，默认 10000 */
  maxLines?: number;
}

const DEFAULT_MAX_LINES = 10000;
/** 大批量解析时每批行数，批间让出主线程 */
const PARSE_YIELD_EVERY = 400;

// ─── Level 解析 ───────────────────────────────────────────

const LEVEL_ALIASES: Record<string, LogLevel> = {
  error: "error",
  err: "error",
  fatal: "error",
  panic: "error",
  critical: "error",
  warn: "warn",
  warning: "warn",
  wrn: "warn",
  info: "info",
  notice: "info",
  debug: "debug",
  trace: "debug",
  verbose: "debug",
  silly: "debug",
};

function emptyLevelCounts(): Record<LogLevel, number> {
  return { error: 0, warn: 0, info: 0, debug: 0, unknown: 0 };
}

function normalizeLevel(raw: unknown): LogLevel {
  if (typeof raw !== "string") return "unknown";
  return LEVEL_ALIASES[raw.toLowerCase()] ?? "unknown";
}

function extractLevelFromText(line: string): LogLevel {
  const bracket = line.match(/\[\s*(error|fatal|panic|critical|err|warn(?:ing)?|wrn|info|notice|debug|trace|verbose)\s*\]/i);
  if (bracket) return normalizeLevel(bracket[1]);
  const logfmt = line.match(/\blevel\s*=\s*(error|fatal|panic|critical|err|warn(?:ing)?|wrn|info|notice|debug|trace|verbose)\b/i);
  if (logfmt) return normalizeLevel(logfmt[1]);
  const sev = line.match(/\bseverity\s*[:=]\s*["']?(ERROR|FATAL|CRITICAL|WARN(?:ING)?|INFO|NOTICE|DEBUG|TRACE|VERBOSE)\b/i);
  if (sev) return normalizeLevel(sev[1]);
  if (/\b(?:ERROR|FATAL|PANIC|CRITICAL)\b/.test(line)) return "error";
  if (/\b(?:WARN|WARNING)\b/.test(line)) return "warn";
  if (/\bINFO\b/.test(line)) return "info";
  if (/\b(?:DEBUG|TRACE|VERBOSE)\b/.test(line)) return "debug";
  return "unknown";
}

// ─── Timestamp 解析 ───────────────────────────────────────

function extractTimestamp(line: string): { ms: number | null; raw: string | null } {
  const iso = line.match(/\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?/);
  if (iso) {
    const ms = Date.parse(iso[0]);
    if (!Number.isNaN(ms)) return { ms, raw: iso[0] };
  }
  const epoch = line.match(/\b(\d{10})(?:\.\d+)?\b/);
  if (epoch) {
    const sec = Number(epoch[1]);
    if (sec > 1_000_000_000 && sec < 2_000_000_000) {
      return { ms: sec * 1000, raw: epoch[1] };
    }
  }
  return { ms: null, raw: null };
}

// ─── Logfmt 解析 ──────────────────────────────────────────

function parseLogfmt(line: string): Record<string, string> | null {
  if (!line.includes("=")) return null;
  const pairs = line.match(/\b[a-zA-Z_][\w.]*=(?:"[^"]*"|[^\s]*)/g);
  if (!pairs || pairs.length < 2) return null;
  const result: Record<string, string> = {};
  for (const pair of pairs) {
    const eqIdx = pair.indexOf("=");
    const key = pair.slice(0, eqIdx);
    let val = pair.slice(eqIdx + 1);
    if (val.startsWith('"') && val.endsWith('"')) val = val.slice(1, -1);
    result[key] = val;
  }
  return result;
}

// ─── 行解析 ───────────────────────────────────────────────

function parseLogLine(raw: string, index: number): LogEntry {
  const trimmed = raw.trimEnd();
  if (!trimmed) {
    return { index, raw: trimmed, level: "unknown", timestamp: null, timestampRaw: null, fields: null };
  }

  if (trimmed.startsWith("{")) {
    try {
      const obj = JSON.parse(trimmed) as Record<string, unknown>;
      const level = normalizeLevel(
        obj.level ?? obj.severity ?? obj.lvl ?? obj.log_level ?? obj.logLevel
      );
      const tsRaw = obj.ts ?? obj.timestamp ?? obj.time ?? obj["@timestamp"] ?? obj.date;
      let tsMs: number | null = null;
      let tsStr: string | null = null;
      if (typeof tsRaw === "string") {
        tsStr = tsRaw;
        const parsed = Date.parse(tsRaw);
        if (!Number.isNaN(parsed)) tsMs = parsed;
      } else if (typeof tsRaw === "number") {
        tsMs = tsRaw > 1e12 ? tsRaw : tsRaw * 1000;
        tsStr = String(tsRaw);
      }
      return { index, raw: trimmed, level: level !== "unknown" ? level : extractLevelFromText(trimmed), timestamp: tsMs, timestampRaw: tsStr, fields: obj };
    } catch {
      // not valid JSON, fall through
    }
  }

  const logfmt = parseLogfmt(trimmed);
  if (logfmt) {
    const level = normalizeLevel(logfmt.level ?? logfmt.severity ?? logfmt.lvl);
    const tsStr = logfmt.ts ?? logfmt.timestamp ?? logfmt.time;
    let tsMs: number | null = null;
    if (tsStr) {
      const parsed = Date.parse(tsStr);
      if (!Number.isNaN(parsed)) tsMs = parsed;
    }
    return { index, raw: trimmed, level: level !== "unknown" ? level : extractLevelFromText(trimmed), timestamp: tsMs, timestampRaw: tsStr ?? null, fields: logfmt as unknown as Record<string, unknown> };
  }

  const { ms, raw: tsRaw } = extractTimestamp(trimmed);
  return { index, raw: trimmed, level: extractLevelFromText(trimmed), timestamp: ms, timestampRaw: tsRaw, fields: null };
}

function parseChunkLines(chunk: string, startIndex: number): { entries: LogEntry[]; nextIndex: number } {
  const lines = chunk.split("\n");
  const entries: LogEntry[] = [];
  let nextIndex = startIndex;
  for (const line of lines) {
    if (!line.trim()) continue;
    entries.push(parseLogLine(line, nextIndex++));
  }
  return { entries, nextIndex };
}

function yieldToMainThread(): Promise<void> {
  return new Promise((resolve) => {
    requestAnimationFrame(() => resolve());
  });
}

// ─── Composable ───────────────────────────────────────────

export function useLogBuffer(options?: LogBufferOptions) {
  const maxLines = options?.maxLines ?? DEFAULT_MAX_LINES;
  const entries: Ref<LogEntry[]> = ref([]);
  const levelCounts = ref<Record<LogLevel, number>>(emptyLevelCounts());
  let nextIndex = 0;

  function applyLevelDelta(batch: LogEntry[], sign: 1 | -1) {
    if (!batch.length) return;
    const counts = { ...levelCounts.value };
    for (const entry of batch) {
      counts[entry.level] += sign;
    }
    levelCounts.value = counts;
  }

  function commitEntries(newEntries: LogEntry[]) {
    if (newEntries.length === 0) return;

    let kept = entries.value;
    let dropped: LogEntry[] = [];
    const overflow = kept.length + newEntries.length - maxLines;
    if (overflow > 0) {
      dropped = kept.slice(0, overflow);
      kept = kept.slice(overflow);
    }

    applyLevelDelta(dropped, -1);
    applyLevelDelta(newEntries, 1);
    entries.value = kept.length ? [...kept, ...newEntries] : [...newEntries];
  }

  /** 一次性设置全部内容（用于非 follow 模式加载，小批量同步解析） */
  function setLines(content: string) {
    const { entries: parsed, nextIndex: ni } = parseChunkLines(content, nextIndex);
    nextIndex = ni;
    const final = parsed.length > maxLines ? parsed.slice(parsed.length - maxLines) : parsed;
    const counts = emptyLevelCounts();
    for (const entry of final) {
      counts[entry.level]++;
    }
    levelCounts.value = counts;
    entries.value = final;
  }

  /** 大批量快照加载：分批解析并让出主线程，避免长时间阻塞 UI */
  async function setLinesAsync(content: string): Promise<void> {
    const lines = content.split("\n");
    const result: LogEntry[] = [];
    let parsed = 0;
    for (const line of lines) {
      if (!line.trim()) continue;
      result.push(parseLogLine(line, nextIndex++));
      parsed += 1;
      if (parsed % PARSE_YIELD_EVERY === 0) {
        await yieldToMainThread();
      }
    }
    const final = result.length > maxLines ? result.slice(result.length - maxLines) : result;
    const counts = emptyLevelCounts();
    for (const entry of final) {
      counts[entry.level]++;
    }
    levelCounts.value = counts;
    entries.value = final;
  }

  /** 追加一块文本（用于 follow 流式接收）；同批内只触发一次响应式更新 */
  function appendLines(chunk: string) {
    const { entries: parsed, nextIndex: ni } = parseChunkLines(chunk, nextIndex);
    nextIndex = ni;
    commitEntries(parsed);
  }

  /** 统计追加块中的非空行数（用于新日志角标） */
  function countLinesInChunk(chunk: string): number {
    let count = 0;
    for (const line of chunk.split("\n")) {
      if (line.trim()) count += 1;
    }
    return count;
  }

  function clear() {
    entries.value = [];
    levelCounts.value = emptyLevelCounts();
    nextIndex = 0;
  }

  return {
    entries,
    setLines,
    setLinesAsync,
    appendLines,
    countLinesInChunk,
    clear,
    levelCounts,
    lineCount: computed(() => entries.value.length),
  };
}
