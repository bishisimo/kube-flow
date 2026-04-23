import "./stores/appChromeTheme";
import { createApp } from "vue";
import App from "./App.vue";
import router from "./router";
import { installDefaultSpellcheckPolicy } from "./bootstrap/spellcheckPolicy";
import "./styles/kf-select-toolbar.css";

/** 全局禁用浏览器默认右键菜单，统一使用应用内交互。 */
window.addEventListener("contextmenu", (event) => {
  event.preventDefault();
});

const app = createApp(App);
app.use(router);

app.config.errorHandler = (err, _instance, info) => {
  console.error("[Vue Error]", info, err);
};

window.addEventListener("unhandledrejection", (event) => {
  console.error("[Unhandled Promise]", event.reason);
});

const root = document.getElementById("app");
app.mount("#app");
if (root) installDefaultSpellcheckPolicy(root);
