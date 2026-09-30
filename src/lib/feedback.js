// src/lib/feedback.js — 统一用户反馈门面
// 视图层反馈统一走这里：toast/confirm/sudo/loading。
// 文案约定：优先传 i18n 键（支持 {name} 插值）；键缺失时自动按原文案处理。
import { showToast } from "./toast.js";
import { showConfirm } from "./confirm.js";
import { promptSudo } from "./sudo.js";
import { friendlyError } from "./errors.js";
import { t } from "./i18n.js";

/** i18n 键优先的文案解析：t(key) 与键名相同视为缺键，回退原文案并做插值 */
function tf(keyOrText, params) {
  if (typeof keyOrText !== "string") return String(keyOrText ?? "");
  const translated = t(keyOrText);
  const text = translated && translated !== keyOrText ? translated : keyOrText;
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (m, k) =>
    Object.prototype.hasOwnProperty.call(params, k) ? String(params[k]) : m
  );
}

export const feedback = {
  toast: {
    success: (keyOrText, params) => showToast(tf(keyOrText, params), "success"),
    info: (keyOrText, params) => showToast(tf(keyOrText, params), "info"),
    warning: (keyOrText, params) => showToast(tf(keyOrText, params), "warning"),
    /** err 可为结构化错误对象（{code,...}）或字符串，内部走 friendlyError */
    error: (err) => showToast(friendlyError(err), "error"),
  },
  confirm: {
    /** 危险操作确认：{ titleKey, descKey, okKey?, cancelKey? } => Promise<boolean> */
    danger(opts) {
      return showConfirm(t(opts.descKey), t(opts.titleKey), {
        danger: true,
        okText: opts.okKey ? t(opts.okKey) : undefined,
        cancelText: opts.cancelKey ? t(opts.cancelKey) : undefined,
      });
    },
  },
  sudo: {
    /** { reasonKey } => Promise<string|null>，null 表示用户取消 */
    prompt(opts) {
      return promptSudo(t(opts.reasonKey));
    },
  },
  loading: {
    /** 统一长任务包装：失败自动弹错误 toast 后 rethrow，调用方可再捕获做局部处理 */
    async wrap(fn) {
      try {
        return await fn();
      } catch (err) {
        feedback.toast.error(err);
        throw err;
      }
    },
  },
};
