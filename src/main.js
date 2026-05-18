import "./app.css";
import App from "./App.svelte";
import { mount } from "svelte";
import { initI18n } from "./lib/i18n.js";

// 从 localStorage 恢复语言偏好
const savedLang = localStorage.getItem("devnexus-lang") || "en";
initI18n(savedLang);

const app = mount(App, {
  target: document.getElementById("app"),
});

export default app;