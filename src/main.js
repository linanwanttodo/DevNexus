import "./app.css";
import App from "./App.svelte";
import { mount } from "svelte";
import { initI18n } from "./lib/i18n.svelte.js";

// 从 localStorage 恢复语言偏好，等待加载完成后再挂载
// 避免侧边栏因翻译未就绪而闪烁
const savedLang = localStorage.getItem("devnexus-lang") || "zh";

let app;
initI18n(savedLang).then(() => {
  app = mount(App, {
    target: document.getElementById("app"),
  });
  // 翻译和组件就绪后显示 UI，避免原始 key 闪现
  requestAnimationFrame(() => {
    document.getElementById("app").classList.add("ready");
  });
});

export default app;