// src/lib/error.js — Vue 版全局错误捕获
import { ref } from "vue";
import { showToast } from "./toast.js";

const errorInfo = ref(null);

export function getError() {
  return errorInfo;
}

export function clearError() {
  errorInfo.value = null;
}

export function captureError(err, componentStack) {
  errorInfo.value = {
    message: err instanceof Error ? err.message : String(err),
    stack: componentStack,
    timestamp: Date.now(),
  };

  // Show toast notification for user-facing errors
  if (err instanceof Error && err.message) {
    showToast(`Error: ${err.message}`, "error", 5000);
  }

  console.error("[ErrorBoundary] Caught error:", err, componentStack);
}
