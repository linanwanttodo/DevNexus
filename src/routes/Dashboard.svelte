<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { t } from "../lib/i18n.svelte.js";
  import { navigate } from "../lib/stores.svelte.js";

  let systemInfo = $state(null);
  let resourceUsage = $state(null);
  let loading = $state(true);
  let error = $state(null);

  function formatUptime(seconds) {
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    return `${days}d ${hours}h ${minutes}m`;
  }

  async function loadSystemInfo() {
    try {
      loading = true;
      error = null;
      systemInfo = await invoke("get_system_info");
      resourceUsage = await invoke("get_resource_usage");
    } catch (err) {
      error = err.message || "Failed to load system information";
    } finally {
      loading = false;
    }
  }

  async function refreshResourceUsage() {
    try {
      resourceUsage = await invoke("get_resource_usage");
    } catch (_) {}
  }

  function progressColor(val) {
    if (val > 80) return "var(--nx-danger)";
    if (val > 60) return "var(--nx-warning)";
    return "var(--nx-success)";
  }

  onMount(() => {
    loadSystemInfo();
    loadEnvironments();
    const interval = setInterval(refreshResourceUsage, 5000);
    return () => clearInterval(interval);
  });

  let stats = $derived([
    { 
      id: "cpu",
      label: "CPU Cores", 
      tkey: "dashboard.cpu_cores",
      value: systemInfo ? systemInfo.cpu_cores.toString() : "--", 
      sub: systemInfo?.cpu_model ? systemInfo.cpu_model.split(" ").slice(0, 2).join(" ") : "", 
      icon: "memory", 
    },
    { 
      id: "memory",
      label: "Memory", 
      tkey: "dashboard.memory",
      value: resourceUsage ? `${resourceUsage.memory_percent.toFixed(0)}%` : "--", 
      sub: resourceUsage ? `${resourceUsage.memory_used_gb}GB / ${resourceUsage.memory_total_gb}GB` : "", 
      icon: "sd_card", 
    },
    { 
      id: "disk",
      label: "Disk", 
      tkey: "dashboard.disk",
      value: resourceUsage ? `${resourceUsage.disk_percent.toFixed(0)}%` : "--", 
      sub: resourceUsage ? `${resourceUsage.disk_used_gb}GB / ${resourceUsage.disk_total_gb}GB` : "", 
      icon: "hard_drive", 
    },
  ]);

  let environments = $state([]);

  async function loadEnvironments() {
    try {
      environments = await invoke("list_environments");
    } catch (_) {}
  }

  const recentEnvs = $derived(
    environments.slice(0, 5).map(env => ({
      name: env.name,
      version: env.version,
      status: env.status === "Active" ? "Running" : "Stopped",
      statusColor: env.status === "Active" ? "running" : "stopped",
    }))
  );
</script>

<div class="nx-page mx-auto max-w-5xl p-8">
  <!-- Header -->
  <div class="mb-8 flex items-center justify-between">
    <div>
      <h1 class="text-xl font-semibold text-nx-text">{t("dashboard.overview")}</h1>
      <p class="mt-0.5 text-xs text-nx-text-muted">System status at a glance</p>
    </div>
    <button class="nx-btn nx-btn-primary flex items-center gap-2" onclick={() => navigate("/environments")}>
      <span class="material-symbols-outlined text-base">add</span>
      {t("dashboard.new_environment")}
    </button>
  </div>

  <!-- Stats Cards -->
  <div class="mb-8 grid grid-cols-3 gap-4">
    {#each stats as stat}
      <div class="nx-card p-5">
        <div class="mb-2 flex items-center justify-between">
          <span class="text-xs font-medium text-nx-text-secondary uppercase tracking-wider">{t(stat.tkey)}</span>
          <span class="material-symbols-outlined text-base text-nx-text-muted">{stat.icon}</span>
        </div>
        <div class="text-2xl font-semibold text-nx-text tracking-tight">{stat.value}</div>
        <div class="mt-0.5 text-xs text-nx-text-muted">{stat.sub}</div>
        {#if stat.id === "cpu" && resourceUsage}
          <div class="nx-progress mt-3">
            <div class="nx-progress-bar" style="width: {resourceUsage.cpu_usage}%; background: {progressColor(resourceUsage.cpu_usage)}"></div>
          </div>
        {:else if stat.id === "memory" && resourceUsage}
          <div class="nx-progress mt-3">
            <div class="nx-progress-bar" style="width: {resourceUsage.memory_percent}%; background: {progressColor(resourceUsage.memory_percent)}"></div>
          </div>
        {:else if stat.id === "disk" && resourceUsage}
          <div class="nx-progress mt-3">
            <div class="nx-progress-bar" style="width: {resourceUsage.disk_percent}%; background: {progressColor(resourceUsage.disk_percent)}"></div>
          </div>
        {/if}
      </div>
    {/each}
  </div>

  <!-- Bottom Section -->
  <div class="grid grid-cols-3 gap-4">
    <!-- Recently Used -->
    <div class="col-span-2 nx-section">
      <div class="nx-section-header">
        <h3 class="text-sm font-medium text-nx-text">{t("dashboard.recently_used")}</h3>
        <span class="nx-badge">{recentEnvs.length}</span>
      </div>
      {#if recentEnvs.length > 0}
        <div class="overflow-x-auto">
          <table class="nx-table w-full">
            <thead>
              <tr>
                <th class="px-4 py-2.5">{t("environments.name")}</th>
                <th class="px-4 py-2.5">Version</th>
                <th class="px-4 py-2.5">{t("software.status")}</th>
              </tr>
            </thead>
            <tbody>
              {#each recentEnvs as env}
                <tr>
                  <td class="px-4 py-3 text-sm text-nx-text font-medium">{env.name}</td>
                  <td class="px-4 py-3">
                    <code class="nx-code">{env.version}</code>
                  </td>
                  <td class="px-4 py-3">
                    <span class="inline-flex items-center gap-1.5 text-xs text-nx-text-secondary">
                      <span class="nx-status-dot {env.statusColor}"></span>
                      {env.status}
                    </span>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else}
        <div class="nx-empty">
          <span class="material-symbols-outlined text-2xl text-nx-text-muted mb-2">code</span>
          <p class="text-xs text-nx-text-muted">No environments configured</p>
        </div>
      {/if}
    </div>

    <!-- System Health -->
    <div class="nx-section">
      <div class="nx-section-header">
        <h3 class="text-sm font-medium text-nx-text">{t("dashboard.system_health")}</h3>
      </div>
      <div class="p-4 space-y-4">
        {#if loading}
          <div class="flex items-center justify-center py-8">
            <span class="material-symbols-outlined nx-animate-spin text-nx-text-muted">progress_activity</span>
          </div>
        {:else if error}
          <div class="text-xs text-nx-text-muted">{error}</div>
        {:else if systemInfo && resourceUsage}
          <div>
            <div class="mb-0.5 text-[10px] font-medium text-nx-text-muted uppercase tracking-wider">{t("dashboard.operating_system")}</div>
            <div class="text-sm text-nx-text">{systemInfo.os_name} {systemInfo.os_version}</div>
            <div class="mt-0.5 text-xs text-nx-text-muted font-mono">{systemInfo.kernel_version}</div>
          </div>
          <div class="nx-divider"></div>
          <div>
            <div class="mb-0.5 text-[10px] font-medium text-nx-text-muted uppercase tracking-wider">{t("dashboard.cpu_usage")}</div>
            <div class="text-sm text-nx-text tabular-nums">{resourceUsage.cpu_usage.toFixed(1)}%</div>
            <div class="nx-progress mt-2">
              <div class="nx-progress-bar" style="width: {resourceUsage.cpu_usage}%; background: {progressColor(resourceUsage.cpu_usage)}"></div>
            </div>
          </div>
          <div class="nx-divider"></div>
          <div>
            <div class="mb-0.5 text-[10px] font-medium text-nx-text-muted uppercase tracking-wider">{t("dashboard.system_uptime")}</div>
            <div class="text-sm text-nx-text tabular-nums">{formatUptime(resourceUsage.uptime_secs)}</div>
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>
