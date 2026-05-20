<script>
  import { getConfirmations, confirmResponse, onConfirmChange } from "../lib/confirm.js";
  import { t, getVersion, onLangChange } from "../lib/i18n.js";

  let confirmations = $state(getConfirmations());
  let _v = $state(getVersion());

  $effect(() => {
    return onLangChange((v) => { _v = v; });
  });

  $effect(() => {
    return onConfirmChange((c) => {
      confirmations = c;
    });
  });
</script>

{#if confirmations.length > 0}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
    {#each confirmations as c (c.id)}
      <div class="mx-4 w-full max-w-md rounded border border-nx-border bg-nx-surface shadow-2xl">
        <div class="px-6 pt-5 pb-4">
          <h3 class="text-lg font-semibold text-nx-text">{c.title}</h3>
          <p class="mt-2 text-sm text-nx-text/70">{c.message}</p>
        </div>
        <div class="flex justify-end gap-3 px-6 py-4 border-t border-nx-border">
          <button
            class="px-4 py-2 text-sm font-medium text-nx-text-secondary hover:bg-nx-raised transition-colors"
            onclick={() => confirmResponse(c.id, false)}
          >
            {_v && t('common.cancel')}
          </button>
          <button
            class="px-4 py-2 text-sm font-medium border border-nx-danger text-nx-danger hover:bg-nx-danger/10 transition-colors"
            onclick={() => confirmResponse(c.id, true)}
          >
            {_v && t('common.confirm')}
          </button>
        </div>
      </div>
    {/each}
  </div>
{/if}
