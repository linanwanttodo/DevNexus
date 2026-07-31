<script>
  import { t } from "../lib/i18n.svelte.js";

  // mode "form": 渲染字段分组表单（add/edit）
  // mode "view": 渲染密码查看详情
  // groups: 字段分组（每组为一行；单字段组整行，多字段组 2 列网格）
  // 每组字段: { id, labelKey, required, type, placeholder, textarea, value, onInput }
  let { title, mode = "form", groups = [], submitLabel = "", onSubmit = () => {}, onClose = () => {}, password = "", onCopy = () => {} } = $props();
</script>

{#snippet field(f)}
  <div>
    <label class="mb-0.5 block text-xs text-nx-text-muted" for={f.id}>{t(f.labelKey)}{f.required ? ' *' : ''}</label>
    {#if f.textarea}
      <textarea id={f.id} value={f.value} oninput={(e) => f.onInput(e.currentTarget.value)} placeholder={f.placeholder} rows="2" class="nx-input w-full text-xs"></textarea>
    {:else}
      <input id={f.id} type={f.type || "text"} value={f.value} oninput={(e) => f.onInput(e.currentTarget.value)} placeholder={f.placeholder} class="nx-input h-8 w-full text-xs" />
    {/if}
  </div>
{/snippet}

<div class="nx-dialog-overlay" role="button" tabindex="0" onkeydown={(e) => e.key === 'Escape' && onClose()} onclick={() => onClose()}>
  <div class="nx-dialog" role="dialog" aria-modal="true" tabindex="-1" onkeydown={(e) => e.stopPropagation()} onclick={(e) => e.stopPropagation()}>
    <div class="nx-dialog-header">
      <h2 class="text-lg font-semibold text-nx-text">{title}</h2>
    </div>

    {#if mode === "view"}
      <div class="nx-dialog-body">
        <div class="nx-card p-4">
          <div class="mb-2 text-xs text-nx-text-muted">{t('passwords.password')}</div>
          <div class="flex items-center gap-2">
            <code class="flex-1 break-all text-sm text-nx-text">{password}</code>
            <button
              class="p-1.5 text-nx-text-secondary"
              onclick={() => onCopy()}
              title={t('passwords.title_copy')}>
              <span class="material-symbols-outlined text-lg">content_copy</span>
            </button>
          </div>
        </div>
      </div>
      <div class="nx-dialog-footer">
        <button class="nx-btn nx-btn-primary" onclick={() => onClose()}>{t('passwords.close')}</button>
      </div>
    {:else}
      <div class="nx-dialog-body space-y-2.5">
        {#each groups as group}
          <div class={group.length > 1 ? "grid grid-cols-2 gap-3" : ""}>
            {#each group as f}{@render field(f)}{/each}
          </div>
        {/each}
      </div>
      <div class="nx-dialog-footer">
        <button class="nx-btn nx-btn-ghost" onclick={() => onClose()}>{t('passwords.cancel')}</button>
        <button class="nx-btn nx-btn-primary" onclick={() => onSubmit()}>{submitLabel}</button>
      </div>
    {/if}
  </div>
</div>
