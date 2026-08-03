// src/lib/toast.js — 基于 Arco Message 的通知封装（保留原 showToast API）
import { Message } from "@arco-design/web-vue";

export function showToast(message, type = "info", duration = 3000) {
  const opts = { content: message, duration };
  switch (type) {
    case "success":
      Message.success(opts);
      break;
    case "error":
      Message.error(opts);
      break;
    case "warning":
      Message.warning(opts);
      break;
    case "loading":
      Message.loading(opts);
      break;
    default:
      Message.info(opts);
  }
}
