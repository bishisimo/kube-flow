/**
 * 带版本管理的 localStorage 抽象层。
 *
 * 功能：
 * - 统一 JSON 序列化/反序列化，错误时降级到 fallback（不静默吞掉）
 * - 每条数据附带版本号，版本变更时可通过 migrate 函数迁移旧数据
 * - QuotaExceeded 等写入错误打印警告而非静默忽略
 */

interface StorageRecord<T> {
  v: number;
  data: T;
}

export interface StorageOptions<T> {
  key: string;
  /** schema 版本号，升版本时旧数据会触发 migrate（若提供），否则降级为 fallback */
  version: number;
  fallback: T;
  migrate?: (old: unknown, oldVersion: number) => T;
}

export interface Storage<T> {
  read(): T;
  write(data: T): void;
}

export function createStorage<T>(opts: StorageOptions<T>): Storage<T> {
  const { key, version, fallback, migrate } = opts;

  function read(): T {
    try {
      const raw = localStorage.getItem(key);
      if (!raw) return fallback;
      const record = JSON.parse(raw) as StorageRecord<T>;
      if (!record || typeof record !== "object" || !("v" in record)) {
        // 旧格式（无版本字段）：尝试 migrate(raw 数据, 0)
        const legacy = JSON.parse(raw) as unknown;
        return migrate ? migrate(legacy, 0) : fallback;
      }
      if (record.v !== version) {
        return migrate ? migrate(record.data, record.v) : fallback;
      }
      return record.data;
    } catch (e) {
      console.warn(`[storage] read failed for "${key}":`, e);
      return fallback;
    }
  }

  function write(data: T): void {
    try {
      const record: StorageRecord<T> = { v: version, data };
      localStorage.setItem(key, JSON.stringify(record));
    } catch (e) {
      console.error(`[storage] write failed for "${key}":`, e);
    }
  }

  return { read, write };
}
