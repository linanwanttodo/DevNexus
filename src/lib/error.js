// src/lib/error.js — 全局错误状态（单一状态源）
// ErrorBoundary 直接使用本模块导出的 ref，禁止复制快照导致双份真相。
import { ref } from "vue";
import { tFormat } from "./i18n.js";
import { feedback } from "./feedback.js";

const errorInfo = ref(null);

export function getError() {
  return errorInfo;
}

export function clearError() {
  errorInfo.value = null;
}

/**
 * 唯一错误捕获入口：写入状态、弹一条错误 toast、输出 console。
 * @param {unknown} err 错误对象或任意值
 * @param {string|null} source 错误来源描述（组件栈 / 文件:行号）
 */
export function captureError(err, source = null) {
  const message = err instanceof Error ? err.message : String(err);
  errorInfo.value = {
    message,
    stack: source,
    timestamp: Date.now(),
  };
  feedback.toast.error(tFormat("common.error_msg", { error: message }));
  console.error("[ErrorBoundary] Caught error:", err, source);
}
