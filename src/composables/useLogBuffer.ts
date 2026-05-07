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

function normalizeLevel(raw: unknown): LogLevel {
  if (typeof raw !== "string") return "unknown";
  return LEVEL_ALIASES[raw.toLowerCase()] ?? "unknown";
}

function extractLevelFromText(line: string): LogLevel {
  // [LEVEL] or [LEVEL]:
  const bracket = line.match(/\[\s*(error|fatal|panic|critical|err|warn(?:ing)?|wrn|info|notice|debug|trace|verbose)\s*\]/i);
  if (bracket) return normalizeLevel(bracket[1]);
  // level=VALUE (logfmt)
  const logfmt = line.match(/\blevel\s*=\s*(error|fatal|panic|critical|err|warn(?:ing)?|wrn|info|notice|debug|trace|verbose)\b/i);
  if (logfmt) return normalizeLevel(logfmt[1]);
  // severity=VALUE
  const sev = line.match(/\bseverity\s*[:=]\s*["']?(ERROR|FATAL|CRITICAL|WARN(?:ING)?|INFO|NOTICE|DEBUG|TRACE|VERBOSE)\b/i);
  if (sev) return normalizeLevel(sev[1]);
  // Bare keywords (last resort)
  if (/\b(?:ERROR|FATAL|PANIC|CRITICAL)\b/.test(line)) return "error";
  if (/\b(?:WARN|WARNING)\b/.test(line)) return "warn";
  if (/\bINFO\b/.test(line)) return "info";
  if (/\b(?:DEBUG|TRACE|VERBOSE)\b/.test(line)) return "debug";
  return "unknown";
}

// ─── Timestamp 解析 ───────────────────────────────────────

function extractTimestamp(line: string): { ms: number | null; raw: string | null } {
  // ISO 8601: 2024-01-15T10:30:00.000Z or 2024-01-15T10:30:00+08:00
  const iso = line.match(/\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?/);
  if (iso) {
    const ms = Date.parse(iso[0]);
    if (!Number.isNaN(ms)) return { ms, raw: iso[0] };
  }
  // Unix epoch seconds: 1705312200 (10+ digits)
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

  // 尝试 JSON 解析
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

  // 尝试 logfmt
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

  // 纯文本 fallback
  const { ms, raw: tsRaw } = extractTimestamp(trimmed);
  return { index, raw: trimmed, level: extractLevelFromText(trimmed), timestamp: ms, timestampRaw: tsRaw, fields: null };
}

// ─── Composable ───────────────────────────────────────────

export function useLogBuffer(options?: LogBufferOptions) {
  const maxLines = options?.maxLines ?? DEFAULT_MAX_LINES;
  const entries: Ref<LogEntry[]> = ref([]);
  let nextIndex = 0;

  /** 一次性设置全部内容（用于非 follow 模式加载） */
  function setLines(content: string) {
    const lines = content.split("\n");
    const result: LogEntry[] = [];
    for (const line of lines) {
      if (!line.trim()) continue;
      result.push(parseLogLine(line, nextIndex++));
    }
    entries.value = result;
  }

  /** 追加一块文本（用于 follow 流式接收） */
  function appendLines(chunk: string) {
    const lines = chunk.split("\n");
    const newEntries: LogEntry[] = [];
    for (const line of lines) {
      if (!line.trim()) continue;
      newEntries.push(parseLogLine(line, nextIndex++));
    }
    if (newEntries.length === 0) return;

    const combined = [...entries.value, ...newEntries];
    // 超过上限时淘汰头部
    if (combined.length > maxLines) {
      entries.value = combined.slice(combined.length - maxLines);
    } else {
      entries.value = combined;
    }
  }

  /** 清空缓冲区 */
  function clear() {
    entries.value = [];
    nextIndex = 0;
  }

  /** 按级别统计 */
  const levelCounts = computed(() => {
    const counts: Record<LogLevel, number> = { error: 0, warn: 0, info: 0, debug: 0, unknown: 0 };
    for (const entry of entries.value) {
      counts[entry.level]++;
    }
    return counts;
  });

  return {
    entries,
    setLines,
    appendLines,
    clear,
    levelCounts,
    /** 当前行数 */
    lineCount: computed(() => entries.value.length),
  };
}
