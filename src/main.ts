import { createApp } from "vue";
import App from "./App.vue";
import router from "./router";

/** 全局禁用浏览器默认右键菜单，统一使用应用内交互。 */
window.addEventListener("contextmenu", (event) => {
  event.preventDefault();
});

const app = createApp(App);
app.use(router);
app.mount("#app");
