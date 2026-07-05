<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.svelte.js";
  import { showConfirm } from "../lib/confirm.svelte.js";
  import { getSearchQuery, setSearchQuery } from "../lib/stores.svelte.js";
  import { t, tFormat } from "../lib/i18n.svelte.js";

  let apps = $state([]);
  let loading = $state(true);
  let error = $state(null);
  let search = $derived(getSearchQuery());

  // Uninstall state
  let uninstalling = $state(null);  // app name being uninstalled

  // Residue scan state
  let scanning = $state(null);       // app name being scanned
  let residueScans = $state({});     // { [appName]: ResidueScan }
  let selectedResidues = $state({}); // { [appName]: { [path]: bool } }
  let cleaningResidues = $state(null); // app name being cleaned
  let scanErrors = $state({});       // { [appName]: string }

  async function loadApps() {
    try {
      loading = true;
      error = null;
      apps = await invoke("list_installed_apps");
    } catch (err) {
      error = err.message || "Failed to list installed apps";
    } finally {
      loading = false;
    }
  }

  async function handleUninstall(app) {
    if (!await showConfirm(tFormat('uninstall_mgr.confirm', { name: app.name }) || `Uninstall ${app.name}?`)) return;

    uninstalling = app.name;
    try {
      const result = await invoke("uninstall_software_deep", { packageName: app.name, appName: app.name });
      showToast(result);
      // Auto-scan after uninstall
      await scanResidues(app, true);
      await loadApps();
    } catch (err) {
      showToast(`Uninstall failed: ${err.message || err}`);
      // Even if uninstall failed, offer to scan & clean residues
      await scanResidues(app, true);
    } finally {
      uninstalling = null;
    }
  }

  async function handleForceUninstall(app) {
    if (!await showConfirm(tFormat('uninstall_mgr.force_confirm', { name: app.name }) || `Force uninstall ${app.name}? This will kill all related processes and remove ALL residue files.`)) return;

    uninstalling = app.name;
    try {
      const result = await invoke("force_uninstall_software", { packageName: app.name, appName: app.name });
      showToast(result);
      delete residueScans[app.name];
      delete selectedResidues[app.name];
      await loadApps();
    } catch (err) {
      showToast(`Force uninstall failed: ${err.message || err}`);
    } finally {
      uninstalling = null;
    }
  }

  async function scanResidues(app, auto = false) {
    scanning = app.name;
    scanErrors[app.name] = null;
    try {
      const scan = await invoke("scan_app_residues", { appName: app.name, packageName: app.name });
      residueScans[app.name] = scan;
      // Select all residue items by default
      const sel = {};
      for (const item of getAllItems(scan)) {
        if (item.is_safe_to_delete) {
          sel[item.path] = true;
        }
      }
      selectedResidues[app.name] = sel;
    } catch (err) {
      scanErrors[app.name] = err.message || err;
      if (!auto) showToast(`Scan failed: ${err.message || err}`);
    } finally {
      scanning = null;
    }
  }

  function getAllItems(scan) {
    if (!scan) return [];
    let items = [
      ...scan.directories,
      ...scan.files,
      ...scan.shortcuts,
      ...scan.services,
    ];
    // registry_keys only exists on Windows
    if (scan.registry_keys) {
      items = items.concat(scan.registry_keys);
    }
    return items;
  }

  function getCategoryIcon(cat) {
    switch (cat) {
      case "config": return "settings";
      case "cache": return "cached";
      case "log": return "description";
      case "temp": return "delete";
      case "data": return "folder";
      case "shortcut": return "shortcut";
      case "service": return "precision_manufacturing";
      case "registry": return "database";
      default: return "file_present";
    }
  }

  function getCategoryLabel(cat) {
    switch (cat) {
      case "config": return t("residue.type_config");
      case "cache": return t("residue.type_cache");
      case "log": return t("residue.type_log");
      case "temp": return t("residue.type_temp");
      case "data": return t("residue.type_data");
      case "shortcut": return t("residue.type_shortcut");
      case "service": return t("residue.type_service");
      case "registry": return t("residue.type_registry");
      default: return cat;
    }
  }

  function formatSize(bytes) {
    if (bytes === 0) return "0 B";
    const units = ["B", "KB", "MB", "GB"];
    let i = 0;
    let size = bytes;
    while (size >= 1024 && i < units.length - 1) {
      size /= 1024;
      i++;
    }
    return `${size.toFixed(1)} ${units[i]}`;
  }

  async function cleanSelected(appName) {
    const appScan = residueScans[appName];
    if (!appScan) return;

    const sel = selectedResidues[appName] || {};
    const paths = Object.keys(sel).filter(p => sel[p]);
    if (paths.length === 0) {
      showToast(t("uninstall_mgr.nothing_selected"));
      return;
    }

    if (!await showConfirm(tFormat('uninstall_mgr.confirm_clean', { count: paths.length }) || `Clean ${paths.length} selected residue item(s)?`)) return;

    cleaningResidues = appName;
    try {
      const result = await invoke("clean_specific_residues", { items: paths });
      showToast(result);
      // Re-scan after cleaning
      await scanResidues({ name: appName });
    } catch (err) {
      showToast(`Cleanup failed: ${err.message || err}`);
    } finally {
      cleaningResidues = null;
    }
  }

  function toggleScan(app) {
    if (residueScans[app.name]) {
      // Close scan panel
      delete residueScans[app.name];
      delete selectedResidues[app.name];
      residueScans = { ...residueScans };
    } else {
      scanResidues(app);
    }
  }

  let filtered = $derived(
    search.trim()
      ? apps.filter(a =>
          a.name.toLowerCase().includes(search.toLowerCase()) ||
          a.source.toLowerCase().includes(search.toLowerCase()) ||
          a.version.toLowerCase().includes(search.toLowerCase())
        )
      : apps
  );

  let sourceFilter = $state("all");
  let sources = $derived([...new Set(apps.map(a => a.source))].sort());

  onMount(() => {
    loadApps();
  });
</script>

<div class="mx-auto max-w-5xl p-5">
  <!-- Header -->
  <div class="mb-6 flex items-center justify-between">
    <div>
      <h1 class="text-xl font-semibold text-nx-text">{t("uninstall_mgr.title")}</h1>
      <p class="mt-1 text-xs text-nx-text-muted">{t("uninstall_mgr.desc")}</p>
    </div>
    <button
      class="nx-btn nx-btn-ghost flex items-center gap-2 cursor-pointer"
      onclick={loadApps}
    >
      <span class="material-symbols-outlined text-lg">refresh</span>
      {t("common.refresh")}
    </button>
  </div>

  <!-- Search & Filter -->
  <div class="mb-4 flex items-center gap-3">
    <div class="nx-search flex-1">
      <span class="nx-search-icon material-symbols-outlined">search</span>
      <input
        class="nx-input"
        type="text"
        placeholder={t("uninstall_mgr.search")}
        value={search}
        oninput={(e) => { setSearchQuery(e.currentTarget.value); }}
      />
      {#if search}
        <button
          class="nx-search-clear material-symbols-outlined cursor-pointer"
          onclick={() => { setSearchQuery(""); }}
        >
          close
        </button>
      {/if}
    </div>
    <select
      class="nx-input px-3 py-2 text-sm"
      value={sourceFilter}
      onchange={(e) => sourceFilter = e.currentTarget.value}
    >
      <option value="all">{t("uninstall_mgr.all_sources")}</option>
      {#each sources as src}
        <option value={src}>{src}</option>
      {/each}
    </select>
  </div>

  <!-- App List -->
  <div class="nx-section">
    {#if loading}
      <div class="flex items-center justify-center py-16">
        <span class="material-symbols-outlined nx-animate-spin text-nx-text-muted text-3xl">progress_activity</span>
      </div>
    {:else if error}
      <div class="nx-empty">
        <span class="material-symbols-outlined text-nx-danger text-3xl">error</span>
        <div class="mt-2 text-sm text-nx-danger">{error}</div>
        <button class="nx-btn nx-btn-primary mt-4 cursor-pointer" onclick={loadApps}>{t("common.retry")}</button>
      </div>
    {:else if apps.length === 0}
      <div class="nx-empty p-12">
        <span class="material-symbols-outlined text-nx-text-muted text-4xl">delete</span>
        <div class="mt-4 text-sm text-nx-text-muted">{t("uninstall_mgr.no_apps")}</div>
      </div>
    {:else}
      <!-- Column headers -->
      <div class="hidden md:flex items-center border-b border-nx-border px-4 py-2 text-xs text-nx-text-muted font-medium">
        <div class="flex-1 min-w-0">{t("uninstall_mgr.app_name")}</div>
        <div class="w-36 text-right">{t("uninstall_mgr.version")}</div>
        <div class="w-28 text-right">{t("uninstall_mgr.source")}</div>
        <div class="w-28 text-right">{t("common.actions")}</div>
      </div>

      {#each filtered as app (app.name + app.source)}
        {#if sourceFilter === "all" || app.source === sourceFilter}
          <div class="border-b border-nx-border last:border-0">
            <!-- Main row -->
            <div class="flex items-center px-4 py-3 hover:bg-nx-surface-hover transition-colors">
              <!-- Name -->
              <div class="flex-1 min-w-0">
                <span class="text-sm font-medium text-nx-text">{app.name}</span>
                {#if residueScans[app.name]}
                  <span class="ml-2 inline-block rounded bg-nx-warning/10 px-1.5 py-0.5 text-[10px] font-medium text-nx-warning">
                    {tFormat("uninstall_mgr.residues_found", { count: residueScans[app.name].total_items })}
                  </span>
                {/if}
              </div>

              <!-- Version -->
              <div class="w-36 text-right">
                <span class="text-xs font-mono text-nx-text-secondary">{app.version}</span>
              </div>

              <!-- Source (Package Manager) -->
              <div class="w-28 text-right">
                <span class="inline-block rounded bg-nx-accent/10 px-2 py-0.5 text-xs font-medium text-nx-accent">{app.source}</span>
              </div>

              <!-- Actions -->
              <div class="w-28 flex justify-end gap-1">
                <button
                  class="nx-btn nx-btn-ghost px-2 py-1 text-xs cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed"
                  onclick={() => toggleScan(app)}
                  disabled={scanning === app.name || cleaningResidues === app.name}
                >
                  {#if scanning === app.name}
                    <span class="material-symbols-outlined text-xs inline nx-animate-spin">progress_activity</span>
                  {:else}
                    <span class="material-symbols-outlined text-xs inline">search</span>
                  {residueScans[app.name] ? t("uninstall_mgr.close_scan") : t("uninstall_mgr.residue_scan")}
                  {/if}
                </button>
                <button
                  class="nx-btn nx-btn-danger px-3 py-1 text-xs cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed"
                  onclick={() => handleUninstall(app)}
                  disabled={uninstalling !== null || scanning === app.name}
                >
                  {#if uninstalling === app.name}
                    <span class="material-symbols-outlined text-xs inline nx-animate-spin">progress_activity</span>
                  {:else}
                    {t("uninstall_mgr.uninstall")}
                  {/if}
                </button>
              </div>
            </div>

            <!-- Residue scan panel (expandable) -->
            {#if residueScans[app.name]}
              {@const scan = residueScans[app.name]}
              <div class="border-t border-nx-border bg-nx-surface/50 px-4 py-3">
                {#if scanErrors[app.name]}
                  <div class="text-sm text-nx-danger">{scanErrors[app.name]}</div>
                {:else}
                  <!-- Summary bar -->
                  <div class="mb-3 flex items-center justify-between">
                    <div class="flex items-center gap-3 text-xs text-nx-text-muted">
                      <span>
                    <span class="font-medium text-nx-text">{scan.total_items}</span> {t("uninstall_mgr.residues_found").replace(/\d+/, "")}
                      </span>
                      <span>
                        共 <span class="font-medium text-nx-text">{formatSize(scan.total_size)}</span>
                      </span>
                    </div>
                    <div class="flex gap-2">
                      <button
                        class="nx-btn nx-btn-ghost flex items-center gap-1 px-2.5 py-1 text-xs cursor-pointer disabled:opacity-30"
                        onclick={() => cleanSelected(app.name)}
                        disabled={cleaningResidues !== null || scanning !== null}
                      >
                        {#if cleaningResidues === app.name}
                          <span class="material-symbols-outlined text-xs nx-animate-spin">progress_activity</span>
                        {:else}
                          <span class="material-symbols-outlined text-xs">cleaning_services</span>
                        {/if}
                        {t("uninstall_mgr.clean_selected")}
                      </button>
                      <button
                        class="nx-btn nx-btn-danger flex items-center gap-1 px-2.5 py-1 text-xs cursor-pointer disabled:opacity-30"
                        onclick={() => handleForceUninstall(app)}
                        disabled={uninstalling !== null || cleaningResidues !== null}
                      >
                        <span class="material-symbols-outlined text-xs">delete_forever</span>
                        {t("uninstall_mgr.force_uninstall")}
                      </button>
                    </div>
                  </div>

                  <!-- Residue items grouped by category -->
                  {#if getAllItems(scan).length === 0}
                    <div class="py-4 text-center text-sm text-nx-text-muted">
                      <span class="material-symbols-outlined text-lg inline">check_circle</span>
                      {t("uninstall_mgr.no_residues")}
                    </div>
                  {:else}
                    {#each ["directories", "files", "shortcuts", "services", ...(scan.registry_keys ? ["registry_keys"] : [])] as key}
                      {#if scan[key] && scan[key].length > 0}
                        <div class="mb-2">
                          <div class="mb-1 flex items-center gap-2">
                            <button
                              class="flex items-center gap-1 text-xs font-medium text-nx-text cursor-pointer hover:text-nx-accent"
                              onclick={() => {
                                const sel = { ...(selectedResidues[app.name] || {}) };
                                const allSelected = scan[key].every(item => sel[item.path]);
                                for (const item of scan[key]) {
                                  if (item.is_safe_to_delete) {
                                    sel[item.path] = !allSelected;
                                  }
                                }
                                selectedResidues[app.name] = sel;
                              }}
                            >
                              <span class="material-symbols-outlined text-sm">
                                {key === "directories" ? "folder" : key === "files" ? "description" : key === "shortcuts" ? "shortcut" : key === "services" ? "precision_manufacturing" : "database"}
                              </span>
                              {t("residue.category_" + (key === "registry_keys" ? "registry" : key))}
                              <span class="text-nx-text-muted">({scan[key].length})</span>
                            </button>
                          </div>
                          <div class="space-y-0.5">
                            {#each scan[key] as item}
                              <div class="flex items-center gap-2 rounded px-2 py-1 text-xs hover:bg-nx-surface-hover">
                                <input
                                  type="checkbox"
                                  checked={selectedResidues[app.name]?.[item.path] ?? true}
                                  disabled={!item.is_safe_to_delete}
                                  onchange={(e) => {
                                    const sel = { ...(selectedResidues[app.name] || {}) };
                                    if (e.currentTarget.checked) {
                                      sel[item.path] = true;
                                    } else {
                                      delete sel[item.path];
                                    }
                                    selectedResidues[app.name] = sel;
                                  }}
                                  class="accent-nx-accent cursor-pointer shrink-0"
                                />
                                <span class="material-symbols-outlined text-nx-text-muted text-sm shrink-0">
                                  {getCategoryIcon(item.category)}
                                </span>
                                <span class="flex-1 truncate text-nx-text-secondary" title={item.path}>
                                  {item.path}
                                </span>
                                <span class="shrink-0 font-mono text-nx-text-muted">
                                  {item.size > 0 ? formatSize(item.size) : ""}
                                </span>
                                  {#if !item.is_safe_to_delete}
                                  <span class="shrink-0 rounded bg-nx-warning/10 px-1 py-0.5 text-[10px] text-nx-warning">{t("uninstall_mgr.caution")}</span>
                                {/if}
                              </div>
                            {/each}
                          </div>
                        </div>
                      {/if}
                    {/each}
                  {/if}
                {/if}
              </div>
            {/if}
          </div>
        {/if}
      {/each}

      <!-- Footer count -->
      <div class="flex items-center justify-between border-t border-nx-border px-4 py-2">
        <span class="text-xs text-nx-text-muted">
          {filtered.length} / {apps.length} {t("uninstall_mgr.apps_count")}
        </span>
      </div>
    {/if}
  </div>
</div>
