<script>
  import { t } from "../../lib/i18n.svelte.js";

  let {
    models = [],
    selected = {},
    aliases = {},
    contexts = {},
    onToggle,
    showCtx = true,
    maxH = "max-h-60",
    extraClass = "",
  } = $props();
</script>

<div class="{maxH} overflow-y-auto rounded-md border border-nx-border bg-nx-bg/50 {extraClass}">
  {#each models as m}
    <div
      class="flex items-center gap-3 px-3 py-2 border-b border-nx-border last:border-0 hover:bg-nx-hover transition-colors cursor-pointer"
      role="option"
      aria-selected={selected[m.id]}
      onclick={() => onToggle(m.id)}
      onkeydown={(e) => e.key === 'Enter' && onToggle(m.id)}
      tabindex="0"
    >
      <!-- Checkbox -->
      <div class="w-5 flex justify-center shrink-0">
        {#if selected[m.id]}
          <div class="w-4 h-4 rounded-sm bg-nx-accent flex items-center justify-center">
            <span class="material-symbols-outlined text-white text-[11px]">check</span>
          </div>
        {:else}
          <div class="w-4 h-4 rounded-sm border border-nx-border-light"></div>
        {/if}
      </div>
      <!-- Model info -->
      {#if showCtx}
        <div class="flex-1 min-w-0">
          <div class="text-xs font-mono text-nx-text truncate">{m.id}</div>
          {#if m.id !== m.name}
            <div class="text-[10px] text-nx-text-muted">{m.name}</div>
          {/if}
        </div>
      {:else}
        <div class="flex-1 text-xs font-mono text-nx-text truncate">{m.id}</div>
      {/if}
      <!-- Alias input + Context length -->
      {#if selected[m.id]}
        <input
          type="text"
          class="{showCtx ? 'w-24' : 'w-32'} text-right nx-input py-0.5 text-[11px]"
          bind:value={aliases[m.id]}
          placeholder={t("apiHub.alias")}
          onclick={(e) => e.stopPropagation()}
          onkeydown={(e) => e.stopPropagation()}
        />
        {#if showCtx}
          <input
            type="number"
            class="w-20 text-right nx-input py-0.5 text-[11px]"
            bind:value={contexts[m.id]}
            placeholder="200000"
            onclick={(e) => e.stopPropagation()}
            onkeydown={(e) => e.stopPropagation()}
          />
          <span class="text-[10px] text-nx-text-muted shrink-0">ctx</span>
        {/if}
      {/if}
      {#if showCtx && m.owned_by}
        <div class="text-[10px] text-nx-text-muted shrink-0">{m.owned_by}</div>
      {/if}
    </div>
  {/each}
</div>
