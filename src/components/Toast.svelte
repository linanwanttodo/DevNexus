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

  function typeColor(type) {
    switch (type) {
      case "success": return "var(--nx-success)";
      case "error": return "var(--nx-danger)";
      case "warning": return "var(--nx-warning)";
      default: return "var(--nx-text-secondary)";
    }
  }
</script>

<div class="fixed top-3 right-3 z-50 flex flex-col gap-2 pointer-events-none">
  {#each toasts as toast (toast.id)}
    <div
      class="pointer-events-auto flex items-center gap-3 px-4 py-3 rounded-lg shadow-lg nx-animate-slide-up"
      style="background: var(--nx-raised); min-width: 280px; max-width: 400px;"
    >
      <span class="material-symbols-outlined text-sm flex-shrink-0" style="color: {typeColor(toast.type)}">
        {typeIcon(toast.type)}
      </span>
      <p class="flex-1 text-sm text-nx-text leading-relaxed">{toast.message}</p>
      <button
        class="flex-shrink-0 text-nx-text-muted hover:text-nx-text transition-colors cursor-pointer bg-none border-none"
        onclick={() => removeToast(toast.id)}
      >
        <span class="material-symbols-outlined text-sm">close</span>
      </button>
    </div>
  {/each}
</div>
