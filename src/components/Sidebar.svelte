<script>
  import { onMount } from "svelte";
  import BrandIcons from "../icons/BrandIcons.svelte";
  import ContainerIcons from "../icons/ContainerIcons.svelte";
  import { getRoute, navigate } from "../lib/stores.svelte.js";
  import { t } from "../lib/i18n.svelte.js";
  import { getVersion } from "@tauri-apps/api/app";
  import { invoke } from "@tauri-apps/api/core";

  let appVersion = $state("1.1.1");
  let resourceUsage = $state(null);

  onMount(() => {
    (async () => {
      try {
        appVersion = await getVersion();
      } catch {
        // non-Tauri env fallback
      }
      loadResourceUsage();
    })();
    const interval = setInterval(loadResourceUsage, 10000);
    return () => clearInterval(interval);
  });

  async function loadResourceUsage() {
    try {
      resourceUsage = await invoke("get_resource_usage");
    } catch {
      // silently fail
    }
  }

  let navItems = $derived.by(() => {
    return [
      { route: "/dashboard", label: t("nav.dashboard"), icon: "dashboard" },
      { route: "/environments", label: t("nav.environments"), icon: "code" },
      { route: "/migration", label: t("nav.migration"), icon: "swap_horiz" },
      { route: "/software", label: t("nav.software"), icon: "apps" },
      { route: "/containers", label: t("nav.containers"), icon: "container" },
      { route: "/mirrors", label: t("nav.mirrors"), icon: "sync" },
      { route: "/processes", label: t("nav.processes"), icon: "lan" },
      { route: "/passwords", label: t("nav.passwords"), icon: "key" },
      { route: "/cookies", label: t("nav.cookies"), icon: "cookie" },
      { route: "/uninstall", label: t("nav.uninstall"), icon: "delete" },
      { route: "/api-hub", label: t("nav.api_hub"), icon: "hub" },
      { route: "/settings", label: t("nav.settings"), icon: "settings" },
    ];
  });

  let currentRoute = $derived(getRoute());

  let cpuPercent = $derived(resourceUsage ? resourceUsage.cpu_usage.toFixed(0) : null);
  let memPercent = $derived(resourceUsage ? resourceUsage.memory_percent.toFixed(0) : null);
  let memBar = $derived(resourceUsage ? Math.min(resourceUsage.memory_percent, 100) : 0);
  let cpuBar = $derived(resourceUsage ? Math.min(resourceUsage.cpu_usage, 100) : 0);

  function handleClick(route) {
    navigate(route);
  }
</script>

<aside
  class="flex h-full w-60 flex-shrink-0 flex-col border-r border-nx-border"
  aria-label="Main navigation"
  style="background: var(--color-sidebar);"
>
  <!-- Logo area -->
  <div class="flex h-11 items-center gap-2.5 border-b border-nx-border px-4">
    <span class="material-symbols-outlined text-nx-accent text-xl">terminal</span>
    <span class="text-sm font-semibold tracking-tight text-nx-text">DevNexus</span>
  </div>

  <!-- Navigation -->
  <nav class="flex-1 overflow-y-auto py-2 px-2">
    <ul class="flex flex-col gap-0.5">
      {#each navItems as item (item.route)}
        <li>
          <button
            type="button"
            role="tab"
            aria-current={currentRoute === item.route ? "page" : undefined}
            class="group relative flex w-full items-center gap-2.5 px-3 py-2 text-sm cursor-pointer rounded-md transition-all duration-150
              {currentRoute === item.route
                ? 'bg-nx-selected text-nx-text font-medium'
                : 'text-nx-text-secondary hover:text-nx-text hover:bg-nx-hover'}"
            onclick={() => handleClick(item.route)}
          >
            <!-- Active indicator bar -->
            {#if currentRoute === item.route}
              <div class="absolute left-0 top-1/2 -translate-y-1/2 w-0.5 h-4 rounded-r-full" style="background: var(--color-brand);"></div>
            {/if}
            <!-- Icon -->
            {#if item.route === '/cookies'}
              <BrandIcons name="cookie" size={18} class="flex-shrink-0 opacity-60 group-hover:opacity-90 transition-opacity" />
            {:else if item.route === '/containers'}
              <ContainerIcons name="docker-logo" size={18} class="flex-shrink-0 opacity-60 group-hover:opacity-90 transition-opacity" />
            {:else}
              <span class="material-symbols-outlined text-lg flex-shrink-0 opacity-50 group-hover:opacity-80 transition-all duration-150">{item.icon}</span>
            {/if}
            <span class="truncate">{item.label}</span>
          </button>
        </li>
      {/each}
    </ul>
  </nav>

  <!-- Status Bar -->
  {#if resourceUsage}
    <div class="border-t border-nx-border px-3 py-2.5 space-y-1.5">
      <div class="flex items-center gap-2 text-[10px] text-nx-text-muted">
        <span class="w-6">CPU</span>
        <div class="flex-1 h-1 bg-nx-border rounded-full overflow-hidden">
          <div class="h-full rounded-full transition-all duration-1000 ease-out" style="width: {cpuBar}%; background: var(--nx-accent); opacity: 0.7;"></div>
        </div>
        <span class="w-7 text-right font-mono">{cpuPercent}%</span>
      </div>
      <div class="flex items-center gap-2 text-[10px] text-nx-text-muted">
        <span class="w-6">MEM</span>
        <div class="flex-1 h-1 bg-nx-border rounded-full overflow-hidden">
          <div class="h-full rounded-full transition-all duration-1000 ease-out" style="width: {memBar}%; background: var(--nx-success); opacity: 0.7;"></div>
        </div>
        <span class="w-7 text-right font-mono">{memPercent}%</span>
      </div>
    </div>
  {/if}

  <!-- Version + GitHub -->
  <div class="border-t border-nx-border px-4 py-2.5">
    <div class="flex items-center gap-2">
      <a
        href="https://github.com/linanwanttodo/DevNexus"
        target="_blank"
        rel="noopener noreferrer"
        class="flex items-center gap-1 text-nx-text-muted hover:text-nx-text transition-colors"
        title="GitHub"
      >
        <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
          <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/>
        </svg>
      </a>
      <span class="text-[11px] text-nx-text-muted">v{appVersion}</span>
    </div>
  </div>
</aside>
