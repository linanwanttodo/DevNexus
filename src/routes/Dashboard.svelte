
<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { t, getVersion, onLangChange } from "../lib/i18n.js";

  let _v = $state(getVersion());
  $effect(() => onLangChange(v => _v = v));

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
    // 每5秒刷新一次资源使用情况
    const interval = setInterval(refreshResourceUsage, 5000);
    return () => clearInterval(interval);
  });

  let stats = $derived([
    { 
      label: "CPU Cores", 
      tkey: "dashboard.cpu_cores",
      value: systemInfo ? systemInfo.cpu_cores.toString() : "--", 
      sub: systemInfo ? systemInfo.cpu_model.split(" ").slice(0, 2).join(" ") : "", 
      subKey: systemInfo ? "" : "dashboard.loading",
      icon: "memory", 
      color: "text-nx-text-secondary" 
    },
    { 
      label: "Memory", 
      tkey: "dashboard.memory",
      value: resourceUsage ? `${resourceUsage.memory_percent.toFixed(0)}%` : "--", 
      sub: resourceUsage ? `${resourceUsage.memory_used_gb}GB / ${resourceUsage.memory_total_gb}GB` : "", 
      subKey: resourceUsage ? "" : "dashboard.loading",
      icon: "sd_card", 
      color: "text-nx-text-secondary" 
    },
    { 
      label: "Disk", 
      tkey: "dashboard.disk",
      value: resourceUsage ? `${resourceUsage.disk_percent.toFixed(0)}%` : "--", 
      sub: resourceUsage ? `${resourceUsage.disk_used_gb}GB / ${resourceUsage.disk_total_gb}GB` : "", 
      subKey: resourceUsage ? "" : "dashboard.loading",
      icon: "hard_drive", 
      color: "text-nx-text-secondary" 
    },
  ]);

  const recentEnvs = [
    { name: "Python", version: "v3.11.4", status: "Running", statusColor: "bg-nx-success" },
    { name: "Node.js", version: "v18.16.0", status: "Stopped", statusColor: "bg-nx-text-muted" },
    { name: "Go", version: "v1.20.5", status: "Running", statusColor: "bg-nx-success" },
  ];
</script>

<div class="mx-auto max-w-5xl">
  <!-- Header -->
  <div class="mb-6 flex items-center justify-between">
    <h1 class="text-xl font-semibold text-nx-text">{_v && t("dashboard.overview")}</h1>
    <button class="flex items-center gap-2 bg-nx-accent px-4 py-2 text-sm font-medium text-white">
      <span class="material-symbols-outlined text-lg">add</span>
      {_v && t("dashboard.new_environment")}
    </button>
  </div>

  <!-- Stats Cards -->
  <div class="mb-6 grid grid-cols-3 gap-4">
    {#each stats as stat}
      <div class="border border-nx-border bg-nx-surface p-4">
        <div class="mb-3 flex items-center justify-between">
          <span class="text-sm text-nx-text-secondary">{_v && t(stat.tkey)}</span>
          <span class="material-symbols-outlined text-nx-text-secondary">{stat.icon}</span>
        </div>
        <div class="text-2xl font-semibold text-nx-text">{stat.value}</div>
            <div class="mt-1 text-xs text-nx-text-muted">{stat.subKey ? (_v && t(stat.subKey)) : stat.sub}</div>
        {#if stat.label === "Memory" && resourceUsage}
          <div class="mt-3 h-1.5 bg-nx-border">
            <div class="h-full bg-nx-warning" style="width: {resourceUsage.memory_percent}%"></div>
          </div>
        {:else if stat.label === "Disk" && resourceUsage}
          <div class="mt-3 h-1.5 bg-nx-border">
            <div class="h-full bg-nx-success" style="width: {resourceUsage.disk_percent}%"></div>
          </div>
        {/if}
      </div>
    {/each}
  </div>

  <!-- Bento Grid -->
  <div class="grid grid-cols-3 gap-4">
    <!-- Recently Used - 2/3 width -->
    <div class="col-span-2 border border-nx-border bg-nx-surface">
      <div class="border-b border-nx-border px-4 py-3">
      <h3 class="text-sm font-medium text-nx-text">{_v && t("dashboard.recently_used")}</h3>
      </div>
      <table class="w-full">
        <thead>
          <tr class="border-b border-nx-border text-xs text-nx-text-muted">
            <th class="px-4 py-2 text-left font-medium">{_v && t("environments.name")}</th>
            <th class="px-4 py-2 text-left font-medium">Version</th>
            <th class="px-4 py-2 text-left font-medium">{_v && t("software.status")}</th>
          </tr>
        </thead>
        <tbody>
          {#each recentEnvs as env}
            <tr class="border-b border-nx-border last:border-0">
              <td class="px-4 py-3 text-sm text-nx-text">{env.name}</td>
              <td class="px-4 py-3 font-mono text-xs text-nx-text-secondary">{env.version}</td>
              <td class="px-4 py-3">
                <span class="inline-flex items-center gap-1.5 text-xs text-nx-text-secondary">
                  <span class="h-1.5 w-1.5 {env.statusColor}"></span>
                  {_v && t(env.status === "Running" ? "dashboard.running" : "dashboard.stopped")}
                </span>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>

    <!-- System Health - 1/3 width -->
    <div class="border border-nx-border bg-nx-surface">
      <div class="border-b border-nx-border px-4 py-3">
      <h3 class="text-sm font-medium text-nx-text">{_v && t("dashboard.system_health")}</h3>
      </div>
      <div class="space-y-4 p-4">
        {#if loading}
          <div class="flex items-center justify-center py-8">
            <span class="material-symbols-outlined animate-spin text-nx-text-muted">progress_activity</span>
          </div>
        {:else if error}
          <div class="text-xs text-nx-text-muted">{error}</div>
        {:else if systemInfo && resourceUsage}
          <div>
            <div class="mb-1 text-xs text-nx-text-muted">{_v && t("dashboard.operating_system")}</div>
            <div class="text-sm text-nx-text">{systemInfo.os_name} {systemInfo.os_version}</div>
            <div class="mt-0.5 text-xs text-nx-text-muted">{_v && t("dashboard.kernel")}: {systemInfo.kernel_version}</div>
          </div>
          <div class="h-px bg-nx-border"></div>
          <div>
            <div class="mb-1 text-xs text-nx-text-muted">{_v && t("dashboard.cpu_usage")}</div>
            <div class="text-sm text-nx-text">{resourceUsage.cpu_usage.toFixed(1)}%</div>
            <div class="mt-2 h-1.5 bg-nx-border">
              <div class="h-full bg-nx-info" style="width: {resourceUsage.cpu_usage}%"></div>
            </div>
          </div>
          <div class="h-px bg-nx-border"></div>
          <div>
            <div class="mb-1 text-xs text-nx-text-muted">{_v && t("dashboard.system_uptime")}</div>
            <div class="text-sm text-nx-text">{formatUptime(resourceUsage.uptime_secs)}</div>
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>
