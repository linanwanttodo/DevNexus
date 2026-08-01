<script>
  import { t } from "../../lib/i18n.svelte.js";
  import ContainerIcons from "../../icons/ContainerIcons.svelte";

  let {
    items = [],
    loading = false,
    error = null,
    actionLoading = "",
    onCreate,
    onRefresh,
    onRemove,
  } = $props();

  function formatCreated(created) { return created || "-"; }
</script>

<div>
  <div class="mb-4 flex items-center justify-between">
    <div class="flex items-center gap-2">
      <button class="nx-btn text-xs h-7" onclick={onCreate}>
        <span class="material-symbols-outlined text-sm">add</span>{t("docker.create")}
      </button>
    </div>
    <button class="nx-btn nx-btn-ghost px-2 text-xs h-7" onclick={onRefresh} disabled={loading}>
      <span class="material-symbols-outlined text-sm {loading ? 'animate-spin' : ''}">refresh</span>
      {t("common.refresh")}
    </button>
  </div>

  <div class="nx-section">
    {#if loading && items.length === 0}
      <div class="flex items-center justify-center py-12">
        <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
      </div>
    {:else if error}
      <div class="p-6 text-center">
        <span class="material-symbols-outlined text-nx-danger text-3xl">error</span>
        <div class="mt-2 text-sm text-nx-danger">{error}</div>
        <button class="nx-btn nx-btn-primary mt-4" onclick={onRefresh}>{t("common.retry")}</button>
      </div>
    {:else if items.length === 0}
      <div class="p-12 text-center">
        <ContainerIcons name="volume" size={36} class="mx-auto text-nx-text-muted" />
        <div class="mt-3 text-sm text-nx-text-muted">{t("docker.no_volumes")}</div>
      </div>
    {:else}
      <table class="nx-table">
        <thead>
          <tr>
            <th>{t("docker.name")}</th>
            <th>{t("docker.driver")}</th>
            <th>{t("docker.mountpoint")}</th>
            <th>{t("docker.created")}</th>
            <th class="text-right w-24">{t("docker.actions")}</th>
          </tr>
        </thead>
        <tbody>
          {#each items as v (v.name)}
            <tr>
              <td class="text-sm font-medium text-nx-text">{v.name}</td>
              <td class="text-xs text-nx-text-secondary">{v.driver}</td>
              <td class="font-mono text-xs text-nx-text-muted max-w-[280px] truncate" title={v.mountpoint}>{v.mountpoint}</td>
              <td class="text-xs text-nx-text-muted">{formatCreated(v.created)}</td>
              <td class="text-right">
                <button class="nx-btn text-xs h-7 px-2 text-nx-danger"
                  onclick={() => onRemove(v.name)} disabled={actionLoading === v.name}>
                  {t("docker.delete")}
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>
