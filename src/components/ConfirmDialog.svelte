<script>
  import { getConfirmations, confirmResponse } from "../lib/confirm.svelte.js";
  import { t } from "../lib/i18n.svelte.js";

  let confirmations = $derived(getConfirmations());
</script>

{#if confirmations.length > 0}
  <div class="nx-dialog-overlay">
    {#each confirmations as c (c.id)}
      <div class="nx-dialog mx-4 w-full max-w-md nx-animate-scale-in">
        <div class="px-6 pt-5 pb-4">
          <h3 class="text-base font-semibold text-nx-text">{c.title}</h3>
          <p class="mt-2 text-sm text-nx-text-secondary leading-relaxed">{c.message}</p>
        </div>
        <div class="flex justify-end gap-3 px-6 py-4 border-t border-nx-border">
          <button
            class="nx-btn h-8 text-xs"
            onclick={() => confirmResponse(c.id, false)}
          >
            {t('common.cancel')}
          </button>
          <button
            class="nx-btn nx-btn-danger h-8 text-xs"
            onclick={() => confirmResponse(c.id, true)}
          >
            {t('common.confirm')}
          </button>
        </div>
      </div>
    {/each}
  </div>
{/if}
