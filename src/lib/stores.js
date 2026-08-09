// src/lib/stores.js — Vue 版轻量全局状态（主题）
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
