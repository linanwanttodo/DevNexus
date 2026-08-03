<script setup>
import { ref, onMounted, onBeforeUnmount, onErrorCaptured } from "vue";
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
        <icon-exclamation-circle-fill class="error-icon" />
        <h2 class="error-title">{{ t("error.title") || "Something went wrong" }}</h2>
      </div>

      <p class="error-msg">{{ errorInfo.message }}</p>

      <details v-if="errorInfo.stack" class="error-details">
        <summary class="error-summary">
          {{ t("error.details") || "Component details" }}
        </summary>
        <pre class="error-stack">{{ errorInfo.stack }}</pre>
      </details>

      <div class="error-actions">
        <a-button @click="() => window.location.reload()">
          {{ t("error.reload") || "Reload page" }}
        </a-button>
        <a-button type="primary" @click="handleClearError">
          {{ t("error.dismiss") || "Dismiss" }}
        </a-button>
      </div>
    </div>
  </div>

  <!-- 正常内容 -->
  <slot v-else />
</template>

<style scoped>
.error-overlay {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(3px);
}

.error-box {
  max-width: 420px;
  margin: 0 16px;
  padding: 24px;
  border-radius: 10px;
  border: 1px solid var(--color-danger-6, rgba(255, 92, 92, 0.5));
  background-color: var(--color-bg-2);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.4);
}

.error-head {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 14px;
}

.error-icon {
  font-size: 26px;
  color: var(--color-danger-6);
}

.error-title {
  font-size: 17px;
  font-weight: 600;
  margin: 0;
  color: var(--color-text-1);
}

.error-msg {
  margin: 0 0 14px;
  font-size: 13px;
  color: var(--color-text-2);
  word-break: break-all;
}

.error-summary {
  cursor: pointer;
  font-size: 12px;
  color: var(--color-text-3);
  margin-bottom: 6px;
}

.error-stack {
  max-height: 130px;
  overflow: auto;
  margin: 0 0 14px;
  padding: 8px;
  border-radius: 6px;
  background-color: var(--color-fill-1);
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
  color: var(--color-text-3);
}

.error-actions {
  display: flex;
  gap: 12px;
}
</style>
