import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import monacoEditorPlugin from "vite-plugin-monaco-editor";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

const monacoPlugin = typeof monacoEditorPlugin === "function" ? monacoEditorPlugin : monacoEditorPlugin.default;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
      vue(),
      monacoPlugin({ languageWorkers: ["editorWorkerService"] }),
    ],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1430,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
