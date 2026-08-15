<script setup>
import { ref, onMounted, onBeforeUnmount, onErrorCaptured } from "vue";
import { Button } from "@/components/ui/button";
import AppIcon from "./AppIcon.vue";
import { showToast } from "../lib/toast.js";
import { t } from "../lib/i18n.js";
import { getError, clearError as clearStoredError, captureError } from "../lib/error.js";

const errorInfo = ref(getError().value);

// 子组件错误捕获（Vue onErrorCaptured）
onErrorCaptured((err, _instance, info) => {
  errorInfo.value = {
    message: err instanceof Error ? err.message : String(err),
    stack: info,
    timestamp: Date.now(),
  };
  captureError(err, info);
  showToast(`Error: ${err.message || "Unknown error"}`, "error", 5000);
  return false; // 阻止继续冒泡，避免全局 handler 重复弹 toast
});

let removeHandlers = [];

function reloadPage() {
  window.location.reload();
}

onMounted(() => {
  // 全局未捕获错误
  const handleError = (event) => {
    const err = event.error || new Error(String(event.error));
    captureError(err, event.loc);
    errorInfo.value = {
      message: err instanceof Error ? err.message : String(err),
      stack: event.loc,
      timestamp: Date.now(),
    };
    showToast(`Error: ${err.message || "Unknown error"}`, "error", 5000);
    event.preventDefault();
  };

  // 全局未处理 Promise 拒绝
  const handleRejection = (event) => {
    const err = event.reason || new Error("Unhandled promise rejection");
    captureError(err, null);
    errorInfo.value = {
      message: err instanceof Error ? err.message : String(err),
      stack: null,
      timestamp: Date.now(),
    };
    showToast(`Error: ${err.message || "Unknown error"}`, "error", 5000);
    event.preventDefault();
  };

  window.addEventListener("error", handleError);
  window.addEventListener("unhandledrejection", handleRejection);
  removeHandlers = [handleError, handleRejection];
});

onBeforeUnmount(() => {
  window.removeEventListener("error", removeHandlers[0]);
  window.removeEventListener("unhandledrejection", removeHandlers[1]);
});

function handleClearError() {
  errorInfo.value = null;
  clearStoredError();
}
</script>

<template>
  <!-- 兜底错误遮罩 -->
  <div v-if="errorInfo" class="error-overlay">
    <div class="error-box">
      <div class="error-head">
        <AppIcon name="exclamation-circle-fill" class="size-7 text-destructive" />
        <h2 class="error-title">{{ t("error.title") || "Something went wrong" }}</h2>
      </div>

      <p class="error-msg">{{ errorInfo.message }}</p>

      <details v-if="errorInfo.stack" class="error-details">
        <summary class="error-summary">
          {{ t("error.details") || "Component details" }}
        </summary>
        <pre class="error-stack">{{ errorInfo.stack }}</pre>
      </details>

      <div class="flex gap-3">
        <Button variant="outline" @click="reloadPage">
          {{ t("error.reload") || "Reload page" }}
        </Button>
        <Button @click="handleClearError">
          {{ t("error.dismiss") || "Dismiss" }}
        </Button>
      </div>
    </div>
  </div>

  <!-- 正常内容 -->
  <slot v-else />
</template>

<style scoped>
/* 错误遮罩：只盖内容区、让出标题栏（top:36px），保证窗口仍可拖动/关闭，
   避免全窗口被冻结成"点不动、移不动"——这是用户反馈的核心问题。 */
.error-overlay {
  position: fixed;
  top: 36px; /* 标题栏高度，标题栏保持可拖拽 + 窗口按钮可用 */
  right: 0;
  bottom: 0;
  left: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: rgb(0 0 0 / 0.55);
  backdrop-filter: blur(3px);
}

.error-box {
  max-width: 420px;
  margin: 0 16px;
  padding: 24px;
  border-radius: var(--radius-lg);
  border: 1px solid var(--color-destructive);
  background-color: var(--color-card);
  box-shadow: 0 20px 60px rgb(0 0 0 / 0.4);
}

.error-head {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 14px;
}

.error-title {
  font-size: 17px;
  font-weight: 600;
  margin: 0;
  color: var(--color-foreground);
}

.error-msg {
  margin: 0 0 14px;
  font-size: 13px;
  color: var(--color-muted-foreground);
  word-break: break-all;
}

.error-summary {
  cursor: pointer;
  font-size: 12px;
  color: var(--color-muted-foreground);
  margin-bottom: 6px;
}

.error-stack {
  max-height: 130px;
  overflow: auto;
  margin: 0 0 14px;
  padding: 8px;
  border-radius: var(--radius-md);
  background-color: var(--color-muted);
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
  color: var(--color-muted-foreground);
}
</style>