<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.js";
  import { showConfirm } from "../lib/confirm.js";
  import { t, getVersion, onLangChange } from "../lib/i18n.js";

  let _v = $state(getVersion());
  $effect(() => onLangChange(v => _v = v));

  let environments = $state([]);
  let loading = $state(true);
  let error = $state(null);

  // 加载环境列表
  async function loadEnvironments() {
    try {
      loading = true;
      error = null;
      environments = await invoke("list_environments");
    } catch (err) {
      error = err.message || "Failed to load environments";
      console.error("Error loading environments:", err);
    } finally {
      loading = false;
    }
  }

  // 添加到 PATH
  async function addToPath(env) {
    try {
      const result = await invoke("add_to_path", { 
        envName: env.name, 
        path: env.path 
      });
      showToast(result);
      await loadEnvironments();
    } catch (err) {
      showToast(`Error: ${err.message || err}`);
    }
  }

  // 从 PATH 移除
  async function removeFromPath(env) {
    if (!await showConfirm(`Remove ${env.name} from PATH?`)) return;
    
    try {
      const result = await invoke("remove_from_path", { 
        envName: env.name, 
        path: env.path 
      });
      showToast(result);
      await loadEnvironments();
    } catch (err) {
      showToast(`Error: ${err.message || err}`);
    }
  }

  // 查看配置文件
  function viewConfig(env) {
    if (env.shell_config) {
      showToast(`Configuration file: ${env.shell_config}`);
    } else {
      showToast("No configuration file found");
    }
  }

  onMount(() => {
    loadEnvironments();
  });
</script>

<div class="mx-auto max-w-5xl">
  <!-- Header -->
  <div class="mb-6 flex items-center justify-between">
    <h1 class="text-xl font-semibold text-nx-text">{_v && t("environments.title")}</h1>
    <button class="flex items-center gap-2 bg-nx-accent px-4 py-2 text-sm font-medium text-white">
      <span class="material-symbols-outlined text-lg">add</span>
      {_v && t("environments.new")}
    </button>
  </div>

  <!-- Environment Table -->
  <div class="border border-nx-border bg-nx-surface">
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
          onclick={loadEnvironments}>
          {_v && t("common.retry")}
        </button>
      </div>
    {:else if environments.length === 0}
      <div class="p-6 text-center">
        <span class="material-symbols-outlined text-nx-text-muted text-3xl">inbox</span>
        <div class="mt-2 text-sm text-nx-text-muted">{_v && t("environments.none")}</div>
        <div class="mt-1 text-xs text-nx-text-muted">{_v && t("environments.none_hint")}</div>
      </div>
    {:else}
    <table class="w-full">
      <thead>
        <tr class="border-b border-nx-border text-xs text-nx-text-muted">
          <th class="px-4 py-3 text-left font-medium">{_v && t("environments.name")}</th>
          <th class="px-4 py-3 text-left font-medium">{_v && t("environments.path")}</th>
          <th class="px-4 py-3 text-left font-medium">{_v && t("software.status")}</th>
          <th class="px-4 py-3 text-right font-medium">{_v && t("port_manager.actions")}</th>
        </tr>
      </thead>
      <tbody>
        {#each environments as env}
          <tr class="group border-b border-nx-border last:border-0">
            <td class="px-4 py-3">
              <div class="flex items-center gap-2">
                <span class="text-sm font-medium text-nx-text">{env.name}</span>
                <span class="font-mono text-xs text-nx-text-muted">v{env.version}</span>
              </div>
            </td>
            <td class="px-4 py-3 font-mono text-xs text-nx-text-secondary">{env.path}</td>
            <td class="px-4 py-3">
              <span class="inline-flex items-center gap-1.5 bg-nx-success/15 px-2 py-0.5 text-xs font-medium text-nx-success">
                <span class="h-1.5 w-1.5 bg-nx-success"></span>
                {env.status}
              </span>
            </td>
            <td class="px-4 py-3">
              <div class="flex items-center justify-end gap-1">
                <button 
                  class="p-1.5 text-nx-text-secondary" 
                  title={_v ? t("environments.add_to_path") : "Add to PATH"}
                  onclick={() => addToPath(env)}>
                  <span class="material-symbols-outlined text-lg">add_road</span>
                </button>
                <button 
                  class="p-1.5 text-nx-text-secondary" 
                  title={_v ? t("environments.remove_from_path") : "Remove from PATH"}
                  onclick={() => removeFromPath(env)}>
                  <span class="material-symbols-outlined text-lg">remove_road</span>
                </button>
                <button 
                  class="p-1.5 text-nx-text-secondary" 
                  title={_v ? t("environments.view_config") : "View Config"}
                  onclick={() => viewConfig(env)}>
                  <span class="material-symbols-outlined text-lg">description</span>
                </button>
              </div>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>

    <!-- Footer -->
    <div class="flex items-center justify-between border-t border-nx-border px-4 py-3">
      <span class="text-xs text-nx-text-muted">{_v && t("environments.count", { count: environments.length })}</span>
      <div class="flex items-center gap-2 text-xs text-nx-text-muted">
        <span>1 of 1</span>
      </div>
    </div>
    {/if}
  </div>
</div>
