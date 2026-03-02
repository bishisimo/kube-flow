/**
 * YAML/编辑 代码高亮主题：供设置页配置，YAML 与编辑页共用。
 */
import { ref, watch } from "vue";

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

const STORAGE_KEY = "kube-flow:yaml-theme";

function loadTheme(): string {
  try {
    const s = localStorage.getItem(STORAGE_KEY);
    if (s && YAML_THEMES.some((t) => t.id === s)) return s;
  } catch {}
  return "atom-one-dark";
}

const themeId = ref<string>(loadTheme());

watch(themeId, (id) => {
  if (YAML_THEMES.some((t) => t.id === id)) {
    try {
      localStorage.setItem(STORAGE_KEY, id);
    } catch {}
  }
});

export function useYamlTheme() {
  function setTheme(id: string) {
    if (YAML_THEMES.some((t) => t.id === id)) {
      themeId.value = id;
    }
  }

  return { themeId, setTheme };
}
