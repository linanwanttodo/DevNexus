<script>
  import { getToasts, removeToast } from "../lib/toast.svelte.js";

  let toasts = $derived(getToasts());

  function typeIcon(type) {
    switch (type) {
      case "success": return "check_circle";
      case "error": return "error";
      case "warning": return "warning";
      default: return "info";
    }
  }

  function borderClass(type) {
    switch (type) {
      case "success": return "border-nx-success";
      case "error": return "border-nx-danger";
      case "warning": return "border-nx-warning";
      default: return "border-nx-info";
    }
  }

  function iconClass(type) {
    switch (type) {
      case "success": return "text-nx-success";
      case "error": return "text-nx-danger";
      case "warning": return "text-nx-warning";
      default: return "text-nx-info";
    }
  }
</script>

<div class="fixed top-4 right-4 z-50 flex flex-col gap-2">
  {#each toasts as toast (toast.id)}
    <div
      class="flex items-start gap-3 rounded border-2 bg-nx-surface px-4 py-3 shadow-lg max-w-sm {borderClass(toast.type)}"
    >
      <span class="material-symbols-outlined mt-0.5 text-sm font-bold {iconClass(toast.type)}">{typeIcon(toast.type)}</span>
      <p class="flex-1 text-sm text-nx-text">{toast.message}</p>
      <button
        class="text-nx-text/50 hover:text-nx-text transition-colors"
        onclick={() => removeToast(toast.id)}
      >
        <span class="material-symbols-outlined text-sm">close</span>
      </button>
    </div>
  {/each}
</div>
