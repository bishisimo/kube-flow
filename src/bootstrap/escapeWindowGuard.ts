/**
 * Tauri / WKWebView 下未消费的 Escape 可能被壳层当成窗口快捷键（例如缩放、最小化），
 * 与「放弃 / 关闭浮层」的语义冲突。
 *
 * 在 document 捕获阶段对 Escape 调用 preventDefault（不 stopPropagation），
 * 子组件仍能收到同一事件以关闭弹窗；Monaco、xterm 等需将 Escape 交给编辑器/PTY 的区域不拦截。
 */
const SKIP_ESCAPE_PREVENT_DEFAULT = ".monaco-editor, .xterm, .xterm-helper-textarea";

export function installEscapeWindowGuard(): void {
  document.addEventListener(
    "keydown",
    (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      const t = e.target;
      if (t instanceof Element && t.closest(SKIP_ESCAPE_PREVENT_DEFAULT)) return;
      e.preventDefault();
    },
    { capture: true },
  );
}
