<script>
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.svelte.js";
  import { t } from "../lib/i18n.svelte.js";

  let activeTab = $state("export");
  let environments = $state([]);
  let loading = $state(true);
  let error = $state(null);

  // 导出：选中的环境名称
  let selectedEnvs = $state([]);
  // 每个环境的版本列表：name -> VersionInfo[]
  let versionsMap = $state({});
  let loadingVersions = $state({});
  // 选中的版本：name -> VersionSnapshot[]
  let selectedVersions = $state({});

  // 导入
  let importManifest = $state(null);
  let importPath = $state("");
  let applyVersions = $state(true);
  let importing = $state(false);
  let importResult = $state(null);

  const versionManagedTypes = ["python", "node", "java", "go", "rust", "cpp"];

  async function loadEnvironments() {
    try {
      loading = true;
      error = null;
      environments = await invoke("list_environments");
    } catch (err) {
      error = err.message || String(err);
    } finally {
      loading = false;
    }
  }

  function toggleEnv(name) {
    if (selectedEnvs.includes(name)) {
      selectedEnvs = selectedEnvs.filter((n) => n !== name);
      versionsMap[name] = undefined;
      selectedVersions[name] = [];
    } else {
      selectedEnvs = [...selectedEnvs, name];
      loadVersions(name);
    }
  }

  async function loadVersions(name) {
    const env = environments.find((e) => e.name === name);
    if (!env || !versionManagedTypes.includes(env.lang_type)) return;
    loadingVersions[name] = true;
    try {
      const vers = await invoke("list_versions", { langType: env.lang_type });
      versionsMap[name] = vers || [];
      if (!selectedVersions[name]) selectedVersions[name] = [];
    } catch (err) {
      versionsMap[name] = [];
      showToast(t("migration.versions_failed").replace("{error}", err.message || err), "error");
    } finally {
      loadingVersions[name] = false;
    }
  }

  function toggleVersion(name, ver) {
    const env = environments.find((e) => e.name === name);
    const snap = { lang_type: env.lang_type, version: ver.version };
    const arr = selectedVersions[name] || [];
    const idx = arr.findIndex((v) => v.version === ver.version);
    if (idx >= 0) arr.splice(idx, 1);
    else arr.push(snap);
    selectedVersions[name] = [...arr];
  }

  let selectedVersionCount = $derived(
    Object.values(selectedVersions).reduce((sum, arr) => sum + (arr ? arr.length : 0), 0)
  );

  async function exportMigration() {
    if (selectedEnvs.length === 0) {
      showToast(t("migration.select_env"), "error");
      return;
    }
    const versions = Object.values(selectedVersions).flat();
    try {
      const json = await invoke("export_migration", {
        selected: { environments: selectedEnvs, versions },
      });
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `devnexus-migration-${new Date().toISOString().slice(0, 10)}.json`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
      showToast(t("migration.exported"));
    } catch (err) {
      showToast(t("migration.export_failed").replace("{error}", err.message || err), "error");
    }
  }

  async function pickImportFile() {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!selected) return;
      importPath = selected;
      importResult = null;
      importManifest = await invoke("load_migration_file", { path: importPath });
    } catch (err) {
      importManifest = null;
      showToast(t("migration.import_failed").replace("{error}", err.message || err), "error");
    }
  }

  async function runImport() {
    if (!importManifest) {
      showToast(t("migration.empty_file"), "error");
      return;
    }
    importing = true;
    importResult = null;
    try {
      const json = JSON.stringify(importManifest);
      const result = await invoke("import_migration", {
        json,
        applyVersions,
      });
      importResult = result;
      showToast(
        t("migration.import_success")
          .replace("{switched}", result.switched)
          .replace("{skipped}", result.skipped)
          .replace("{failed}", result.failed)
      );
      await loadEnvironments();
    } catch (err) {
      showToast(t("migration.import_failed").replace("{error}", err.message || err), "error");
    } finally {
      importing = false;
    }
  }

  onMount(loadEnvironments);
</script>

<div class="mx-auto max-w-4xl p-5">
  <!-- Header -->
  <div class="mb-6 flex items-center justify-between">
    <div>
      <h1 class="text-xl font-semibold text-nx-text">{t("migration.title")}</h1>
      <p class="mt-1 text-xs text-nx-text-muted">{t("migration.desc")}</p>
    </div>
  </div>

  <!-- Tabs -->
  <div class="mb-4 flex gap-1 border-b border-nx-border">
    <button
      type="button"
      class="px-3 py-2 text-sm transition-colors {activeTab === 'export'
        ? 'border-b-2 border-nx-accent font-medium text-nx-text'
        : 'text-nx-text-muted hover:text-nx-text'}"
      onclick={() => (activeTab = "export")}
    >
      {t("migration.tab_export")}
    </button>
    <button
      type="button"
      class="px-3 py-2 text-sm transition-colors {activeTab === 'import'
        ? 'border-b-2 border-nx-accent font-medium text-nx-text'
        : 'text-nx-text-muted hover:text-nx-text'}"
      onclick={() => (activeTab = "import")}
    >
      {t("migration.tab_import")}
    </button>
  </div>

  {#if activeTab === "export"}
    <div class="mb-4 flex justify-end">
      <button
        class="nx-btn nx-btn-primary flex items-center gap-2"
        onclick={exportMigration}
        disabled={selectedEnvs.length === 0}
      >
        <span class="material-symbols-outlined text-lg">file_download</span>
        {t("migration.export")}
      </button>
    </div>

    <div class="nx-section">
      {#if loading}
        <div class="flex items-center justify-center py-12">
          <span class="material-symbols-outlined nx-animate-spin text-nx-text-muted text-3xl">progress_activity</span>
        </div>
      {:else if error}
        <div class="nx-empty">
          <span class="material-symbols-outlined text-nx-danger text-3xl">error</span>
          <div class="mt-2 text-sm text-nx-danger">{error}</div>
          <button class="nx-btn nx-btn-primary mt-4" onclick={loadEnvironments}>{t("common.retry")}</button>
        </div>
      {:else if environments.length === 0}
        <div class="nx-empty">
          <span class="material-symbols-outlined text-nx-text-muted text-4xl">inbox</span>
          <div class="mt-2 text-sm text-nx-text-muted">{t("migration.no_envs")}</div>
        </div>
      {:else}
        <div class="divide-y divide-nx-border">
          {#each environments as env}
            <div class="py-3">
              <label class="flex cursor-pointer items-center gap-3">
                <input
                  type="checkbox"
                  checked={selectedEnvs.includes(env.name)}
                  onchange={() => toggleEnv(env.name)}
                  class="h-4 w-4 rounded border-nx-border bg-nx-bg"
                />
                <div class="min-w-0">
                  <div class="flex items-center gap-2">
                    <span class="text-sm font-medium text-nx-text">{env.name}</span>
                    <span class="font-mono text-xs text-nx-text-muted">v{env.version}</span>
                    <span class="nx-pill text-[10px]">{env.lang_type}</span>
                  </div>
                  <div class="truncate font-mono text-xs text-nx-text-muted">{env.path}</div>
                </div>
              </label>

              {#if selectedEnvs.includes(env.name) && versionManagedTypes.includes(env.lang_type)}
                <div class="ml-7 mt-2">
                  {#if loadingVersions[env.name]}
                    <div class="flex items-center gap-2 text-xs text-nx-text-muted">
                      <span class="material-symbols-outlined nx-animate-spin text-sm">progress_activity</span>
                      {t("common.loading")}
                    </div>
                  {:else if versionsMap[env.name] && versionsMap[env.name].length > 0}
                    <div class="flex flex-wrap gap-1.5">
                      {#each versionsMap[env.name] as ver}
                        <button
                          type="button"
                          class="nx-pill text-[11px] transition-colors {selectedVersions[env.name]?.some((v) => v.version === ver.version) ? 'bg-nx-accent text-white' : 'hover:bg-nx-hover'}"
                          onclick={() => toggleVersion(env.name, ver)}
                        >
                          {ver.version}
                        </button>
                      {/each}
                    </div>
                  {:else}
                    <div class="text-xs text-nx-text-muted">{t("migration.no_versions")}</div>
                  {/if}
                </div>
              {/if}
            </div>
          {/each}
        </div>

        <div class="flex items-center justify-between border-t border-nx-border px-1 py-3">
          <span class="text-xs text-nx-text-muted">
            {t("migration.summary")
              .replace("{envs}", selectedEnvs.length)
              .replace("{versions}", selectedVersionCount)}
          </span>
          <button
            class="nx-btn nx-btn-ghost text-xs"
            onclick={loadEnvironments}
            disabled={loading}
          >
            <span class="material-symbols-outlined text-sm">refresh</span>
            {t("common.refresh")}
          </button>
        </div>
      {/if}
    </div>
  {:else}
    <!-- Import tab -->
    <div class="nx-section space-y-4 p-4">
      <p class="text-xs text-nx-text-muted">{t("migration.import_note")}</p>

      <div class="flex flex-wrap items-center gap-3">
        <button class="nx-btn nx-btn-primary flex items-center gap-2" onclick={pickImportFile}>
          <span class="material-symbols-outlined text-lg">folder_open</span>
          {t("migration.import_pick")}
        </button>
        {#if importPath}
          <span class="truncate font-mono text-xs text-nx-text-muted" title={importPath}>{importPath}</span>
        {/if}
      </div>

      {#if importManifest}
        <div class="rounded-md border border-nx-border bg-nx-surface p-3">
          <div class="mb-2 text-sm font-medium text-nx-text">{t("migration.import_preview")}</div>
          <div class="grid gap-1 text-xs text-nx-text-muted sm:grid-cols-2">
            <div>{t("migration.exported_at")}: {importManifest.meta?.exported_at || "—"}</div>
            <div>{t("migration.meta_os")}: {importManifest.meta?.source_os || "—"}</div>
            <div>{t("migration.meta_host")}: {importManifest.meta?.hostname || "—"}</div>
            <div>DevNexus: {importManifest.meta?.devnexus_version || "—"}</div>
          </div>
          <div class="mt-3 text-xs text-nx-text">
            {importManifest.environments?.length || 0} envs · {importManifest.versions?.length || 0} versions
          </div>
          {#if importManifest.environments?.length}
            <ul class="mt-2 max-h-40 space-y-1 overflow-y-auto text-xs text-nx-text-muted">
              {#each importManifest.environments as env}
                <li class="flex gap-2">
                  <span class="font-medium text-nx-text">{env.name}</span>
                  <span class="font-mono">{env.version}</span>
                  <span class="nx-pill text-[10px]">{env.lang_type}</span>
                </li>
              {/each}
            </ul>
          {/if}
          {#if importManifest.versions?.length}
            <div class="mt-2 flex flex-wrap gap-1.5">
              {#each importManifest.versions as ver}
                <span class="nx-pill text-[11px]">{ver.lang_type}@{ver.version}</span>
              {/each}
            </div>
          {/if}
        </div>

        <label class="flex cursor-pointer items-center gap-2 text-sm text-nx-text">
          <input type="checkbox" bind:checked={applyVersions} class="h-4 w-4 rounded border-nx-border bg-nx-bg" />
          {t("migration.apply_versions")}
        </label>

        <button
          class="nx-btn nx-btn-primary flex items-center gap-2"
          onclick={runImport}
          disabled={importing}
        >
          {#if importing}
            <span class="material-symbols-outlined nx-animate-spin text-lg">progress_activity</span>
          {:else}
            <span class="material-symbols-outlined text-lg">file_upload</span>
          {/if}
          {t("migration.import")}
        </button>
      {/if}

      {#if importResult}
        <div class="rounded-md border border-nx-border p-3 text-xs">
          <div class="mb-2 font-medium text-nx-text">
            switched {importResult.switched} · skipped {importResult.skipped} · failed {importResult.failed}
          </div>
          <ul class="max-h-48 space-y-1 overflow-y-auto text-nx-text-muted">
            {#each importResult.details as line}
              <li class="font-mono">{line}</li>
            {/each}
          </ul>
        </div>
      {/if}
    </div>
  {/if}
</div>
