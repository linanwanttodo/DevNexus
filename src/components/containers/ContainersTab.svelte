<script>
  import { t } from "../../lib/i18n.svelte.js";
  import ContainerIcons from "../../icons/ContainerIcons.svelte";

  let {
    items = [],
    loading = false,
    error = null,
    search = "",
    showAll = false,
    actionLoading = "",
    onShowAllChange,
    onRefresh,
    onAction,
    onLogs,
    onTerminal,
  } = $props();

  function shortId(id) { return id ? id.substring(0, 12) : ""; }
  function formatCreated(created) { return created || "-"; }
</script>

<div>
  <div class="mb-4 flex items-center justify-between">
    <div class="flex items-center gap-3">
      <label class="flex items-center gap-1.5 text-xs text-nx-text-muted cursor-pointer select-none">
        <input type="checkbox" checked={showAll} onchange={(e) => onShowAllChange(e.currentTarget.checked)} class="rounded border-nx-border bg-nx-bg" />
        {t("docker.show_all")}
      </label>
    </div>
    <button class="nx-btn nx-btn-ghost px-2 py-1 text-xs" onclick={onRefresh} disabled={loading}>
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
        <ContainerIcons name="container" size={36} class="mx-auto text-nx-text-muted" />
        <div class="mt-3 text-sm text-nx-text-muted">{search ? t("docker.no_matching") : t("docker.no_containers")}</div>
      </div>
    {:else}
      <table class="nx-table">
        <thead>
          <tr>
            <th class="w-4"></th>
            <th>{t("docker.name")}</th>
            <th>{t("docker.image")}</th>
            <th>{t("docker.ports")}</th>
            <th class="w-28">{t("docker.created")}</th>
            <th class="text-right w-80">{t("docker.actions")}</th>
          </tr>
        </thead>
        <tbody>
          {#each items as c (c.id)}
            <tr>
              <td class="!pr-0">
                {#if c.status === "running"}
                  <ContainerIcons name="container-running" size={16} />
                {:else if c.status === "paused"}
                  <ContainerIcons name="container-paused" size={16} />
                {:else}
                  <ContainerIcons name="container-exited" size={16} />
                {/if}
              </td>
              <td>
                <div class="flex flex-col">
                  <span class="text-sm font-medium text-nx-text">{c.name}</span>
                  <span class="font-mono text-xs text-nx-text-muted">{shortId(c.id)}</span>
                </div>
              </td>
              <td class="font-mono text-xs text-nx-text-secondary">{c.image}</td>
              <td class="font-mono text-xs text-nx-text-muted max-w-[180px] truncate">{c.ports || "-"}</td>
              <td class="text-xs text-nx-text-muted">{formatCreated(c.created)}</td>
              <td class="text-right">
                <span class="flex items-center justify-end gap-1">
                  {#if c.status === "running"}
                    <button class="nx-btn text-xs h-7 px-2 text-nx-warning" onclick={() => onAction(c.name, "pause")} disabled={actionLoading === c.name}>{t("docker.pause")}</button>
                    <button class="nx-btn text-xs h-7 px-2 text-nx-danger" onclick={() => onAction(c.name, "stop")} disabled={actionLoading === c.name}>{t("docker.stop")}</button>
                  {:else if c.status === "paused"}
                    <button class="nx-btn text-xs h-7 px-2" onclick={() => onAction(c.name, "unpause")} disabled={actionLoading === c.name}>{t("docker.unpause")}</button>
                  {:else}
                    <button class="nx-btn text-xs h-7 px-2 text-nx-success" onclick={() => onAction(c.name, "start")} disabled={actionLoading === c.name}>{t("docker.start")}</button>
                  {/if}
                  <button class="nx-btn text-xs h-7 px-2" onclick={() => onAction(c.name, "restart")} disabled={actionLoading === c.name}>{t("docker.restart")}</button>
                  <button class="nx-btn text-xs h-7 px-2" onclick={() => onLogs(c.name)}><span class="material-symbols-outlined text-sm">list_alt</span></button>
                  <button class="nx-btn text-xs h-7 px-2" onclick={() => onTerminal(c.name)}><span class="material-symbols-outlined text-sm">terminal</span></button>
                  <button class="nx-btn text-xs h-7 px-2 text-nx-danger" onclick={() => onAction(c.name, "rm")} disabled={actionLoading === c.name}>{t("docker.delete")}</button>
                </span>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      <div class="flex items-center justify-between border-t border-nx-border px-4 py-2">
        <span class="text-xs text-nx-text-muted">{items.length} {t("docker.containers_count")}</span>
      </div>
    {/if}
  </div>
</div>
