// src/lib/confirm.js — 全局确认对话框（Promise API，基于 shadcn Dialog）
// 用法不变：showConfirm(message, title, opts) => Promise<boolean>
// 由 ConfirmDialog 组件（挂在 App.vue）消费 confirmState 渲染。
import { ref } from "vue";
import { t } from "./i18n.js";

export const confirmState = ref(null);

/**
 * 弹出确认对话框，返回 Promise<boolean>
 * @param {string} message 提示文案
 * @param {string} [title] 标题（默认 "Confirm"）
 * @param {object} [opts] 额外选项：{ okText, cancelText, danger }
 */
export function showConfirm(message, title = "Confirm", opts = {}) {
  return new Promise((resolve) => {
    confirmState.value = {
      message,
      title: title || t("common.confirm") || "Confirm",
      okText: opts.okText || t("common.confirm") || "Confirm",
      cancelText: opts.cancelText || t("common.cancel") || "Cancel",
      danger: !!opts.danger,
      resolve,
    };
  });
}

/** 由 ConfirmDialog 调用，结算当前确认框 */
export function confirmResolve(result) {
  const state = confirmState.value;
  if (state) {
    state.resolve(result);
    confirmState.value = null;
  }
}