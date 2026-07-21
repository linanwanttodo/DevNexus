<script>
  import { getConfirmations, confirmResponse } from "../lib/confirm.svelte.js";
  import { t } from "../lib/i18n.svelte.js";

  let confirmations = $derived(getConfirmations());
</script>

{#if confirmations.length > 0}
  <div class="nx-dialog-overlay">
    {#each confirmations as c (c.id)}
      <div class="nx-dialog mx-4 w-full max-w-xs nx-animate-scale-in">
        <div class="px-3 py-3">
          <p class="text-sm text-nx-text-secondary">{c.message}</p>
        </div>
        <div class="flex justify-end gap-2 px-3 py-2 border-t border-nx-border">
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
