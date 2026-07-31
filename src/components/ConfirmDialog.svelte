<script>
  import { getConfirmations, confirmResponse } from "../lib/confirm.svelte.js";
  import { t } from "../lib/i18n.svelte.js";

  let confirmations = $derived(getConfirmations());
</script>

{#if confirmations.length > 0}
  <div class="nx-dialog-overlay">
    {#each confirmations as c (c.id)}
      <div class="nx-dialog mx-4 nx-animate-scale-in" style="width: max-content; min-width: 200px; max-width: min(300px, 85vw);">
        <div class="px-4 py-4">
          {#if c.title}
            <h3 class="text-sm font-semibold text-nx-text mb-1.5">{c.title}</h3>
          {/if}
          <p class="text-sm text-nx-text-secondary leading-relaxed">{c.message}</p>
        </div>
        <div class="flex justify-end gap-2 px-4 py-3 border-t border-nx-border">
          <button
            class="nx-btn h-7 text-xs"
            onclick={() => confirmResponse(c.id, false)}
          >
            {t('common.cancel')}
          </button>
          <button
            class="nx-btn nx-btn-danger h-7 text-xs"
            onclick={() => confirmResponse(c.id, true)}
          >
            {t('common.confirm')}
          </button>
        </div>
      </div>
    {/each}
  </div>
{/if}
