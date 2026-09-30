// src/lib/confirm.js — 全局确认对话框（Promise API，基于 shadcn Dialog）
// 并发调用按序排队展示：每次 resolve 后自动推进下一个，先前的 Promise 不再被覆盖悬挂。
import { ref } from "vue";
import { t } from "./i18n.js";

export const confirmState = ref(null);

const confirmQueue = [];

function advanceConfirmQueue() {
  const next = confirmQueue.shift();
  if (next) {
    confirmState.value = next;
  } else {
    confirmState.value = null;
  }
}

/**
 * 弹出确认对话框，返回 Promise<boolean>
 * @param {string} message 提示文案
 * @param {string} [title] 标题（默认取 common.confirm）
 * @param {object} [opts] 额外选项：{ okText, cancelText, danger }
 */
export function showConfirm(message, title, opts = {}) {
  return new Promise((resolve) => {
    confirmQueue.push({
      message,
      title: title || t("common.confirm"),
      okText: opts.okText || t("common.confirm"),
      cancelText: opts.cancelText || t("common.cancel"),
      danger: !!opts.danger,
      resolve,
    });
    if (confirmState.value === null) advanceConfirmQueue();
  });
}

/** 由 ConfirmDialog 调用，结算当前确认框并推进队列 */
export function confirmResolve(result) {
  const state = confirmState.value;
  if (state) state.resolve(result);
  advanceConfirmQueue();
}
