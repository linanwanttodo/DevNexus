<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.js";
  import { showConfirm } from "../lib/confirm.js";
  import { getSearchQuery, onSearchChange, setSearchQuery } from "../lib/stores.js";
  import { t, getVersion, onLangChange } from "../lib/i18n.js";

  let _v = $state(getVersion());
  $effect(() => onLangChange(v => _v = v));

  let ports = $state([]);
  let loading = $state(true);
  let error = $state(null);
  let search = $state(getSearchQuery());
  let killing = $state(null);

  $effect(() => {
    return onSearchChange((q) => {
      search = q;
    });
  });

  async function loadPorts() {
    try {
      loading = true;
      error = null;
      ports = await invoke("list_ports");
    } catch (err) {
      error = err.message || "Failed to list ports";
    } finally {
      loading = false;
    }
  }

  async function killPort(port) {
    if (!await showConfirm(`Kill process on port ${port}?`)) return;
    killing = port;
    try {
      const msg = await invoke("kill_port", { port });
      showToast(msg);
      await loadPorts();
    } catch (err) {
      showToast(`Failed: ${err.message || err}`);
    } finally {
      killing = null;
    }
  }

  let filtered = $derived(
    search.trim()
      ? ports.filter(p =>
          p.port.toString().includes(search) ||
          p.process_name.toLowerCase().includes(search.toLowerCase()) ||
          p.pid.toString().includes(search)
        )
      : ports
  );

  onMount(() => {
    loadPorts();
  });
</script>

<div class="mx-auto max-w-4xl">
  <div class="mb-6 flex items-center justify-between">
    <div>
      <h1 class="text-xl font-semibold text-nx-text">{_v && t("port_manager.title")}</h1>
      <p class="mt-1 text-xs text-nx-text-muted">{_v && t("port_manager.description")}</p>
    </div>
    <button
      class="flex items-center gap-2 border border-nx-border px-4 py-2 text-sm font-medium text-nx-text-secondary"
      onclick={loadPorts}
    >
      <span class="material-symbols-outlined text-lg">refresh</span>
      {_v && t("port_manager.refresh")}
    </button>
  </div>

  <!-- Search -->
  <div class="mb-4">
    <div class="relative">
      <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-nx-text-muted">search</span>
      <input
        type="text"
        placeholder={_v ? t("port_manager.search_placeholder") : "Search..."}
        value={search}
        oninput={(e) => { search = e.target.value; setSearchQuery(e.target.value); }}
        class="w-full border border-nx-border bg-nx-surface px-10 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-accent"
      />
      {#if search}
        <button
          class="absolute right-3 top-1/2 -translate-y-1/2 text-nx-text-muted"
          onclick={() => search = ""}
        >
          <span class="material-symbols-outlined text-sm">close</span>
        </button>
      {/if}
    </div>
  </div>

  <!-- Ports Table -->
  <div class="border border-nx-border bg-nx-surface">
    {#if loading}
      <div class="flex items-center justify-center py-12">
        <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
      </div>
    {:else if error}
      <div class="p-6 text-center">
        <span class="material-symbols-outlined text-nx-danger text-3xl">error</span>
        <div class="mt-2 text-sm text-nx-danger">{error}</div>
        <button class="mt-4 bg-nx-accent px-4 py-2 text-sm font-medium text-white" onclick={loadPorts}>{_v && t("common.retry")}</button>
      </div>
    {:else if filtered.length === 0}
      <div class="p-12 text-center">
        <span class="material-symbols-outlined text-nx-text-muted text-4xl">lan</span>
        <div class="mt-4 text-sm text-nx-text-muted">
          {search ? (_v && t("port_manager.no_matching")) : (_v && t("port_manager.no_ports"))}
        </div>
      </div>
    {:else}
    <table class="w-full">
      <thead>
        <tr class="border-b border-nx-border text-xs text-nx-text-muted">
          <th class="px-4 py-3 text-left font-medium w-20">{_v && t("port_manager.port")}</th>
          <th class="px-4 py-3 text-left font-medium w-16">{_v && t("port_manager.proto")}</th>
          <th class="px-4 py-3 text-left font-medium">{_v && t("port_manager.process")}</th>
          <th class="px-4 py-3 text-left font-medium w-20">{_v && t("port_manager.pid")}</th>
          <th class="px-4 py-3 text-right font-medium w-24">{_v && t("port_manager.actions")}</th>
        </tr>
      </thead>
      <tbody>
        {#each filtered as entry}
          <tr class="border-b border-nx-border last:border-0">
            <td class="px-4 py-3">
              <span class="font-mono text-sm font-medium text-nx-accent">{entry.port}</span>
            </td>
            <td class="px-4 py-3">
              <span class="text-xs text-nx-text-muted">{entry.protocol}</span>
            </td>
            <td class="px-4 py-3 text-sm text-nx-text-secondary">{entry.process_name}</td>
            <td class="px-4 py-3 font-mono text-xs text-nx-text-muted">{entry.pid}</td>
            <td class="px-4 py-3">
              <div class="flex justify-end">
                <button
                  class="px-3 py-1 text-xs font-medium text-nx-danger border border-nx-border disabled:opacity-30"
                  onclick={() => killPort(entry.port)}
                  disabled={killing !== null}
                >
                  {killing === entry.port ? "..." : (_v && t("port_manager.kill"))}
                </button>
              </div>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>

    <div class="flex items-center justify-between border-t border-nx-border px-4 py-2">
      <span class="text-xs text-nx-text-muted">{filtered.length} {_v && t(filtered.length === 1 ? "port_manager.port" : "port_manager.ports")}</span>
      <button class="text-xs text-nx-text-muted" onclick={loadPorts}>{_v && t("port_manager.refresh")}</button>
    </div>
    {/if}
  </div>
</div>