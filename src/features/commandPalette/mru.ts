/**
 * 命令面板的 Most-Recently-Used 记录：以 localStorage 持久化最近执行的命令 id 序列。
 * 供排序时加分：越靠前表示越近使用。
 */
import { ref } from "vue";
import { createStorage } from "../../utils/storage";

const STORAGE_KEY = "kube-flow:command-palette-mru";
const MAX_ENTRIES = 40;

const mruStorage = createStorage<string[]>({
  key: STORAGE_KEY,
  version: 1,
  fallback: [],
  migrate: (old) => (Array.isArray(old) ? (old as string[]).slice(0, MAX_ENTRIES) : []),
});

const mru = ref<string[]>(mruStorage.read());

export function useCommandMru() {
  function record(id: string) {
    const next = [id, ...mru.value.filter((x) => x !== id)].slice(0, MAX_ENTRIES);
    mru.value = next;
    mruStorage.write(next);
  }

  /** 返回 id 在 MRU 中的倒序得分：越近越高，未出现返回 0。 */
  function scoreOf(id: string): number {
    const idx = mru.value.indexOf(id);
    if (idx < 0) return 0;
    return Math.max(1, MAX_ENTRIES - idx);
  }

  return { mru, record, scoreOf };
}
