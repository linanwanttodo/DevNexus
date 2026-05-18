<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let groups = $state([]);
  let loading = $state(true);
  let error = $state(null);
  let testing = $state(null);
  let selectedCountry = $state("all");

  const countries = [
    { id: "all", label: "All Countries" },
    { id: "CN", label: "China" },
    { id: "RU", label: "Russia" },
    { id: "US", label: "United States" },
    { id: "EU", label: "Europe" },
    { id: "JP", label: "Japan" },
  ];

  async function loadMirrors() {
    try {
      loading = true;
      error = null;
      groups = await invoke("list_mirrors");
      // 标记当前激活的镜像
      for (const g of groups) {
        for (const m of g.mirrors) {
          m.is_active = g.current_url && m.url === g.current_url;
        }
      }
    } catch (err) {
      error = err.message || "Failed to load mirrors";
    } finally {
      loading = false;
    }
  }

  async function testMirror(groupId, mirrorUrl) {
    testing = mirrorUrl;
    try {
      const latency = await invoke("test_mirror_latency", { url: mirrorUrl });
      for (const g of groups) {
        if (g.id === groupId) {
          for (const m of g.mirrors) {
            if (m.url === mirrorUrl) {
              m.latency_ms = latency;
            }
          }
        }
      }
    } catch (err) {
      console.error("Latency test failed:", err);
    } finally {
      testing = null;
    }
  }

  async function switchMirror(groupId, mirrorUrl) {
    try {
      const msg = await invoke("switch_mirror", { mirrorId: groupId, url: mirrorUrl });
      alert(msg);
      await loadMirrors();
    } catch (err) {
      alert(`Failed: ${err.message || err}`);
    }
  }

  function getCountryFlag(code) {
    const flags = { CN: "CN", RU: "RU", US: "US", EU: "EU", JP: "JP" };
    return flags[code] || code;
  }

  let filteredGroups = $derived(
    selectedCountry === "all"
      ? groups
      : groups.map(g => ({
          ...g,
          mirrors: g.mirrors.filter(m => m.country === selectedCountry || g.mirrors.every(x => x.country !== selectedCountry))
        }))
  );

  onMount(() => { loadMirrors(); });
</script>

<div class="mx-auto max-w-5xl">
  <div class="mb-6 flex items-center justify-between">
    <div>
      <h1 class="text-xl font-semibold text-nx-text">Package Mirrors</h1>
      <p class="mt-1 text-xs text-nx-text-muted">Select the fastest mirror for each package manager</p>
    </div>
    <div class="flex items-center gap-3">
      <select
        bind:value={selectedCountry}
        class="border border-nx-border bg-nx-surface px-3 py-1.5 text-sm text-nx-text outline-none"
      >
        {#each countries as c}
          <option value={c.id}>{c.label}</option>
        {/each}
      </select>
      <button class="border border-nx-border px-4 py-1.5 text-sm text-nx-text-secondary" onclick={loadMirrors}>
        Refresh
      </button>
    </div>
  </div>

  {#if loading}
    <div class="flex items-center justify-center py-12">
      <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
    </div>
  {:else if error}
    <div class="p-6 text-center">
      <span class="material-symbols-outlined text-nx-danger text-3xl">error</span>
      <div class="mt-2 text-sm text-nx-danger">{error}</div>
      <button class="mt-4 bg-nx-accent px-4 py-2 text-sm font-medium text-white" onclick={loadMirrors}>Retry</button>
    </div>
  {:else}
    <div class="space-y-4">
      {#each filteredGroups as group}
        <div class="border border-nx-border bg-nx-surface">
          <div class="flex items-center gap-3 border-b border-nx-border px-4 py-3">
            <span class="text-nx-accent text-sm font-medium">{group.label}</span>
            {#if group.current_url}
              <span class="text-xs text-nx-text-muted">Active: {group.current_url}</span>
            {/if}
          </div>
          <div class="p-3 grid grid-cols-1 gap-2">
            {#each group.mirrors as mirror}
              <div class="flex items-center justify-between border border-nx-border bg-nx-bg px-3 py-2 {mirror.is_active ? 'border-nx-accent' : ''}">
                <div class="flex items-center gap-3 flex-1 min-w-0">
                  <span class="text-xs text-nx-text-muted w-10">{getCountryFlag(mirror.country)}</span>
                  <div class="min-w-0">
                    <div class="text-sm text-nx-text truncate">{mirror.name}</div>
                    <code class="text-xs text-nx-text-muted truncate block">{mirror.url}</code>
                  </div>
                </div>
                <div class="flex items-center gap-2 flex-shrink-0 ml-3">
                  <button
                    class="px-2 py-1 text-xs text-nx-text-muted border border-nx-border"
                    onclick={() => testMirror(group.id, mirror.url)}
                    disabled={testing !== null}
                  >
                    {testing === mirror.url ? "..." : mirror.latency_ms > 0 ? `${mirror.latency_ms}ms` : "Test"}
                  </button>
                  {#if mirror.is_active}
                    <span class="text-xs text-nx-success font-medium">Active</span>
                  {:else}
                    <button
                      class="px-2 py-1 text-xs font-medium bg-nx-accent text-white"
                      onclick={() => switchMirror(group.id, mirror.url)}
                    >
                      Use
                    </button>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>