// src/lib/errors.js — 后端错误文案 → i18n key 映射（Vue 版）
import { t } from "./i18n.js";

/** 已知的后端错误文案 → i18n key（懒解析，保证语言切换后取到当前语言的文案） */
const knownPatterns = {
  "Provider already exists": "errors.provider_exists",
  "already exists": "errors.already_exists",
  "Invalid version string": "errors.invalid_version",
};

/** 把 invoke 抛出的错误（字符串或对象）转为用户可读的本地化文案 */
export function friendlyError(err) {
  const msg = typeof err === "string" ? err : err?.message || String(err);
  for (const [pattern, key] of Object.entries(knownPatterns)) {
    if (msg.includes(pattern)) return t(key);
  }
  return msg;
}
