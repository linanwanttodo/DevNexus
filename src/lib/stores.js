// src/lib/stores.js — Vue 版轻量全局状态（主题 / 窗口置顶）
// 路由状态由 vue-router 接管（hash 模式），此处仅保留主题等全局偏好。
import { ref } from "vue";

const initialTheme =
  typeof window !== "undefined"
    ? localStorage.getItem("devnexus-theme") || "dark"
    : "dark";

const theme = ref(initialTheme);

export function getTheme() {
  return theme;
}

export function setTheme(pref) {
  theme.value = pref;
  if (typeof window !== "undefined") {
    localStorage.setItem("devnexus-theme", pref);
    applyTheme(pref);
  }
}

/** 解析偏好（light/dark/system）并应用到 DOM（Tailwind .dark + html data-theme） */
export function applyTheme(pref) {
  if (typeof window === "undefined") return;
  const resolved =
    pref === "system"
      ? window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      : pref;
  document.documentElement.classList.toggle("dark", resolved === "dark");
  document.documentElement.setAttribute("data-theme", resolved);
}

// ---- 窗口置顶（类似微信的"窗口置于顶层"）----
// 持久化到 localStorage，重启后 TitleBar 会恢复状态
const initialWindowTop =
  typeof window !== "undefined"
    ? localStorage.getItem("devnexus-window-top") === "1"
    : false;

const windowTop = ref(initialWindowTop);

export function getWindowTop() {
  return windowTop;
}

export function setWindowTop(value) {
  windowTop.value = value;
  if (typeof window !== "undefined") {
    localStorage.setItem("devnexus-window-top", value ? "1" : "0");
  }
}
