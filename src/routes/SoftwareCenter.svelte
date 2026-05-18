<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let selectedCategory = $state("all");
  let filterInstalled = $state(false);
  let filterUpdates = $state(false);
  let software = $state([]);
  let loading = $state(true);
  let error = $state(null);
  let installing = $state(false);
  let currentItem = $state(null);

  const categories = [
    { id: "all", label: "All Software" },
    { id: "ide", label: "IDEs & Editors" },
    { id: "database", label: "Databases" },
    { id: "cli", label: "CLI Tools" },
    { id: "runtime", label: "Runtimes" },
  ];

  // 加载软件列表
  async function loadSoftware() {
    try {
      loading = true;
      error = null;
      software = await invoke("list_software");
    } catch (err) {
      error = err.message || "Failed to load software list";
      console.error("Error loading software:", err);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadSoftware();
  });

  // 处理安装/卸载操作
  async function handleAction(item) {
    if (!item.package_name) {
      alert("Package name not available");
      return;
    }

    if (item.action === "Install") {
      if (!confirm(`Install ${item.name}?`)) return;
      
      installing = true;
      currentItem = item;
      
      try {
        const result = await invoke("install_software", { packageName: item.package_name });
        alert(result);
        await loadSoftware();
      } catch (err) {
        alert(`Installation failed: ${err.message || err}`);
      } finally {
        installing = false;
        currentItem = null;
      }
    } else if (item.action === "Open") {
      // 打开已安装的软件（需要实现）
      alert(`Opening ${item.name}...\n(This feature needs platform-specific implementation)`);
    }
  }

  let filteredSoftware = $derived(software.filter((s) => {
    if (selectedCategory !== "all" && s.category !== selectedCategory) return false;
    if (filterInstalled && s.status !== "installed") return false;
    if (filterUpdates && s.status !== "updates") return false;
    return true;
  }));
</script>

<div class="flex gap-6">
  <!-- Sidebar Filters -->
  <aside class="w-48 flex-shrink-0">
    <div class="mb-6">
      <h3 class="mb-2 text-xs font-medium uppercase tracking-wider text-nx-text-muted">Categories</h3>
      <ul class="space-y-px">
        {#each categories as cat}
          <li>
            <button
              class="w-full px-3 py-1.5 text-left text-sm {selectedCategory === cat.id
                ? 'bg-nx-raised text-nx-text font-medium'
                : 'text-nx-text-secondary'}"
              onclick={() => selectedCategory = cat.id}>
              {cat.label}
            </button>
          </li>
        {/each}
      </ul>
    </div>

    <div>
      <h3 class="mb-2 text-xs font-medium uppercase tracking-wider text-nx-text-muted">Status</h3>
      <div class="space-y-2">
        <label class="flex items-center gap-2 text-sm text-nx-text-secondary cursor-pointer">
          <input type="checkbox" bind:checked={filterInstalled} class="border-nx-border bg-nx-bg text-nx-text" />
          Installed
        </label>
        <label class="flex items-center gap-2 text-sm text-nx-text-secondary cursor-pointer">
          <input type="checkbox" bind:checked={filterUpdates} class="border-nx-border bg-nx-bg text-nx-text" />
          Updates Available
        </label>
      </div>
    </div>
  </aside>

  <!-- Software Grid -->
  <div class="flex-1">
    <div class="mb-6 flex items-center justify-between">
      <h1 class="text-xl font-semibold text-nx-text">Software Center</h1>
      <button 
        class="border border-nx-border px-4 py-2 text-sm font-medium text-nx-text-secondary"
        onclick={loadSoftware}>
        <span class="material-symbols-outlined text-lg inline-block align-middle mr-1">refresh</span>
        Refresh
      </button>
    </div>

    {#if loading}
      <div class="flex items-center justify-center py-12">
        <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
      </div>
    {:else if error}
      <div class="p-6 text-center">
        <span class="material-symbols-outlined text-nx-text-muted text-3xl">error</span>
        <div class="mt-2 text-sm text-nx-text-secondary">{error}</div>
        <button 
          class="mt-4 bg-nx-text px-4 py-2 text-sm font-medium text-nx-deep"
          onclick={loadSoftware}>
          Retry
        </button>
      </div>
    {:else if filteredSoftware.length === 0}
      <div class="p-6 text-center">
        <span class="material-symbols-outlined text-nx-text-muted text-3xl">search_off</span>
        <div class="mt-2 text-sm text-nx-text-muted">No software found</div>
        <div class="mt-1 text-xs text-nx-text-muted">Try adjusting your filters</div>
      </div>
    {:else}
    <div class="grid grid-cols-2 gap-4 xl:grid-cols-3">
      {#each filteredSoftware as item}
        <div class="border border-nx-border bg-nx-surface p-4">
          <div class="mb-3 flex items-start justify-between">
            <div class="flex h-10 w-10 items-center justify-center bg-nx-bg">
              <span class="material-symbols-outlined text-nx-text-secondary">package</span>
            </div>
            <span class="px-2 py-0.5 text-xs font-medium
              {item.status === 'installed' ? 'bg-nx-text/15 text-nx-text' : item.status === 'available' ? 'bg-nx-text-secondary/15 text-nx-text-secondary' : 'bg-nx-overlay text-nx-text-muted'}">
              {item.status === 'installed' ? 'Installed' : item.status === 'available' ? 'Available' : 'System'}
            </span>
          </div>
          <h3 class="mb-1 text-sm font-medium text-nx-text">{item.name}</h3>
          <p class="mb-4 font-mono text-xs text-nx-text-muted">{item.version}</p>
          <button
            class="w-full px-4 py-2 text-xs font-medium disabled:opacity-50 disabled:cursor-not-allowed
            {item.action === 'Install'
              ? 'bg-nx-accent text-white'
              : item.action === 'Uninstall'
                ? 'bg-nx-danger text-white'
                : item.action === 'System Managed'
                  ? 'cursor-not-allowed bg-nx-overlay text-nx-text-muted'
                  : 'border border-nx-border bg-nx-bg text-nx-text-secondary'}"
            disabled={item.action === 'System Managed' || installing}
            onclick={() => handleAction(item)}>
            {installing && currentItem?.name === item.name ? 'Processing...' : item.action}
          </button>
        </div>
      {/each}
    </div>
    {/if}
  </div>
</div>
