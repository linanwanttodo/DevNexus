// src/lib/errors.js — 后端错误 -> 本地化文案
// 三段式：结构化错误对象（{code, params, detail}）优先；其次旧字符串模式匹配；最后通用回退。
import { t } from "./i18n.js";

/** 旧版字符串错误模式（未迁移命令的过渡兼容，随模块迁移逐步缩减） */
export const LEGACY_PATTERNS = {
  "Provider already exists": "errors.provider_exists",
  "already exists": "errors.already_exists",
  "Invalid version string": "errors.invalid_version",
};

/** 判断 invoke 的 reject 值是否为结构化错误对象 */
export function isStructuredError(err) {
  return !!err && typeof err === "object" && typeof err.code === "string";
}

/** {name} 风格插值 */
export function interpolate(text, params) {
  if (!params) return text;
  return String(text).replace(/\{(\w+)\}/g, (m, k) =>
    Object.prototype.hasOwnProperty.call(params, k) ? String(params[k]) : m
  );
}

/** 把 invoke 抛出的错误（结构化对象或字符串）转为用户可读的本地化文案 */
export function friendlyError(err) {
  if (isStructuredError(err)) {
    const key = `errors.${err.code}`;
    const text = t(key);
    if (text && text !== key) return interpolate(text, err.params);
    if (err.detail) return err.detail;
    return err.code;
  }
  const msg = typeof err === "string" ? err : err?.message || String(err);
  for (const [pattern, key] of Object.entries(LEGACY_PATTERNS)) {
    if (msg.includes(pattern)) return t(key);
  }
  return msg;
}
