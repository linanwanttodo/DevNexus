// src/lib/i18n.js — Vue 版国际化（复用现有 locales JSON 与回退链）
import { ref } from "vue";

const lang = ref("en");
const translations = ref({});
const version = ref(0);

// 语言加载回退链：目标语言 → 中文 → 英文，保证 translations 永不为空，
// 避免 t() 返回原始键名导致侧边栏闪现 "nav.xxx" 之类的未翻译文案
const FALLBACK_LANGS = ["zh", "en"];

export async function initI18n(l) {
  const candidates = [l, ...FALLBACK_LANGS.filter((x) => x !== l)];
  let lastError = null;
  for (const cand of candidates) {
    try {
      const loaded = (await import(`../locales/${cand}.json`)).default;
      if (loaded && typeof loaded === "object" && Object.keys(loaded).length > 0) {
        translations.value = loaded;
        lang.value = cand;
        version.value++;
        localStorage.setItem("devnexus-lang", lang.value);
        // 同步系统托盘菜单文案（Rust 侧持久化语言并更新菜单文字）
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          await invoke("update_tray_menu", { lang: cand });
        } catch {
          // 非 Tauri 环境忽略
        }
        return;
      }
    } catch (e) {
      lastError = e;
    }
  }
  // 全部失败：保留空对象（t() 会返回空串而非键名），并记录错误
  console.error("Failed to load any language pack:", lastError);
}

export function t(key) {
  if (Object.keys(translations.value).length === 0) {
    return ""; // 翻译未就绪：返回空串，绝不返回原始键名（防止 UI 闪现键）
  }
  const keys = key.split(".");
  let val = translations.value;
  for (const k of keys) {
    val = val?.[k];
  }
  return typeof val === "string" && val.length > 0 ? val : key;
}

export function tFormat(key, vars) {
  let text = t(key);
  if (typeof text === "string") {
    for (const [k, v] of Object.entries(vars)) {
      text = text.replaceAll(`{${k}}`, v);
    }
  }
  return text;
}

export function getLang() {
  return lang;
}

export function getVersion() {
  return version;
}
