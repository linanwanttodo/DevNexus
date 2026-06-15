<script>
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.svelte.js";
  import { t } from "../lib/i18n.svelte.js";
  import { getError, clearError as clearStoredError, captureError } from "../lib/error.svelte.js";

  let errorInfo = $state(null);

  onMount(() => {
    // Check for stored errors periodically (store updates won't trigger reactivity here)
    const checkError = () => {
      const err = getError();
      if (err && err.timestamp !== errorInfo?.timestamp) {
        errorInfo = err;
      }
    };

    // Global error handler for uncaught errors
    const handleError = (event) => {
      const err = event.error || new Error(String(event.error));
      captureError(err, event.loc);
      errorInfo = {
        message: err instanceof Error ? err.message : String(err),
        stack: event.loc,
        timestamp: Date.now()
      };
      showToast(`Error: ${err.message || "Unknown error"}`, "error", 5000);
      event.preventDefault();
    };

    // Global handler for unhandled promise rejections
    const handleRejection = (event) => {
      const err = event.reason || new Error("Unhandled promise rejection");
      captureError(err, null);
      errorInfo = {
        message: err instanceof Error ? err.message : String(err),
        stack: null,
        timestamp: Date.now()
      };
      showToast(`Error: ${err.message || "Unknown error"}`, "error", 5000);
      event.preventDefault();
    };

    window.addEventListener("error", handleError);
    window.addEventListener("unhandledrejection", handleRejection);

    // Check for errors periodically
    const interval = setInterval(checkError, 500);

    return () => {
      window.removeEventListener("error", handleError);
      window.removeEventListener("unhandledrejection", handleRejection);
      clearInterval(interval);
    };
  });

  function handleClearError() {
    errorInfo = null;
    clearStoredError();
  }
</script>

{#if errorInfo}
  <div class="fixed inset-0 z-[100] flex items-center justify-center bg-nx-bg/80 backdrop-blur-sm">
    <div class="mx-4 max-w-md rounded border border-nx-danger/50 bg-nx-surface p-6 shadow-xl">
      <div class="mb-4 flex items-center gap-3">
        <span class="material-symbols-outlined text-3xl text-nx-danger">error</span>
        <h2 class="text-lg font-semibold text-nx-text">{t("error.title") || "Something went wrong"}</h2>
      </div>
      
      <p class="mb-4 text-sm text-nx-text-secondary">
        {errorInfo.message}
      </p>
      
      {#if errorInfo.stack}
        <details class="mb-4">
          <summary class="cursor-pointer text-xs text-nx-text-muted hover:text-nx-text-secondary">
            {t("error.details") || "Component details"}
          </summary>
          <pre class="mt-2 max-h-32 overflow-auto rounded bg-nx-bg p-2 text-xs text-nx-text-muted">{errorInfo.stack}</pre>
        </details>
      {/if}
      
      <div class="flex gap-3">
        <button
          class="flex-1 rounded border border-nx-border bg-nx-bg px-4 py-2 text-sm font-medium text-nx-text-secondary transition-colors hover:bg-nx-raised"
          onclick={() => window.location.reload()}
        >
          {t("error.reload") || "Reload page"}
        </button>
        <button
          class="flex-1 rounded bg-nx-accent px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-nx-accent/90"
          onclick={handleClearError}
        >
          {t("error.dismiss") || "Dismiss"}
        </button>
      </div>
    </div>
  </div>
{/if}