/**
 * 应用壳明暗（与 YAML/Monaco 编辑主题解耦，避免编辑器深色与主界面被强行绑在一起）。
 */
import { ref, computed, watch } from "vue";
import { createStorage } from "../utils/storage";

export type AppChromeScheme = "light" | "dark" | "system";

const storage = createStorage<AppChromeScheme>({
  key: "kube-flow:app-chrome-scheme",
  version: 1,
  fallback: "light",
  migrate: (old) => {
    if (old === "dark" || old === "light" || old === "system") return old;
    return "light";
  },
});

export const appChromeScheme = ref<AppChromeScheme>(storage.read());

const mediaDark = ref(
  typeof window !== "undefined" ? window.matchMedia("(prefers-color-scheme: dark)").matches : false
);
if (typeof window !== "undefined") {
  const m = window.matchMedia("(prefers-color-scheme: dark)");
  m.addEventListener("change", (e) => {
    mediaDark.value = e.matches;
  });
}

/** 供 Naive `NConfigProvider`、`:root` 的 `data-kf-chrome` 等使用。 */
export const appChromeIsDark = computed(() => {
  if (appChromeScheme.value === "light") return false;
  if (appChromeScheme.value === "dark") return true;
  return mediaDark.value;
});

export function setAppChromeScheme(s: AppChromeScheme) {
  appChromeScheme.value = s;
  storage.write(s);
}

export const APP_CHROME_OPTIONS: { label: string; value: AppChromeScheme }[] = [
  { label: "浅色", value: "light" },
  { label: "深色", value: "dark" },
  { label: "跟随系统", value: "system" },
];

watch(
  appChromeIsDark,
  (dark) => {
    if (typeof document === "undefined") return;
    document.documentElement.dataset.kfChrome = dark ? "dark" : "light";
  },
  { immediate: true }
);
