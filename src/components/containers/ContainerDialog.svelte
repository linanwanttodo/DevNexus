<script>
  import { t } from "../../lib/i18n.svelte.js";

  // config: {
  //   title, icon, width,
  //   fields: [{ id, type, placeholder, value, onInput, enterSubmit }],
  //   loading, submitLabel, loadingLabel, canSubmit,
  //   onSubmit, onClose,
  // }
  let { config } = $props();
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
  <div class="w-full {config.width} bg-nx-surface border border-nx-border rounded-lg shadow-xl overflow-hidden">
    <div class="flex items-center justify-between px-3 py-2 border-b border-nx-border">
      <h2 class="text-sm font-semibold text-nx-text flex items-center gap-1.5">
        {#if config.icon}<span class="material-symbols-outlined text-lg">{config.icon}</span>{/if}
        {config.title}
      </h2>
      <button class="text-nx-text-muted hover:text-nx-text" onclick={config.onClose}>
        <span class="material-symbols-outlined text-lg">close</span>
      </button>
    </div>
    <div class="p-3 {config.fields.length > 1 ? 'space-y-2' : ''}">
      {#each config.fields as f}
        <input
          id={f.id}
          type={f.type || "text"}
          value={f.value}
          oninput={(e) => f.onInput(e.currentTarget.value)}
          placeholder={f.placeholder}
          class="nx-input w-full h-8 text-xs"
          onkeydown={(e) => { if (e.key === 'Enter' && f.enterSubmit !== false) config.onSubmit(); }}
        />
      {/each}
      <div class="mt-3 flex justify-end gap-2">
        <button class="nx-btn h-7 text-xs" onclick={config.onClose}>{t("common.cancel")}</button>
        <button class="nx-btn nx-btn-primary h-7 text-xs" onclick={config.onSubmit} disabled={config.loading || !config.canSubmit}>
          {config.loading ? config.loadingLabel : config.submitLabel}
        </button>
      </div>
    </div>
  </div>
</div>
