import { showToast } from "./toast.svelte.js";

let errorInfo = $state(null);

export function getError() {
  return errorInfo;
}

export function clearError() {
  errorInfo = null;
}

export function captureError(err, componentStack) {
  errorInfo = {
    message: err instanceof Error ? err.message : String(err),
    stack: componentStack,
    timestamp: Date.now()
  };

  // Show toast notification for user-facing errors
  if (err instanceof Error && err.message) {
    showToast(`Error: ${err.message}`, "error", 5000);
  }

  console.error("[ErrorBoundary] Caught error:", err, componentStack);
}

// Export as reactive state for derived
export const errorStore = {
  get value() { return errorInfo; },
  subscribe(fn) {
    fn(errorInfo);
    return () => {};
  }
};