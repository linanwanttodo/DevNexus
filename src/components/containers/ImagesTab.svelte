<script>
  import { t } from "../../lib/i18n.svelte.js";
  import ContainerIcons from "../../icons/ContainerIcons.svelte";

  let {
    items = [],
    loading = false,
    error = null,
    search = "",
    actionLoading = "",
    onPull,
    onBuild,
    onRefresh,
    onPush,
    onTag,
    onRemove,
  } = $props();

  function shortId(id) { return id ? id.substring(0, 12) : ""; }
  function formatCreated(created) { return created || "-"; }
  function formatSize(size) { return size || "-"; }
</script>

<div>
  <div class="mb-4 flex items-center justify-between">
    <div class="flex items-center gap-2">
      <button class="nx-btn text-xs h-7" onclick={onPull}>
        <span class="material-symbols-outlined text-sm">download</span>{t("docker.pull")}
      </button>
      <button class="nx-btn text-xs h-7" onclick={onBuild}>
        <span class="material-symbols-outlined text-sm">construction</span>{t("docker.build")}
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
        <ContainerIcons name="image" size={36} class="mx-auto text-nx-text-muted" />
        <div class="mt-3 text-sm text-nx-text-muted">{search ? t("docker.no_matching") : t("docker.no_images")}</div>
      </div>
    {:else}
      <table class="nx-table">
        <thead>
          <tr>
            <th>{t("docker.repository")}</th>
            <th>{t("docker.tag")}</th>
            <th>{t("docker.image_id")}</th>
            <th>{t("docker.created")}</th>
            <th class="text-right">{t("docker.size")}</th>
            <th class="text-right w-24">{t("docker.actions")}</th>
          </tr>
        </thead>
        <tbody>
          {#each items as img (img.id)}
            <tr>
              <td class="text-sm font-medium text-nx-text">{img.repository}</td>
              <td><span class="nx-pill font-mono text-[10px]">{img.tag}</span></td>
              <td class="font-mono text-xs text-nx-text-muted">{shortId(img.id)}</td>
              <td class="text-xs text-nx-text-muted">{formatCreated(img.created)}</td>
              <td class="text-right text-xs text-nx-text-secondary">{formatSize(img.size)}</td>
              <td class="text-right">
                <span class="flex items-center justify-end gap-1">
                  <button class="nx-btn text-xs h-7 px-2" onclick={() => onPush(img)}>{t("docker.push")}</button>
                  <button class="nx-btn text-xs h-7 px-2" onclick={() => onTag(img)}>{t("docker.tag")}</button>
                  <button class="nx-btn text-xs h-7 px-2 text-nx-danger"
                    onclick={() => onRemove(img.id, `${img.repository}:${img.tag}`)} disabled={actionLoading === img.id}>
                    {t("docker.delete")}
                  </button>
                </span>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      <div class="flex items-center justify-between border-t border-nx-border px-4 py-2">
        <span class="text-xs text-nx-text-muted">{items.length} {t("docker.images_count")}</span>
      </div>
    {/if}
  </div>
</div>
