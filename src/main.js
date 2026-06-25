import "./app.css";
import App from "./App.svelte";
import { mount } from "svelte";
import { initI18n } from "./lib/i18n.svelte.js";

// 从 localStorage 恢复语言偏好，等待加载完成后再挂载
// 避免侧边栏因翻译未就绪而闪烁
const savedLang = localStorage.getItem("devnexus-lang") || "en";

let app;
initI18n(savedLang).then(() => {
  app = mount(App, {
    target: document.getElementById("app"),
  });
});

export default app;