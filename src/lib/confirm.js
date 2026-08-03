// src/lib/confirm.js — 基于 Arco Modal.confirm 的确认对话框封装（保留原 Promise API）
import { Modal } from "@arco-design/web-vue";
import { t } from "./i18n.js";

/**
 * 弹出确认对话框，返回 Promise<boolean>
 * @param {string} message 提示文案
 * @param {string} [title] 标题（默认 "Confirm"）
 * @param {object} [opts] 额外选项：{ okText, cancelText, danger, icon }
 */
export function showConfirm(message, title = "Confirm", opts = {}) {
  return new Promise((resolve) => {
    Modal.confirm({
      title: title || t("common.confirm") || "Confirm",
      content: message,
      okText: opts.okText || t("common.confirm") || "Confirm",
      cancelText: opts.cancelText || t("common.cancel") || "Cancel",
      okButtonProps: opts.danger ? { status: "danger" } : undefined,
      onOk: () => resolve(true),
      onCancel: () => resolve(false),
    });
  });
}
