/**
 * 工作台资源与命令面板「>」资源动作桥接：由 Main 挂载时注册，供 TokenSpec.values / Executor 调用。
 */
import { shallowRef } from "vue";
import type { TokenValueCandidate } from "../features/commandPalette/types";

export const workbenchResourcePaletteAdapter = shallowRef<{
  getValueCandidates: () => TokenValueCandidate[];
  runAction: (id: string) => void;
} | null>(null);
