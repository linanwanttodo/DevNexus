<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.svelte.js";
  import { showConfirm } from "../lib/confirm.svelte.js";
  import { navigate } from "../lib/stores.svelte.js";
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
  let copiedCommand = $state("");

  const categories = [
    { id: "all", label: t("software.all") },
    { id: "ide", label: t("software.ide") },
    { id: "database", label: t("software.database") },
    { id: "cli", label: t("software.cli") },
    { id: "runtime", label: t("software.runtime") },
    { id: "cli-code", label: t("software.cli_code") },
  ];

  const cliCodeTools = [
    { name: "Claude Code", publisher: "Anthropic", command: "npm install -g @anthropic-ai/claude-code@latest", desc: "AI coding assistant by Anthropic" },
    { name: "Gemini Code", publisher: "Google", command: "npm install -g gemini-code@latest", desc: "AI coding assistant by Google" },
    { name: "OpenCode", publisher: "OpenCode", command: "npm install -g opencode-ai@latest", desc: "Open-source AI coding assistant" },
    { name: "Qoder Code", publisher: "Qoder", command: "npm install -g @qoder-ai/qodercli@latest", desc: "AI-powered coding assistant" },
    { name: "Reasonix", publisher: "Reasonix", command: "npm install -g reasonix@latest", desc: "DeepSeek-native coding agent" },
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
      'IntelliJ IDEA Community':'intellij',
      'Sublime Text':'sublime',
      'Zed':'zed',
      'Postman':'postman',
      'MySQL Workbench':'mysql',
      'TablePlus':'tableplus',
      'GParted':'gparted',
      'Homebrew':'homebrew',
      'curl':'swap_vert',
      'wget':'download',
      'OpenSSH Client':'lock',
      'GCC':'code',
      'Clang':'code',
      'CMake':'build',
      'htop':'monitoring',
      'tmux':'terminal',
      'ripgrep':'search',
      'fd':'folder_open',
      'jq':'data_object',
      'fzf':'filter_list',
    };
    return map[sw] || 'default';
  }

  async function copyCommand(command, name) {
    try {
      await navigator.clipboard.writeText(command);
      copiedCommand = name;
      showToast(`${command}`, "success");
      setTimeout(() => { copiedCommand = ""; }, 2000);
    } catch {
      showToast(t('common.copy_failed'), "error");
    }
  }

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

  async function handleAction(item) {
    if (!item.package_name) {
      showToast(t('common.no_package_name'));
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
        showToast(t('common.install_failed').replace('{error}', err.message || err));
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
        showToast(t('common.uninstall_failed').replace('{error}', err.message || err));
      } finally {
        installing = false;
        currentItem = null;
      }
    } else if (item.action === "Open") {
      showToast(t('common.opening').replace('{name}', item.name));
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

<div class="flex h-full flex-col">
  <!-- Header with back button -->
  <div class="flex items-center gap-2 border-b border-nx-border px-5 py-2.5">
    <button class="nx-back-btn" onclick={() => navigate("/dashboard")}>
      <span class="material-symbols-outlined text-lg">arrow_back</span>
      {t("nav.dashboard")}
    </button>
    <span class="text-xs text-nx-text-muted">/</span>
    <h1 class="text-sm font-medium text-nx-text">{t("software.title")}</h1>
  </div>

  <!-- Category pills + filters bar -->
  <div class="flex items-center justify-between border-b border-nx-border px-5 py-2.5">
    <div class="flex items-center gap-1 flex-wrap">
      {#each categories as cat}
        <button
          class="px-3 py-1.5 text-xs font-medium rounded-md transition-colors
            {selectedCategory === cat.id
              ? 'bg-nx-accent-bg text-nx-accent'
              : 'text-nx-text-secondary hover:text-nx-text hover:bg-nx-hover'}"
          onclick={() => selectedCategory = cat.id}>
          {cat.label}
        </button>
      {/each}
    </div>

    <div class="flex items-center gap-3">
      <label class="flex items-center gap-1.5 text-xs text-nx-text-muted cursor-pointer select-none">
        <input type="checkbox" bind:checked={filterInstalled} class="rounded border-nx-border bg-nx-bg" />
        {t("software.installed_filter")}
      </label>
      <label class="flex items-center gap-1.5 text-xs text-nx-text-muted cursor-pointer select-none">
        <input type="checkbox" bind:checked={filterUpdates} class="rounded border-nx-border bg-nx-bg" />
        {t("software.updates_filter")}
      </label>
      <button class="nx-btn nx-btn-ghost px-2 py-1 text-xs" onclick={loadSoftware}>
        <span class="material-symbols-outlined text-sm">refresh</span>
      </button>
    </div>
  </div>

  <!-- Content area -->
  <div class="flex-1 overflow-y-auto p-5">
    {#if selectedCategory === "cli-code"}
      <!-- CLI Code tools -->
      <div class="grid grid-cols-2 gap-3 xl:grid-cols-3">
        {#each cliCodeTools as tool}
          <div class="nx-card p-4 flex flex-col">
            <div class="mb-3 flex items-start justify-between">
              <div class="flex h-9 w-9 items-center justify-center rounded-lg bg-nx-hover">
                <span class="material-symbols-outlined text-nx-text-secondary text-lg">terminal</span>
              </div>
            </div>
            <h3 class="mb-0.5 text-sm font-medium text-nx-text">{tool.name}</h3>
            <p class="mb-0.5 text-xs text-nx-text-muted">{tool.publisher}</p>
            <p class="mb-3 text-xs text-nx-text-secondary flex-1 leading-relaxed">{tool.desc}</p>
            <div class="flex items-center gap-2 rounded-lg border border-nx-border bg-nx-bg px-3 py-2 font-mono text-xs text-nx-text-secondary">
              <span class="flex-1 truncate" title={tool.command}>{tool.command}</span>
              <button
                class="flex-shrink-0 px-2 py-1 text-xs font-medium rounded transition-colors {copiedCommand === tool.name ? 'bg-nx-success-bg text-nx-success' : 'text-nx-text-muted hover:text-nx-text-secondary hover:bg-nx-hover'}"
                onclick={() => copyCommand(tool.command, tool.name)}>
                <span class="material-symbols-outlined text-sm align-middle">
                  {copiedCommand === tool.name ? 'check' : 'content_copy'}
                </span>
              </button>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <!-- Regular software -->
      {#if loading}
        <div class="flex items-center justify-center py-16">
          <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
        </div>
      {:else if !hasPackageManager && !pmChecking}
        <div class="nx-card p-8 text-center">
          <span class="material-symbols-outlined text-nx-text-muted text-4xl">package_2</span>
          <h2 class="mt-4 text-base font-semibold text-nx-text">{t("software.no_pm_title")}</h2>
          <p class="mt-2 max-w-md mx-auto text-sm text-nx-text-secondary leading-relaxed">{t("software.no_pm_desc")}</p>
          <div class="mt-6 flex flex-wrap justify-center gap-4">
            {#if packageManagers.length > 0}
              {#each packageManagers as pm}
                <span class="nx-pill">{pm.name}</span>
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
        <div class="flex flex-col items-center justify-center py-16">
          <span class="material-symbols-outlined text-nx-text-muted text-3xl">error</span>
          <div class="mt-2 text-sm text-nx-text-secondary">{error}</div>
          <button class="nx-btn nx-btn-primary mt-4" onclick={loadSoftware}>
            Retry
          </button>
        </div>
      {:else if filteredSoftware.length === 0}
        <div class="flex flex-col items-center justify-center py-16">
          <span class="material-symbols-outlined text-nx-text-muted text-3xl">search_off</span>
          <div class="mt-2 text-sm text-nx-text-muted">{t("software.none")}</div>
          <div class="mt-1 text-xs text-nx-text-muted">{t("software.none_hint")}</div>
        </div>
      {:else}
        <div class="grid grid-cols-2 gap-3 xl:grid-cols-3">
          {#each filteredSoftware as item}
            <div class="nx-card p-4 flex flex-col">
              <div class="mb-3 flex items-start justify-between">
                <div class="flex h-9 w-9 items-center justify-center rounded-lg bg-nx-hover">
                  <BrandIcons name={brandIconName(item.name)} size={20} />
                </div>
                <span class="nx-pill text-[10px] {item.status === 'installed' ? 'text-nx-success bg-nx-success-bg' : item.status === 'available' ? '' : ''}">
                  {item.status === 'installed' ? t('software.installed') : item.status === 'available' ? t('software.available') : t('software.system')}
                </span>
              </div>
              <h3 class="mb-1 text-sm font-medium text-nx-text">{item.name}</h3>
              <p class="mb-4 font-mono text-xs text-nx-text-muted">{item.version}</p>
              <button
                class="nx-btn w-full text-xs disabled:opacity-50 disabled:cursor-not-allowed
                  {item.action === 'Install'
                    ? 'nx-btn-primary'
                    : item.action === 'Uninstall'
                      ? 'nx-btn-danger'
                      : item.action === 'System Managed'
                        ? 'opacity-40 cursor-not-allowed'
                        : 'nx-btn-ghost'}"
                disabled={item.action === 'System Managed' || installing}
                onclick={() => handleAction(item)}>
                {installing && currentItem?.name === item.name ? t('software.processing') : item.action}
              </button>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</div>
