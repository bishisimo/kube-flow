/**
 * YAML/编辑 代码高亮主题：为浅色/深色应用壳各保存一套，随界面明暗切换；YAML 与编辑页共用。
 */
import { computed, ref, watch } from "vue";
import * as monaco from "monaco-editor";
import { createStorage } from "../utils/storage";
import { appChromeIsDark } from "./appChromeTheme";

export const YAML_THEMES = [
  { id: "atom-one-dark", label: "Atom One Dark" },
  { id: "atom-one-light", label: "Atom One Light" },
  { id: "monokai", label: "Monokai" },
  { id: "dracula", label: "Dracula" },
  { id: "nord", label: "Nord" },
  { id: "github", label: "GitHub" },
  { id: "github-dark", label: "GitHub Dark" },
  { id: "vs2015", label: "VS 2015" },
  { id: "color-brewer", label: "Color Brewer" },
  { id: "rainbow", label: "Rainbow" },
  { id: "tomorrow-night-bright", label: "Tomorrow Night Bright" },
  { id: "shades-of-purple", label: "Shades of Purple" },
] as const;
type YamlThemeId = (typeof YAML_THEMES)[number]["id"];

const MONACO_THEME_PREFIX = "kube-flow-";
const THEME_PAIR_KEY = "kube-flow:yaml-themes";
const THEME_LEGACY_SINGLE_KEY = "kube-flow:yaml-theme";

const YAML_DARK_THEMES = new Set<YamlThemeId>([
  "atom-one-dark",
  "monokai",
  "dracula",
  "nord",
  "github-dark",
  "vs2015",
  "tomorrow-night-bright",
  "shades-of-purple",
]);

function isYamlThemeId(id: string): id is YamlThemeId {
  return YAML_THEMES.some((t) => t.id === id);
}

function normalizeLightEditorThemeId(id: string): YamlThemeId {
  if (isYamlThemeId(id) && !YAML_DARK_THEMES.has(id)) return id;
  return "atom-one-light";
}

function normalizeDarkEditorThemeId(id: string): YamlThemeId {
  if (isYamlThemeId(id) && YAML_DARK_THEMES.has(id)) return id;
  return "atom-one-dark";
}

/** 供设置页 NSelect：仅适合浅色应用壳的 Monaco 主题。 */
export const EDITOR_LIGHT_THEME_OPTIONS = YAML_THEMES.filter((t) => !YAML_DARK_THEMES.has(t.id)).map(
  (t) => ({ label: t.label, value: t.id })
);
/** 供设置页 NSelect：仅适合深色应用壳的 Monaco 主题。 */
export const EDITOR_DARK_THEME_OPTIONS = YAML_THEMES.filter((t) => YAML_DARK_THEMES.has(t.id)).map((t) => ({
  label: t.label,
  value: t.id,
}));

const themePairStorage = createStorage<{ light: YamlThemeId; dark: YamlThemeId }>({
  key: THEME_PAIR_KEY,
  version: 1,
  fallback: { light: "atom-one-light", dark: "atom-one-dark" },
  migrate: (old) => {
    if (old && typeof old === "object" && "light" in old && "dark" in old) {
      const o = old as { light: unknown; dark: unknown };
      return {
        light: normalizeLightEditorThemeId(typeof o.light === "string" ? o.light : "atom-one-light"),
        dark: normalizeDarkEditorThemeId(typeof o.dark === "string" ? o.dark : "atom-one-dark"),
      };
    }
    return { light: "atom-one-light", dark: "atom-one-dark" };
  },
});

function readLegacySingleThemeId(): string | null {
  const raw = localStorage.getItem(THEME_LEGACY_SINGLE_KEY);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (parsed && typeof parsed === "object" && "v" in parsed) {
      const d = (parsed as { data?: unknown }).data;
      return typeof d === "string" ? d : null;
    }
  } catch {
    return null;
  }
  return null;
}

function pairFromLegacySingleId(id: string): { light: YamlThemeId; dark: YamlThemeId } {
  if (isYamlThemeId(id) && YAML_DARK_THEMES.has(id)) {
    return { light: "atom-one-light", dark: id };
  }
  if (isYamlThemeId(id) && !YAML_DARK_THEMES.has(id)) {
    return { light: id, dark: "atom-one-dark" };
  }
  return { light: "atom-one-light", dark: "atom-one-dark" };
}

function loadEditorThemePairFromStorage(): { light: YamlThemeId; dark: YamlThemeId } {
  if (localStorage.getItem(THEME_PAIR_KEY) === null) {
    const legacy = readLegacySingleThemeId();
    if (legacy) {
      const p = pairFromLegacySingleId(legacy);
      themePairStorage.write(p);
      try {
        localStorage.removeItem(THEME_LEGACY_SINGLE_KEY);
      } catch {
        /* ignore */
      }
      return p;
    }
  }
  const r = themePairStorage.read();
  return { light: normalizeLightEditorThemeId(r.light), dark: normalizeDarkEditorThemeId(r.dark) };
}

const _editorPairInit = loadEditorThemePairFromStorage();
const themeIdLight = ref<YamlThemeId>(_editorPairInit.light);
const themeIdDark = ref<YamlThemeId>(_editorPairInit.dark);

/** 根据当前应用壳明暗，选择对应的编辑器语法高亮主题（若配置异常则归一化）。 */
export const activeYamlThemeId = computed<YamlThemeId>(() =>
  appChromeIsDark.value
    ? normalizeDarkEditorThemeId(themeIdDark.value)
    : normalizeLightEditorThemeId(themeIdLight.value)
);

function monacoThemeName(id: YamlThemeId): string {
  return `${MONACO_THEME_PREFIX}${id}`;
}

const YAML_THEME_COLORS: Record<YamlThemeId, monaco.editor.IColors> = {
  "atom-one-dark": {
    "editor.background": "#282c34",
    "editor.foreground": "#abb2bf",
    "editorLineNumber.foreground": "#5c6370",
    "editorLineNumber.activeForeground": "#d7dae0",
  },
  "atom-one-light": {
    "editor.background": "#fafafa",
    "editor.foreground": "#383a42",
    "editorLineNumber.foreground": "#a0a1a7",
    "editorLineNumber.activeForeground": "#383a42",
  },
  monokai: {
    "editor.background": "#272822",
    "editor.foreground": "#f8f8f2",
    "editorLineNumber.foreground": "#75715e",
    "editorLineNumber.activeForeground": "#f8f8f2",
  },
  dracula: {
    "editor.background": "#282a36",
    "editor.foreground": "#f8f8f2",
    "editorLineNumber.foreground": "#6272a4",
    "editorLineNumber.activeForeground": "#f8f8f2",
  },
  nord: {
    "editor.background": "#2e3440",
    "editor.foreground": "#d8dee9",
    "editorLineNumber.foreground": "#616e88",
    "editorLineNumber.activeForeground": "#eceff4",
  },
  github: {
    "editor.background": "#ffffff",
    "editor.foreground": "#24292f",
    "editorLineNumber.foreground": "#a1a1aa",
    "editorLineNumber.activeForeground": "#1f2937",
  },
  "github-dark": {
    "editor.background": "#0d1117",
    "editor.foreground": "#c9d1d9",
    "editorLineNumber.foreground": "#6e7681",
    "editorLineNumber.activeForeground": "#c9d1d9",
  },
  vs2015: {
    "editor.background": "#1e1e1e",
    "editor.foreground": "#d4d4d4",
    "editorLineNumber.foreground": "#858585",
    "editorLineNumber.activeForeground": "#c6c6c6",
  },
  "color-brewer": {
    "editor.background": "#f7f4ef",
    "editor.foreground": "#2f2a26",
    "editorLineNumber.foreground": "#b8a79a",
    "editorLineNumber.activeForeground": "#4a3f35",
  },
  rainbow: {
    "editor.background": "#fdfcff",
    "editor.foreground": "#3f3d56",
    "editorLineNumber.foreground": "#b2a8d3",
    "editorLineNumber.activeForeground": "#5b4b8a",
  },
  "tomorrow-night-bright": {
    "editor.background": "#1d1f21",
    "editor.foreground": "#eaeaea",
    "editorLineNumber.foreground": "#969896",
    "editorLineNumber.activeForeground": "#ffffff",
  },
  "shades-of-purple": {
    "editor.background": "#2d2b55",
    "editor.foreground": "#ffffff",
    "editorLineNumber.foreground": "#a599e9",
    "editorLineNumber.activeForeground": "#ffffff",
  },
};

const YAML_THEME_RULES: Record<YamlThemeId, monaco.editor.ITokenThemeRule[]> = {
  "atom-one-dark": [
    { token: "keyword", foreground: "C678DD" },
    { token: "string", foreground: "98C379" },
    { token: "number", foreground: "D19A66" },
    { token: "type", foreground: "56B6C2" },
  ],
  "atom-one-light": [
    { token: "keyword", foreground: "A626A4" },
    { token: "string", foreground: "50A14F" },
    { token: "number", foreground: "986801" },
    { token: "type", foreground: "0184BC" },
  ],
  monokai: [
    { token: "keyword", foreground: "F92672" },
    { token: "string", foreground: "E6DB74" },
    { token: "number", foreground: "AE81FF" },
    { token: "type", foreground: "66D9EF" },
  ],
  dracula: [
    { token: "keyword", foreground: "FF79C6" },
    { token: "string", foreground: "F1FA8C" },
    { token: "number", foreground: "BD93F9" },
    { token: "type", foreground: "8BE9FD" },
  ],
  nord: [
    { token: "keyword", foreground: "81A1C1" },
    { token: "string", foreground: "A3BE8C" },
    { token: "number", foreground: "B48EAD" },
    { token: "type", foreground: "88C0D0" },
  ],
  github: [
    { token: "keyword", foreground: "CF222E" },
    { token: "string", foreground: "0A3069" },
    { token: "number", foreground: "0550AE" },
    { token: "type", foreground: "8250DF" },
  ],
  "github-dark": [
    { token: "keyword", foreground: "FF7B72" },
    { token: "string", foreground: "A5D6FF" },
    { token: "number", foreground: "79C0FF" },
    { token: "type", foreground: "D2A8FF" },
  ],
  vs2015: [
    { token: "keyword", foreground: "569CD6" },
    { token: "string", foreground: "CE9178" },
    { token: "number", foreground: "B5CEA8" },
    { token: "type", foreground: "4EC9B0" },
  ],
  "color-brewer": [
    { token: "keyword", foreground: "7F3B08" },
    { token: "string", foreground: "1B7837" },
    { token: "number", foreground: "A6611A" },
    { token: "type", foreground: "2166AC" },
  ],
  rainbow: [
    { token: "keyword", foreground: "8E44AD" },
    { token: "string", foreground: "27AE60" },
    { token: "number", foreground: "E67E22" },
    { token: "type", foreground: "2980B9" },
  ],
  "tomorrow-night-bright": [
    { token: "keyword", foreground: "C397D8" },
    { token: "string", foreground: "B9CA4A" },
    { token: "number", foreground: "DE935F" },
    { token: "type", foreground: "81A2BE" },
  ],
  "shades-of-purple": [
    { token: "keyword", foreground: "FF9D00" },
    { token: "string", foreground: "A5FF90" },
    { token: "number", foreground: "FF628C" },
    { token: "type", foreground: "9EFFFF" },
  ],
};

let monacoThemesReady = false;

function ensureMonacoYamlThemesRegistered() {
  if (monacoThemesReady) return;
  for (const theme of YAML_THEMES) {
    const id = theme.id;
    monaco.editor.defineTheme(monacoThemeName(id), {
      base: YAML_DARK_THEMES.has(id) ? "vs-dark" : "vs",
      inherit: true,
      rules: YAML_THEME_RULES[id],
      colors: YAML_THEME_COLORS[id],
    });
  }
  monacoThemesReady = true;
}

watch([themeIdLight, themeIdDark], () => {
  themePairStorage.write({
    light: normalizeLightEditorThemeId(themeIdLight.value),
    dark: normalizeDarkEditorThemeId(themeIdDark.value),
  });
});

export function useYamlTheme() {
  return {
    themeIdLight,
    themeIdDark,
    activeYamlThemeId,
  };
}

/** 返回 Monaco 主题名；随应用壳明暗在「浅色编辑器主题 / 深色编辑器主题」之间切换。 */
export function useYamlMonacoTheme() {
  ensureMonacoYamlThemesRegistered();
  const monacoTheme = computed(() =>
    isYamlThemeId(activeYamlThemeId.value)
      ? monacoThemeName(activeYamlThemeId.value)
      : monacoThemeName("atom-one-dark")
  );
  return { monacoTheme };
}
