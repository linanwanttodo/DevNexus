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

// ---- 窗口置顶 ----
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

// ---- 灵动岛 ----
// 持久化到 localStorage，主应用启动时按此恢复悬浮窗显示。
// 默认开启：未显式关闭（"0"）即视为启用——打开软件默认显示灵动岛，
// 与设置页开关默认开启保持一致（开关显示必须与真实行为一致）。
const initialIslandEnabled =
  typeof window !== "undefined"
    ? localStorage.getItem("devnexus-island-enabled") !== "0"
    : true;

const islandEnabled = ref(initialIslandEnabled);

export function getIslandEnabled() {
  return islandEnabled;
}

// syncOnly=true 时只回写本地状态（localStorage + 内存 ref），不反向调用 Rust 命令。
// 用途：托盘菜单切换后 Rust 会广播 island-state 事件，前端收到后只需同步本地状态；
// 若此处再 invoke island_set_enabled，Rust 又会广播 island-state → 形成无限循环
// （前端→Rust→事件→前端→Rust…），导致桌面反复开/关灵动岛。
// 只有用户在前端主动切换（侧边栏/标题栏/设置页）时才 push 到 Rust（syncOnly=false）。
export function setIslandEnabled(value, syncOnly = false) {
  islandEnabled.value = value;
  if (typeof window !== "undefined") {
    localStorage.setItem("devnexus-island-enabled", value ? "1" : "0");
    if (syncOnly) return;
    // 同步到 Rust 侧持久化状态，保证托盘菜单 check 项与前端开关一致
    import("@tauri-apps/api/core")
      .then(({ invoke }) => invoke("island_set_enabled", { enabled: value }))
      .catch(() => {});
  }
}

// ---- DeepSeek API Key（灵动岛余额查询用）----
// 双写：localStorage（本窗口回显）+ Rust 内存（跨窗口共享，岛窗口读取）。
// 不要只用 localStorage——Tauri 多窗口的 localStorage 按 origin 隔离，
// 主窗口写入后岛窗口读不到，会一直显示"未配置 Key"。
const initialDeepSeekKey =
  typeof window !== "undefined"
    ? localStorage.getItem("devnexus-deepseek-key") || ""
    : "";

const deepSeekKey = ref(initialDeepSeekKey);

export function getDeepSeekKey() {
  return deepSeekKey;
}

export function setDeepSeekKey(value) {
  deepSeekKey.value = value;
  if (typeof window !== "undefined") {
    localStorage.setItem("devnexus-deepseek-key", value);
    // 同步到 Rust 侧内存，供独立窗口（灵动岛）读取
    import("@tauri-apps/api/core")
      .then(({ invoke }) => invoke("deepseek_set_key", { key: value }))
      .catch(() => {});
  }
}
