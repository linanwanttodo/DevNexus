<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { t } from "../lib/i18n.svelte.js";
  import { navigate } from "../lib/stores.svelte.js";

  let systemInfo = $state(null);
  let resourceUsage = $state(null);
  let loading = $state(true);
  let error = $state(null);

  // 格式化运行时间
  function formatUptime(seconds) {
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    return `${days}d ${hours}h ${minutes}m`;
  }

  // 加载系统信息
  async function loadSystemInfo() {
    try {
      loading = true;
      error = null;
      systemInfo = await invoke("get_system_info");
      resourceUsage = await invoke("get_resource_usage");
    } catch (err) {
      error = err.message || "Failed to load system information";
      console.error("Error loading system info:", err);
    } finally {
      loading = false;
    }
  }

  // 定时刷新资源使用情况（每5秒）
  async function refreshResourceUsage() {
    try {
      resourceUsage = await invoke("get_resource_usage");
    } catch (err) {
      console.error("Error refreshing resource usage:", err);
    }
  }

  onMount(() => {
    loadSystemInfo();
    loadEnvironments();
    // 每5秒刷新一次资源使用情况
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
      subKey: systemInfo ? "" : "dashboard.loading",
      icon: "memory", 
      color: "text-nx-text-secondary" 
    },
    { 
      id: "memory",
      label: "Memory", 
      tkey: "dashboard.memory",
      value: resourceUsage ? `${resourceUsage.memory_percent.toFixed(0)}%` : "--", 
      sub: resourceUsage ? `${resourceUsage.memory_used_gb}GB / ${resourceUsage.memory_total_gb}GB` : "", 
      subKey: resourceUsage ? "" : "dashboard.loading",
      icon: "sd_card", 
      color: "text-nx-text-secondary" 
    },
    { 
      id: "disk",
      label: "Disk", 
      tkey: "dashboard.disk",
      value: resourceUsage ? `${resourceUsage.disk_percent.toFixed(0)}%` : "--", 
      sub: resourceUsage ? `${resourceUsage.disk_used_gb}GB / ${resourceUsage.disk_total_gb}GB` : "", 
      subKey: resourceUsage ? "" : "dashboard.loading",
      icon: "hard_drive", 
      color: "text-nx-text-secondary" 
    },
  ]);

  // 加载环境列表
  let environments = $state([]);

  async function loadEnvironments() {
    try {
      environments = await invoke("list_environments");
    } catch (err) {
      console.error("Error loading environments:", err);
    }
  }

  const recentEnvs = $derived(
    environments.slice(0, 5).map(env => ({
      name: env.name,
      version: env.version,
      status: env.status === "Active" ? "Running" : "Stopped",
      statusColor: env.status === "Active" ? "bg-nx-success" : "bg-nx-text-muted",
    }))
  );
</script>

<div class="mx-auto max-w-5xl p-8">
  <!-- Header -->
  <div class="mb-8 flex items-center justify-between">
    <h1 class="text-2xl font-semibold text-nx-text">{t("dashboard.overview")}</h1>
    <button class="nx-btn nx-btn-primary flex items-center gap-2" onclick={() => navigate("/environments")}>
      <span class="material-symbols-outlined text-lg">add</span>
      {t("dashboard.new_environment")}
    </button>
  </div>

  <!-- Stats Cards -->
  <div class="mb-6 grid grid-cols-3 gap-5">
    {#each stats as stat}
      <div class="nx-card p-5">
        <div class="mb-3 flex items-center justify-between">
          <span class="text-sm text-nx-text-secondary">{t(stat.tkey)}</span>
          <span class="material-symbols-outlined text-nx-text-secondary">{stat.icon}</span>
        </div>
        <div class="text-2xl font-semibold text-nx-text">{stat.value}</div>
            <div class="mt-1 text-xs text-nx-text-muted">{stat.subKey ? t(stat.subKey) : stat.sub}</div>
        {#if stat.id === "memory" && resourceUsage}
          <div class="nx-progress mt-3">
            <div class="nx-progress-bar warning" style="width: {resourceUsage.memory_percent}%"></div>
          </div>
        {:else if stat.id === "disk" && resourceUsage}
          <div class="nx-progress mt-3">
            <div class="nx-progress-bar success" style="width: {resourceUsage.disk_percent}%"></div>
          </div>
        {/if}
      </div>
    {/each}
  </div>

  <!-- Bento Grid -->
  <div class="grid grid-cols-3 gap-5">
    <!-- Recently Used - 2/3 width -->
    <div class="col-span-2 nx-section">
      <div class="nx-section-header">
        <h3 class="text-sm font-medium text-nx-text">{t("dashboard.recently_used")}</h3>
      </div>
      <table class="nx-table w-full">
        <thead>
          <tr class="text-xs text-nx-text-muted">
            <th class="px-4 py-2 text-left font-medium">{t("environments.name")}</th>
            <th class="px-4 py-2 text-left font-medium">Version</th>
            <th class="px-4 py-2 text-left font-medium">{t("software.status")}</th>
          </tr>
        </thead>
        <tbody>
          {#each recentEnvs as env}
            <tr>
              <td class="px-4 py-3 text-sm text-nx-text">{env.name}</td>
              <td class="px-4 py-3 font-mono text-xs text-nx-text-secondary">{env.version}</td>
              <td class="px-4 py-3">
                <span class="inline-flex items-center gap-1.5 text-xs text-nx-text-secondary">
                  <span class="nx-status-dot {env.statusColor}"></span>
                  {t(env.status === "Running" ? "dashboard.running" : "dashboard.stopped")}
                </span>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>

    <!-- System Health - 1/3 width -->
    <div class="nx-section">
      <div class="nx-section-header">
        <h3 class="text-sm font-medium text-nx-text">{t("dashboard.system_health")}</h3>
      </div>
      <div class="space-y-5 px-5 py-4">
        {#if loading}
          <div class="flex items-center justify-center py-8">
            <span class="material-symbols-outlined nx-animate-spin text-nx-text-muted">progress_activity</span>
          </div>
        {:else if error}
          <div class="text-xs text-nx-text-muted">{error}</div>
        {:else if systemInfo && resourceUsage}
          <div>
            <div class="mb-1 text-xs text-nx-text-muted">{t("dashboard.operating_system")}</div>
            <div class="text-sm text-nx-text">{systemInfo.os_name} {systemInfo.os_version}</div>
            <div class="mt-0.5 text-xs text-nx-text-muted">{t("dashboard.kernel")}: {systemInfo.kernel_version}</div>
          </div>
          <div class="nx-divider"></div>
          <div>
            <div class="mb-1 text-xs text-nx-text-muted">{t("dashboard.cpu_usage")}</div>
            <div class="text-sm text-nx-text">{resourceUsage.cpu_usage.toFixed(1)}%</div>
            <div class="nx-progress mt-2">
              <div class="nx-progress-bar info" style="width: {resourceUsage.cpu_usage}%"></div>
            </div>
          </div>
          <div class="nx-divider"></div>
          <div>
            <div class="mb-1 text-xs text-nx-text-muted">{t("dashboard.system_uptime")}</div>
            <div class="text-sm text-nx-text">{formatUptime(resourceUsage.uptime_secs)}</div>
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>
