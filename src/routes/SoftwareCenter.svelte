<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.svelte.js";
  import { showConfirm } from "../lib/confirm.svelte.js";
  import { t, tFormat } from "../lib/i18n.svelte.js";
  import BrandIcons from "../icons/BrandIcons.svelte";

  let selectedCategory = $state("all");
  let filterInstalled = $state(false);
  let filterUpdates = $state(false);
  let software = $state([]);
  let loading = $state(true);
  let error = $state(null);
  let installing = $state(false);
  let currentItem = $state(null);
  let packageManagers = $state([]);
  let pmChecking = $state(true);

  const categories = [
    { id: "all", label: t("software.all") },
    { id: "ide", label: t("software.ide") },
    { id: "database", label: t("software.database") },
    { id: "cli", label: t("software.cli") },
    { id: "runtime", label: t("software.runtime") },
  ];

  function brandIconName(sw) {
    const map = {
      'Visual Studio Code':'vscode',
      'Neovim':'neovim',
      'Vim':'vim',
      'Node.js':'nodejs',
      'Python 3':'python',
      'Go':'go',
      'Rust':'rust',
      'Git':'git',
      'Docker Desktop':'docker',
      'Docker Engine':'docker',
      'Redis':'redis',
      'SQLite':'sqlite',
      'DBeaver Community':'dbeaver',
      'PostgreSQL Client':'postgresql',
      'Java (JDK)':'java',
      'Ruby':'ruby',
      'curl':'default',
      'wget':'default',
      'OpenSSH Client':'default',
      'GCC':'default',
      'Clang':'default',
      'CMake':'default',
      'htop':'default',
      'tmux':'default',
      'ripgrep':'default',
      'fd':'default',
      'jq':'default',
      'fzf':'default',
      'GParted':'default',
      'Sublime Text':'default',
      'Zed':'default',
      'Postman':'default',
      'IntelliJ IDEA Community':'default',
      'MySQL Workbench':'default',
      'TablePlus':'default',
      'Homebrew':'default'
    };
    return map[sw] || 'default';
  }

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

  // 检测系统是否有可用的包管理器
  async function checkPackageManagers() {
    try {
      pmChecking = true;
      packageManagers = await invoke("list_package_managers");
    } catch (err) {
      console.error("Error checking package managers:", err);
      packageManagers = [];
    } finally {
      pmChecking = false;
    }
  }

  onMount(() => {
    loadSoftware();
    checkPackageManagers();
  });

  // 处理安装/卸载操作
  async function handleAction(item) {
    if (!item.package_name) {
      showToast("Package name not available");
      return;
    }

    if (item.action === "Install") {
      if (!await showConfirm(tFormat('software.install_confirm', { name: item.name }) || `Install ${item.name}?`)) return;
      
      installing = true;
      currentItem = item;
      
      try {
        const result = await invoke("install_software", { packageName: item.package_name });
        showToast(result);
        await loadSoftware();
      } catch (err) {
        showToast(`Installation failed: ${err.message || err}`);
      } finally {
        installing = false;
        currentItem = null;
      }
    } else if (item.action === "Uninstall") {
      if (!await showConfirm(tFormat('software.uninstall_confirm', { name: item.name }) || `Uninstall ${item.name}?`)) return;

      const removeData = await showConfirm(tFormat('software.uninstall_data_confirm', { name: item.name }) || `Also remove config and data files for ${item.name}?`);

      installing = true;
      currentItem = item;

      try {
        let result;
        if (removeData) {
          result = await invoke("uninstall_software_deep", { packageName: item.package_name, appName: item.name });
        } else {
          result = await invoke("uninstall_software", { packageName: item.package_name });
        }
        showToast(result);
        await loadSoftware();
      } catch (err) {
        showToast(`Uninstall failed: ${err.message || err}`);
      } finally {
        installing = false;
        currentItem = null;
      }
    } else if (item.action === "Open") {
      showToast(`Opening ${item.name}...\n(This feature needs platform-specific implementation)`);
    }
  }

  let hasPackageManager = $derived(packageManagers.length > 0);

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
      <h3 class="mb-2 text-xs font-medium uppercase tracking-wider text-nx-text-muted">{t("software.categories")}</h3>
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
      <h3 class="mb-2 text-xs font-medium uppercase tracking-wider text-nx-text-muted">{t("software.status")}</h3>
      <div class="space-y-2">
        <label class="flex items-center gap-2 text-sm text-nx-text-secondary cursor-pointer">
          <input type="checkbox" bind:checked={filterInstalled} class="border-nx-border bg-nx-bg text-nx-text" />
          {t("software.installed_filter")}
        </label>
        <label class="flex items-center gap-2 text-sm text-nx-text-secondary cursor-pointer">
          <input type="checkbox" bind:checked={filterUpdates} class="border-nx-border bg-nx-bg text-nx-text" />
          {t("software.updates_filter")}
        </label>
      </div>
    </div>
  </aside>

  <!-- Software Grid -->
  <div class="flex-1">
    <div class="mb-6 flex items-center justify-between">
      <h1 class="text-xl font-semibold text-nx-text">{t("software.title")}</h1>
      <button 
        class="border border-nx-border px-4 py-2 text-sm font-medium text-nx-text-secondary"
        onclick={loadSoftware}>
        <span class="material-symbols-outlined text-lg inline-block align-middle mr-1">refresh</span>
        {t("common.refresh")}
      </button>
    </div>

    {#if loading}
      <div class="flex items-center justify-center py-12">
        <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
      </div>
    {:else if !hasPackageManager && !pmChecking}
      <!-- 未检测到包管理器时的引导提示 -->
      <div class="border border-nx-border bg-nx-surface p-8 text-center">
        <span class="material-symbols-outlined text-nx-text-muted text-4xl">package_2</span>
        <h2 class="mt-4 text-lg font-semibold text-nx-text">{t("software.no_pm_title")}</h2>
        <p class="mt-2 max-w-md mx-auto text-sm text-nx-text-secondary">{t("software.no_pm_desc")}</p>
        <div class="mt-6 flex flex-wrap justify-center gap-4">
          {#if packageManagers.length > 0}
            {#each packageManagers as pm}
              <span class="px-3 py-1 text-xs bg-nx-text/10 text-nx-text rounded">{pm.name}</span>
            {/each}
          {:else}
            <div class="text-left text-sm text-nx-text-secondary">
              <p class="font-medium mb-2">{t("software.no_pm_suggest")}</p>
              <ul class="list-disc list-inside space-y-1">
                <li><span class="font-medium">macOS:</span> <a href="https://brew.sh" target="_blank" class="text-nx-accent underline">Homebrew</a></li>
                <li><span class="font-medium">Linux:</span> apt, dnf, pacman, zypper, apk</li>
                <li><span class="font-medium">Windows:</span> winget (Win 11/10 1809+)，<a href="https://chocolatey.org/install" target="_blank" class="text-nx-accent underline">Chocolatey</a></li>
              </ul>
            </div>
          {/if}
        </div>
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
        <div class="mt-2 text-sm text-nx-text-muted">{t("software.none")}</div>
        <div class="mt-1 text-xs text-nx-text-muted">{t("software.none_hint")}</div>
      </div>
    {:else}
    <div class="grid grid-cols-2 gap-4 xl:grid-cols-3">
      {#each filteredSoftware as item}
        <div class="border border-nx-border bg-nx-surface p-4">
          <div class="mb-3 flex items-start justify-between">
            <div class="flex h-10 w-10 items-center justify-center bg-nx-bg">
              <BrandIcons name={brandIconName(item.name)} size={22} class="text-nx-text-secondary" />
            </div>
            <span class="px-2 py-0.5 text-xs font-medium
              {item.status === 'installed' ? 'bg-nx-text/15 text-nx-text' : item.status === 'available' ? 'bg-nx-text-secondary/15 text-nx-text-secondary' : 'bg-nx-overlay text-nx-text-muted'}">
              {item.status === 'installed' ? t('software.installed') : item.status === 'available' ? t('software.available') : t('software.system')}
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
            {installing && currentItem?.name === item.name ? t('software.processing') : item.action}
          </button>
        </div>
      {/each}
    </div>
    {/if}
  </div>
</div>
