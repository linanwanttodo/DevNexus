// src/main.js — Vue 3 + Arco Design 入口
import { createApp } from "vue";
import ArcoVue from "@arco-design/web-vue";
import ArcoVueIcon from "@arco-design/web-vue/es/icon";
import "@arco-design/web-vue/dist/arco.css";

import App from "./App.vue";
import { router } from "./router.js";
import { initI18n } from "./lib/i18n.js";
import { applyTheme } from "./lib/stores.js";
import "./styles/tokens.css";
import "./styles/app.css";

async function bootstrap() {
  // 读取偏好（主题 + 语言）
  const lang = localStorage.getItem("devnexus-lang") || navigator.language.slice(0, 2) || "en";
  const themePref = localStorage.getItem("devnexus-theme") || "dark";

  await initI18n(lang);
  applyTheme(themePref);

  const app = createApp(App);
  app.use(router);
  app.use(ArcoVue);
  app.use(ArcoVueIcon);
  app.mount("#app");

  // 标记就绪，淡入（配合 index.html 防闪烁脚本）
  const el = document.getElementById("app");
  if (el) el.classList.add("ready");
}

bootstrap();
