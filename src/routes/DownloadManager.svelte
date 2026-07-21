<script>
  import { onMount, onDestroy } from "svelte";
  import { navigate } from "../lib/stores.svelte.js";
  import { t, tFormat } from "../lib/i18n.svelte.js";
  import {
    loadDownloads, loadConfig, saveConfig,
    addDownload, pauseDownload, resumeDownload,
    cancelDownload, deleteDownload,
    startMonitor, stopMonitor,
    getDownloads, getConfig,
    formatBytes, formatSpeed, formatETA, getStatusInfo,
    getGithubMirrors, saveGithubMirrors, readClipboard, isGithubUrl
  } from "../lib/downloads.svelte.js";
  import { showToast } from "../lib/toast.svelte.js";
  import { showConfirm } from "../lib/confirm.svelte.js";

  let downloadList = $state([]);
  let config = $state(null);
  let loading = $state(true);
  let showAddDialog = $state(false);
  let showConfigDialog = $state(false);
  let showMirrorDialog = $state(false);
  let mirrors = $state([]);
  let newUrl = $state("");
  let newSavePath = $state("");
  let filterStatus = $state("all");
  let searchQuery = $state("");
  let pollInterval = $state(null);

  let stats = $derived.by(() => {
    const total = downloadList.length;
    const active = downloadList.filter(d => d.status === "Downloading").length;
    const completed = downloadList.filter(d => d.status === "Completed").length;
    const failed = downloadList.filter(d => d.status === "Failed").length;
    const totalSpeed = downloadList.reduce((sum, d) => sum + (d.speed || 0), 0);
    return { total, active, completed, failed, totalSpeed };
  });

  let filteredDownloads = $derived.by(() => {
    return downloadList.filter(d => {
      if (filterStatus !== "all" && d.status !== filterStatus) return false;
      if (searchQuery) {
        const q = searchQuery.toLowerCase();
        return d.filename.toLowerCase().includes(q) ||
               d.url.toLowerCase().includes(q);
      }
      return true;
    }).map(d => ({ ...d, statusInfo: getStatusInfo(d.status) }));
  });

  onMount(async () => {
    await Promise.all([loadDownloads(), loadConfig()]);
    downloadList = getDownloads();
    config = getConfig();
    startMonitor();
    loading = false;
    pollInterval = setInterval(async () => {
      await loadDownloads();
      downloadList = getDownloads();
    }, 1000);
  });

  onDestroy(() => {
    if (pollInterval) clearInterval(pollInterval);
    stopMonitor();
  });

  async function handleAddDownload() {
    if (!newUrl.trim()) return;
    try {
      await addDownload(newUrl.trim(), newSavePath.trim() || null);
      showAddDialog = false;
      newUrl = "";
      newSavePath = "";
      showToast(t("downloads.add_success"));
    } catch {}
  }

  async function handlePause(taskId) { await pauseDownload(taskId); }
  async function handleResume(taskId) { await resumeDownload(taskId); }
  async function handleCancel(taskId) { await cancelDownload(taskId); }

  async function handleDeleteWithFile(taskId) {
    const confirmed = await showConfirm(tFormat("downloads.delete_with_file_confirm", { name: taskId.slice(0, 8) + "..." }));
    if (!confirmed) return;
    await deleteDownload(taskId, true);
  }

  async function openAddDialog() {
    newUrl = await readClipboard();
    showAddDialog = true;
  }

  async function handleSaveConfig() {
    try {
      await saveConfig(config);
      showConfigDialog = false;
      showToast(t("downloads.config_saved"));
    } catch {
      showToast(t("downloads.config_save_failed"));
    }
  }

  async function openMirrorDialog() {
    mirrors = await getGithubMirrors();
    showMirrorDialog = true;
  }

  function addMirror() {
    mirrors = [...mirrors, { name: "", url_prefix: "https://", enabled: true }];
  }

  function removeMirror(i) {
    mirrors = mirrors.filter((_, idx) => idx !== i);
  }

  async function handleSaveMirrors() {
    await saveGithubMirrors(mirrors);
    showMirrorDialog = false;
    showToast(t("downloads.mirror_saved"));
  }

  function openFile(task) {
    showToast(t("downloads.opening").replace("{name}", task.filename));
  }
</script>

<div class="flex h-full flex-col">
  <!-- Header -->
  <div class="flex items-center gap-2 border-b border-nx-border px-5 py-2.5">
    <button class="nx-back-btn" onclick={() => navigate("/dashboard")}>
      <span class="material-symbols-outlined text-lg">arrow_back</span>
      {t("nav.dashboard")}
    </button>
    <span class="text-xs text-nx-text-muted">/</span>
    <h1 class="text-sm font-medium text-nx-text">{t("downloads.title")}</h1>
  </div>

  <!-- Toolbar -->
  <div class="flex items-center justify-between border-b border-nx-border px-5 py-2.5 gap-3">
    <div class="flex items-center gap-1 flex-wrap">
      {#each ["all", "Downloading", "Paused", "Completed", "Failed"] as status}
        <button
          class="px-3 py-1.5 text-xs font-medium rounded-md transition-colors
            {filterStatus === status
              ? 'bg-nx-accent-bg text-nx-accent'
              : 'text-nx-text-secondary hover:text-nx-text hover:bg-nx-hover'}"
          onclick={() => filterStatus = status}>
          {status === "all" ? t("downloads.all") : t(`downloads.status_${status.toLowerCase()}`)}
        </button>
      {/each}
    </div>
    <div class="flex items-center gap-2">
      <input
        type="text"
        bind:value={searchQuery}
        placeholder={t("downloads.search_placeholder")}
        class="nx-input w-40 text-xs px-2.5 py-1.5"
      />
      <button class="nx-btn nx-btn-ghost px-2 py-1 text-xs" onclick={openAddDialog}>
        <span class="material-symbols-outlined text-sm">add</span>
        {t("downloads.add")}
      </button>
      <button class="nx-btn nx-btn-ghost px-2 py-1 text-xs" onclick={() => showConfigDialog = true}>
        <span class="material-symbols-outlined text-sm">settings</span>
      </button>
      <button class="nx-btn nx-btn-ghost px-2 py-1 text-xs" onclick={openMirrorDialog}>
        <span class="material-symbols-outlined text-sm">cloud_sync</span>
      </button>
    </div>
  </div>

  <!-- Stats bar -->
  <div class="flex items-center gap-4 border-b border-nx-border px-5 py-2 text-xs text-nx-text-muted">
    <span>{t("downloads.total")}: {stats.total}</span>
    <span>{t("downloads.active")}: <span class="text-nx-accent">{stats.active}</span></span>
    <span>{t("downloads.completed")}: <span class="text-nx-success">{stats.completed}</span></span>
    <span>{t("downloads.failed")}: <span class="text-nx-error">{stats.failed}</span></span>
    <span>{t("downloads.speed")}: <span class="text-nx-accent">{formatSpeed(stats.totalSpeed)}</span></span>
  </div>

  <!-- Content -->
  <div class="flex-1 overflow-y-auto p-5">
    {#if loading}
      <div class="flex items-center justify-center py-16">
        <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
      </div>
    {:else if filteredDownloads.length === 0}
      <div class="nx-card p-8 text-center">
        <span class="material-symbols-outlined text-nx-text-muted text-4xl">download</span>
        <h2 class="mt-4 text-base font-semibold text-nx-text">{t("downloads.no_downloads")}</h2>
        <p class="mt-2 text-sm text-nx-text-secondary">{t("downloads.no_downloads_desc")}</p>
        <button class="mt-4 nx-btn nx-btn-primary text-xs px-4 py-2" onclick={() => showAddDialog = true}>
          {t("downloads.add")}
        </button>
      </div>
    {:else}
      <div class="space-y-3">
        {#each filteredDownloads as task (task.id)}
          <div class="nx-card p-4">
            <div class="flex items-start gap-3">
              <div class="flex-shrink-0 mt-0.5">
                <span class="material-symbols-outlined text-lg {task.statusInfo.color}">{task.statusInfo.icon}</span>
              </div>
              <div class="flex-1 min-w-0">
                <div class="flex items-center justify-between gap-2 mb-1">
                  <h3 class="text-sm font-medium text-nx-text truncate" title={task.filename}>{task.filename}</h3>
                  <div class="flex items-center gap-1 flex-shrink-0">
                    {#if task.status === "Downloading"}
                      <button class="nx-btn nx-btn-ghost px-2 py-1 text-xs" onclick={() => handlePause(task.id)}>
                        <span class="material-symbols-outlined text-sm">pause</span>
                      </button>
                      <button class="nx-btn nx-btn-ghost px-2 py-1 text-xs" onclick={() => handleCancel(task.id)}>
                        <span class="material-symbols-outlined text-sm">stop</span>
                      </button>
                    {:else if task.status === "Paused"}
                      <button class="nx-btn nx-btn-ghost px-2 py-1 text-xs" onclick={() => handleResume(task.id)}>
                        <span class="material-symbols-outlined text-sm">play_arrow</span>
                      </button>
                      <button class="nx-btn nx-btn-ghost px-2 py-1 text-xs" onclick={() => handleCancel(task.id)}>
                        <span class="material-symbols-outlined text-sm">stop</span>
                      </button>
                    {:else if task.status === "Failed"}
                      <button class="nx-btn nx-btn-ghost px-2 py-1 text-xs" onclick={() => handleResume(task.id)}>
                        <span class="material-symbols-outlined text-sm">refresh</span>
                      </button>
                    {:else if task.status === "Completed"}
                      <button class="nx-btn nx-btn-ghost px-2 py-1 text-xs" onclick={() => openFile(task)}>
                        <span class="material-symbols-outlined text-sm">folder_open</span>
                      </button>
                    {/if}
                    <button class="nx-btn nx-btn-ghost px-2 py-1 text-xs text-nx-error" onclick={() => handleDeleteWithFile(task.id)}>
                      <span class="material-symbols-outlined text-sm">delete</span>
                    </button>
                  </div>
                </div>
                <p class="text-xs text-nx-text-muted truncate mb-2" title={task.url}>{task.url}</p>

                <!-- Segmented progress bar (IDM-style) -->
                {#if task.total_size > 0 && task.chunks?.length}
                  <div class="mb-2">
                    <div class="flex items-center justify-between text-xs text-nx-text-secondary mb-1">
                      <span>{formatBytes(task.downloaded_size)} / {formatBytes(task.total_size)}</span>
                      <span>{((task.downloaded_size / task.total_size) * 100).toFixed(1)}%</span>
                    </div>
                    <div class="h-2 bg-nx-border rounded-sm overflow-hidden flex gap-px">
                      {#each task.chunks as chunk}
                        {@const chunkSize = chunk.end - chunk.start + 1}
                        {@const pct = (chunkSize / task.total_size) * 100}
                        <div
                          class="h-full transition-colors duration-300 {chunk.status === 'Completed' ? 'bg-nx-success' : chunk.status === 'Downloading' ? 'bg-nx-accent' : chunk.status === 'Failed' ? 'bg-nx-error' : 'bg-nx-text-muted/20'}"
                          style="width: {pct}%"
                          title="[{chunk.status}] {formatBytes(chunk.downloaded)}/{formatBytes(chunkSize)}"
                        ></div>
                      {/each}
                    </div>
                  </div>
                {:else if task.total_size > 0}
                  <div class="mb-2">
                    <div class="flex items-center justify-between text-xs text-nx-text-secondary mb-1">
                      <span>{formatBytes(task.downloaded_size)} / {formatBytes(task.total_size)}</span>
                      <span>{((task.downloaded_size / task.total_size) * 100).toFixed(1)}%</span>
                    </div>
                    <div class="h-2 bg-nx-border rounded-sm overflow-hidden">
                      <div class="h-full rounded-sm transition-all duration-300"
                        style="width: {(task.downloaded_size / task.total_size) * 100}%; background: var(--nx-accent);"></div>
                    </div>
                  </div>
                {/if}

                <!-- Stats -->
                <div class="flex items-center gap-4 text-xs text-nx-text-muted">
                  <span>{t("downloads.speed")}: <span class="text-nx-text-secondary">{formatSpeed(task.speed)}</span></span>
                  <span>{t("downloads.eta")}: <span class="text-nx-text-secondary">{formatETA(task.eta_seconds)}</span></span>
                  {#if task.error}
                    <span class="text-nx-error truncate" title={task.error}>{t("downloads.error")}: {task.error}</span>
                  {/if}
                </div>
              </div>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<!-- Add download dialog -->
{#if showAddDialog}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onclick={() => showAddDialog = false}>
    <div class="nx-card w-full max-w-[300px] mx-4" onclick={(e) => e.stopPropagation()}>
      <div class="flex items-center justify-between border-b border-nx-border px-3 py-2">
        <h2 class="text-sm font-semibold text-nx-text">{t("downloads.add")}</h2>
        <button class="text-nx-text-muted hover:text-nx-text" onclick={() => showAddDialog = false}>
          <span class="material-symbols-outlined text-lg">close</span>
        </button>
      </div>
      <div class="p-3">
        <input type="url" bind:value={newUrl} placeholder="https://example.com/file.zip"
          class="nx-input w-full h-8 text-xs"
          onkeydown={(e) => { if (e.key === 'Enter') handleAddDownload(); }} />
        <div class="mt-3 flex justify-end gap-2">
          <button class="nx-btn h-7 text-xs" onclick={() => showAddDialog = false}>{t("common.cancel")}</button>
          <button class="nx-btn nx-btn-primary h-7 text-xs" onclick={handleAddDownload} disabled={!newUrl.trim()}>
            {t("downloads.add")}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Config dialog -->
{#if showConfigDialog && config}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onclick={() => showConfigDialog = false}>
    <div class="nx-card w-full max-w-[340px] mx-4" onclick={(e) => e.stopPropagation()}>
      <div class="flex items-center justify-between border-b border-nx-border px-3 py-2">
        <h2 class="text-sm font-semibold text-nx-text">{t("downloads.config_title")}</h2>
        <button class="text-nx-text-muted hover:text-nx-text" onclick={() => showConfigDialog = false}>
          <span class="material-symbols-outlined text-lg">close</span>
        </button>
      </div>
      <div class="p-3 space-y-2">
        <div class="grid grid-cols-2 gap-2">
          <div>
            <label class="block text-xs text-nx-text-muted mb-0.5">{t("downloads.max_concurrent")}</label>
            <input type="number" bind:value={config.max_concurrent_tasks} class="nx-input w-full h-7 text-xs" min="1" max="16" />
          </div>
          <div>
            <label class="block text-xs text-nx-text-muted mb-0.5">{t("downloads.max_chunks")}</label>
            <input type="number" bind:value={config.max_chunks_per_task} class="nx-input w-full h-7 text-xs" min="1" max="32" />
          </div>
        </div>
        <div>
          <label class="block text-xs text-nx-text-muted mb-0.5">{t("downloads.default_save_path")}</label>
          <input type="text" bind:value={config.default_save_path} class="nx-input w-full h-7 text-xs" />
        </div>
        <div class="grid grid-cols-2 gap-2">
          <div>
            <label class="block text-xs text-nx-text-muted mb-0.5">{t("downloads.retry_count")}</label>
            <input type="number" bind:value={config.retry_count} class="nx-input w-full h-7 text-xs" min="0" max="10" />
          </div>
          <div class="flex items-end pb-0.5">
            <label class="flex items-center gap-1.5 text-xs text-nx-text-muted cursor-pointer">
              <input type="checkbox" bind:checked={config.enable_resume} class="rounded border-nx-border bg-nx-bg" />
              {t("downloads.enable_resume")}
            </label>
          </div>
        </div>
        <label class="flex items-center gap-1.5 text-xs text-nx-text-muted cursor-pointer">
          <input type="checkbox" bind:checked={config.auto_mirror_github} class="rounded border-nx-border bg-nx-bg" />
          {t("downloads.auto_mirror")}
        </label>
        <details class="text-xs">
          <summary class="text-nx-accent cursor-pointer">Cookies</summary>
          <textarea bind:value={config.cookie_string} rows="2"
            placeholder="key=value; key2=value2"
            class="nx-input w-full mt-1 text-xs"></textarea>
        </details>
      </div>
      <div class="flex justify-end gap-2 border-t border-nx-border px-3 py-2">
        <button class="nx-btn h-7 text-xs" onclick={() => showConfigDialog = false}>{t("common.cancel")}</button>
        <button class="nx-btn nx-btn-primary h-7 text-xs" onclick={handleSaveConfig}>{t("common.save")}</button>
      </div>
    </div>
  </div>
{/if}

<!-- Mirror management dialog -->
{#if showMirrorDialog}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onclick={() => showMirrorDialog = false}>
    <div class="nx-card w-full max-w-[400px] mx-4" onclick={(e) => e.stopPropagation()}>
      <div class="flex items-center justify-between border-b border-nx-border px-3 py-2">
        <h2 class="text-sm font-semibold text-nx-text">{t("downloads.mirrors")}</h2>
        <button class="text-nx-text-muted hover:text-nx-text" onclick={() => showMirrorDialog = false}>
          <span class="material-symbols-outlined text-lg">close</span>
        </button>
      </div>
      <div class="p-3 space-y-2 max-h-[300px] overflow-y-auto">
        {#each mirrors as mirror, i}
          <div class="flex items-center gap-1.5">
            <input type="checkbox" bind:checked={mirror.enabled} class="rounded border-nx-border bg-nx-bg shrink-0" title="Enabled" />
            <input type="text" bind:value={mirror.name} placeholder="Name"
              class="nx-input w-20 h-7 text-xs" />
            <input type="text" bind:value={mirror.url_prefix} placeholder="https://..."
              class="nx-input flex-1 h-7 text-xs" />
            <button
              class="text-xs px-1 rounded {mirror.strip_host ? 'text-nx-accent bg-nx-accent-bg' : 'text-nx-text-muted'}"
              onclick={() => mirror.strip_host = !mirror.strip_host}
              title="strip_host: remove original domain">
              {mirror.strip_host ? 'strip' : 'full'}
            </button>
            <button class="text-nx-error hover:text-nx-error/80 shrink-0" onclick={() => removeMirror(i)}>
              <span class="material-symbols-outlined text-sm">remove_circle</span>
            </button>
          </div>
        {/each}
      </div>
      <div class="flex justify-between gap-2 px-3 py-2 border-t border-nx-border">
        <button class="nx-btn h-7 text-xs" onclick={addMirror}>
          <span class="material-symbols-outlined text-xs">add</span> {t("downloads.add_mirror")}
        </button>
        <div class="flex gap-2">
          <button class="nx-btn h-7 text-xs" onclick={() => showMirrorDialog = false}>{t("common.cancel")}</button>
          <button class="nx-btn nx-btn-primary h-7 text-xs" onclick={handleSaveMirrors}>{t("common.save")}</button>
        </div>
      </div>
    </div>
  </div>
{/if}