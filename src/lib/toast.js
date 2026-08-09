// src/lib/toast.js — 基于 sonner 的通知封装（保留原 showToast API）
import { toast } from "vue-sonner";

export function showToast(message, type = "info", duration = 3000) {
  switch (type) {
    case "success":
      toast.success(message, { duration });
      break;
    case "error":
      toast.error(message, { duration });
      break;
    case "warning":
      toast.warning(message, { duration });
      break;
    case "loading":
      toast.loading(message, { duration });
      break;
    default:
      toast(message, { duration });
  }
}