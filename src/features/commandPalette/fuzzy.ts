/**
 * 子序列模糊匹配：对查询串中每个字符在候选串中按顺序查找首个匹配位置，
 * 基于命中位置、连续性、词首性、完整前缀/子串等维度打分。
 *
 * 返回 { score, indices }，score 为 0 表示未命中（调用方应过滤掉）。
 * indices 为 query 字符在 target 中的实际下标序列，可用于高亮。
 */

export interface FuzzyMatch {
  score: number;
  indices: number[];
}

const SCORE_MISS = 0;
const SCORE_MATCH = 16;
const SCORE_CONSECUTIVE_BONUS = 14;
const SCORE_WORD_START_BONUS = 10;
const SCORE_CAMEL_BONUS = 4;
const SCORE_LEADING_GAP_PENALTY = 3;
const SCORE_GAP_PENALTY = 1;
const SCORE_PREFIX_BONUS = 30;
const SCORE_SUBSTRING_BONUS = 22;
const SCORE_EXACT_BONUS = 60;

function isWordBoundary(prev: string | undefined): boolean {
  if (!prev) return true;
  return !/[a-z0-9\u4e00-\u9fff]/i.test(prev);
}

export function fuzzyMatch(query: string, target: string): FuzzyMatch {
  if (!query) return { score: 0, indices: [] };
  if (!target) return { score: SCORE_MISS, indices: [] };

  const q = query.toLowerCase();
  const t = target.toLowerCase();

  if (q === t) {
    return { score: SCORE_EXACT_BONUS + q.length * SCORE_MATCH, indices: rangeIndices(q.length) };
  }

  const substringIdx = t.indexOf(q);
  if (substringIdx >= 0) {
    const base =
      (substringIdx === 0 ? SCORE_PREFIX_BONUS : SCORE_SUBSTRING_BONUS) + q.length * SCORE_MATCH;
    const leadingPenalty = substringIdx * SCORE_LEADING_GAP_PENALTY;
    const indices: number[] = [];
    for (let i = 0; i < q.length; i += 1) indices.push(substringIdx + i);
    return { score: Math.max(1, base - leadingPenalty), indices };
  }

  let score = 0;
  let qi = 0;
  let lastIdx = -2;
  const indices: number[] = [];

  for (let ti = 0; ti < t.length && qi < q.length; ti += 1) {
    if (t[ti] === q[qi]) {
      let pieceScore = SCORE_MATCH;
      if (ti === lastIdx + 1) pieceScore += SCORE_CONSECUTIVE_BONUS;
      else if (ti > 0) pieceScore -= Math.min(ti - lastIdx - 1, 6) * SCORE_GAP_PENALTY;
      if (isWordBoundary(t[ti - 1])) pieceScore += SCORE_WORD_START_BONUS;
      if (target[ti] && target[ti] !== t[ti]) pieceScore += SCORE_CAMEL_BONUS;
      score += pieceScore;
      indices.push(ti);
      lastIdx = ti;
      qi += 1;
    }
  }

  if (qi < q.length) return { score: SCORE_MISS, indices: [] };
  return { score: Math.max(1, score), indices };
}

function rangeIndices(n: number): number[] {
  const out: number[] = [];
  for (let i = 0; i < n; i += 1) out.push(i);
  return out;
}
