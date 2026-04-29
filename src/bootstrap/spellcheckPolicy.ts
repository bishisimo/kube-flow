/**
 * 为技术类输入（资源名、Kind、label、查询等）默认关闭系统拼写/拆词建议；
 * 若需要自然语言拼写检查，在元素上设置 spellcheck="true" 或 data-kf-spellcheck="on"。
 */
const OPT_IN_ATTR = "data-kf-spellcheck";

function isSpellcheckOptIn(el: HTMLInputElement | HTMLTextAreaElement): boolean {
  return el.spellcheck === true || el.getAttribute("spellcheck") === "true" || el.getAttribute(OPT_IN_ATTR) === "on";
}

function isTextLikeInput(el: HTMLInputElement): boolean {
  const t = (el.type || "text").toLowerCase();
  return t === "text" || t === "search" || t === "url" || t === "tel" || t === "email" || t === "";
}

function applyEl(el: HTMLInputElement | HTMLTextAreaElement): void {
  if (isSpellcheckOptIn(el)) return;
  if (el instanceof HTMLInputElement && !isTextLikeInput(el)) return;
  el.spellcheck = false;
  el.setAttribute("autocorrect", "off");
  el.setAttribute("autocapitalize", "off");
}

function scan(root: ParentNode): void {
  root.querySelectorAll("input, textarea").forEach((node) => {
    if (node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement) applyEl(node);
  });
}

/** 在子树内对新出现的 input/textarea 应用策略。 */
export function installDefaultSpellcheckPolicy(root: ParentNode): () => void {
  scan(root);
  const mo = new MutationObserver((records) => {
    for (const r of records) {
      for (const n of r.addedNodes) {
        if (!(n instanceof Element)) continue;
        if (n instanceof HTMLInputElement || n instanceof HTMLTextAreaElement) applyEl(n);
        else scan(n);
      }
    }
  });
  mo.observe(root, { childList: true, subtree: true });
  return () => mo.disconnect();
}
