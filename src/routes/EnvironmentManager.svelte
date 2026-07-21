<script>
  import { invoke } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.svelte.js";
  import { showConfirm } from "../lib/confirm.svelte.js";
  import { t, tFormat } from "../lib/i18n.svelte.js";

  let environments = $state([]);
  let loading = $state(true);
  let error = $state(null);
  let showCreateModal = $state(false);
  let newEnvName = $state("");
  let newEnvPath = $state("");
  let creating = $state(false);

  // 展开状态：key 为 env.name，value 为展开/折叠
  let expanded = $state({});
  // 每个环境的版本列表：key 为 env.name
  let versionsMap = $state({});
  // 正在加载版本
  let loadingVersions = $state({});
  // 正在切换版本
  let switchingVersion = $state({});
  // 正在强制刷新版本
  let refreshing = $state({});
  // 正在全局刷新
  let refreshingAll = $state(false);

  // 支持版本管理的语言类型
  const versionManagedTypes = ["python", "node", "java", "go", "rust", "cpp"];

  // 加载环境列表
  async function loadEnvironments() {
    try {
      loading = true;
      error = null;
      environments = await invoke("list_environments");
    } catch (err) {
      error = err.message || t('common.error');
      console.error("Error loading environments:", err);
    } finally {
      loading = false;
    }
  }

  // 导出环境配置为 JSON
  async function exportEnvironments() {
    try {
      const filePath = await save({
        filters: [{ name: "JSON", extensions: ["json"] }],
        defaultPath: `devnexus-environments-${new Date().toISOString().slice(0, 10)}.json`,
      });
      if (!filePath) return; // 用户取消
      const msg = await invoke("save_export_file", { path: filePath });
      showToast(msg, "success");
    } catch (err) {
      showToast(t('common.error_msg').replace('{error}', err.message || err), "error");
    }
  }

  // 切换展开/折叠
  async function toggleExpand(env) {
    if (!expanded[env.name]) {
      expanded[env.name] = true;
      // 展开时加载版本列表（走缓存，不会卡顿）
      await loadVersions(env);
      // 如果版本数 ≤ 1，自动收起，没必要展示
      const versions = versionsMap[env.name];
      if (!versions || versions.length <= 1) {
        expanded[env.name] = false;
      }
    } else {
      expanded[env.name] = false;
    }
  }

  // 加载指定环境的版本列表
  async function loadVersions(env, forceRefresh = false) {
    loadingVersions[env.name] = true;
    try {
      versionsMap[env.name] = await invoke("list_versions", {
        langType: env.lang_type,
        forceRefresh: forceRefresh || undefined,
      });
    } catch (err) {
      console.error(`Error loading versions for ${env.name}:`, err);
      showToast(t('common.error_msg').replace('{error}', err.message || err));
      versionsMap[env.name] = [];
    } finally {
      loadingVersions[env.name] = false;
    }
  }

  // 强制刷新版本列表（跳过缓存）
  async function refreshVersions(env) {
    refreshing[env.name] = true;
    try {
      await loadVersions(env, true);
      showToast(t('common.all_refreshed'));
    } finally {
      refreshing[env.name] = false;
    }
  }

  // 全局刷新：重新加载环境列表 + 清除所有版本缓存，重新扫描
  async function refreshAll() {
    refreshingAll = true;
    try {
      await loadEnvironments();
      const promises = environments
        .filter(env => versionManagedTypes.includes(env.lang_type) && expanded[env.name])
        .map(env => loadVersions(env, true));
      await Promise.all(promises);
      showToast(t('common.all_refreshed'));
    } finally {
      refreshingAll = false;
    }
  }

  // 切换版本
  async function switchVersion(env, version) {
    if (switchingVersion[env.name]) return;
    switchingVersion[env.name] = true;
    try {
      const result = await invoke("switch_version", {
        langType: env.lang_type,
        version: version.version,
      });
      showToast(result);
      // 重新扫描版本列表（失效缓存后回扫）
      await loadVersions(env, true);
      // 手动修正活跃标记：刚切换的版本就是活跃的
      if (versionsMap[env.name]) {
        versionsMap[env.name] = versionsMap[env.name].map(v => ({
          ...v,
          is_active: v.version === version.version,
        }));
      }
      // 刷新环境列表（更新显示版本号）
      await loadEnvironments();
    } catch (err) {
      showToast(t('common.error_msg').replace('{error}', err.message || err));
    } finally {
      switchingVersion[env.name] = false;
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
      showToast(t('common.error_msg').replace('{error}', err.message || err));
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
      showToast(t('common.error_msg').replace('{error}', err.message || err));
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

  // 创建环境
  async function createEnvironment() {
    if (!newEnvName.trim() || !newEnvPath.trim()) return;
    creating = true;
    try {
      const result = await invoke("add_to_path", { 
        envName: newEnvName.trim(), 
        path: newEnvPath.trim() 
      });
      showToast(result);
      showCreateModal = false;
      newEnvName = "";
      newEnvPath = "";
      await loadEnvironments();
    } catch (err) {
      showToast(t('common.error_msg').replace('{error}', err.message || err));
    } finally {
      creating = false;
    }
  }

  onMount(() => {
    loadEnvironments();
  });
</script>

<div class="mx-auto max-w-5xl p-5">
  <!-- Header -->
  <div class="mb-6 flex items-center justify-between">
    <h1 class="text-xl font-semibold text-nx-text">{t("environments.title")}</h1>
    <div class="flex items-center gap-2">
      <button
        class="nx-btn nx-btn-ghost flex items-center gap-2"
        onclick={refreshAll}
        disabled={refreshingAll}
      >
        <span class="material-symbols-outlined text-lg {refreshingAll ? 'nx-animate-spin' : ''}">
          {refreshingAll ? 'progress_activity' : 'refresh'}
        </span>
        {t("environments.refresh")}
      </button>
      <button
        class="nx-btn nx-btn-ghost flex items-center gap-2"
        onclick={exportEnvironments}
      >
        <span class="material-symbols-outlined text-lg">file_download</span>
        {t("environments.export")}
      </button>
      <button class="nx-btn nx-btn-primary flex items-center gap-2" onclick={() => showCreateModal = true}>
        <span class="material-symbols-outlined text-lg">add</span>
        {t("environments.new")}
      </button>
    </div>
  </div>

  <!-- Environment Table -->
  <div class="nx-section">
    {#if loading}
      <div class="flex items-center justify-center py-12">
        <span class="material-symbols-outlined nx-animate-spin text-nx-text-muted text-3xl">progress_activity</span>
      </div>
    {:else if error}
      <div class="nx-empty">
        <span class="material-symbols-outlined text-nx-text-muted text-3xl">error</span>
        <div class="mt-2 text-sm text-nx-text-secondary">{error}</div>
        <button 
          class="nx-btn nx-btn-primary mt-4"
          onclick={loadEnvironments}>
          {t("common.retry")}
        </button>
      </div>
    {:else if environments.length === 0}
      <div class="nx-empty">
        <span class="material-symbols-outlined text-nx-text-muted text-3xl">inbox</span>
        <div class="mt-2 text-sm text-nx-text-muted">{t("environments.none")}</div>
        <div class="mt-1 text-xs text-nx-text-muted">{t("environments.none_hint")}</div>
      </div>
    {:else}
    <table class="nx-table w-full">
      <thead>
        <tr class="border-b border-nx-border text-xs text-nx-text-muted">
          <th class="w-8 px-2 py-3"></th>
          <th class="px-2 py-3 text-left font-medium">{t("environments.name")}</th>
          <th class="px-2 py-3 text-left font-medium">{t("environments.path")}</th>
          <th class="px-2 py-3 text-left font-medium">{t("environments.status")}</th>
          <th class="px-4 py-3 text-right font-medium">{t("environments.actions")}</th>
        </tr>
      </thead>
      <tbody>
        {#each environments as env}
          <tr class="group border-b border-nx-border last:border-0">
            <td class="w-8 px-2 py-3">
              {#if versionManagedTypes.includes(env.lang_type)}
                <button
                  class="flex items-center justify-center p-0.5 text-nx-text-muted"
                  onclick={() => toggleExpand(env)}
                >
                  <span class="material-symbols-outlined text-base transition-transform duration-200 {expanded[env.name] ? 'rotate-90' : ''}">
                    chevron_right
                  </span>
                </button>
              {/if}
            </td>
            <td class="px-2 py-3">
              <div class="flex items-center gap-2">
                <span class="text-sm font-medium text-nx-text">{env.name}</span>
                <span class="font-mono text-xs text-nx-text-muted">v{env.version}</span>
              </div>
            </td>
            <td class="px-2 py-3 font-mono text-xs text-nx-text-secondary">{env.path}</td>
            <td class="px-2 py-3">
              <span class="nx-pill inline-flex items-center gap-1.5 bg-nx-success/15 px-2 py-0.5 text-xs font-medium text-nx-success">
                <span class="nx-status-dot bg-nx-success"></span>
                {env.status}
              </span>
            </td>
            <td class="px-4 py-3">
              <div class="flex items-center justify-end gap-1">
                <button 
                  class="p-1.5 text-nx-text-secondary" 
                  title={t("environments.add_to_path")}
                  onclick={() => addToPath(env)}>
                  <span class="material-symbols-outlined text-lg">add_road</span>
                </button>
                <button 
                  class="p-1.5 text-nx-text-secondary" 
                  title={t("environments.remove_from_path")}
                  onclick={() => removeFromPath(env)}>
                  <span class="material-symbols-outlined text-lg">remove_road</span>
                </button>
                <button 
                  class="p-1.5 text-nx-text-secondary" 
                  title={t("environments.view_config")}
                  onclick={() => viewConfig(env)}>
                  <span class="material-symbols-outlined text-lg">description</span>
                </button>
              </div>
            </td>
          </tr>
          <!-- 展开的版本列表行 -->
          {#if expanded[env.name]}
            <tr class="border-b border-nx-border last:border-0">
              <td colspan="5" class="bg-nx-bg/50 px-4 py-2">
                <!-- 版本列表头部：标题 + 刷新按钮 -->
                <div class="mb-2 flex items-center justify-between">
                  <span class="text-xs font-medium text-nx-text-muted uppercase tracking-wide">
                    {t("environments.versions")}
                  </span>
                  <button
                    class="nx-btn nx-btn-ghost flex items-center gap-1 px-2 py-0.5 text-xs"
                    onclick={() => refreshVersions(env)}
                    disabled={refreshing[env.name]}
                  >
                    <span class="material-symbols-outlined text-sm {refreshing[env.name] ? 'nx-animate-spin' : ''}">
                      {refreshing[env.name] ? 'progress_activity' : 'refresh'}
                    </span>
                    {t("environments.refresh")}
                  </button>
                </div>
                {#if loadingVersions[env.name]}
                  <div class="flex items-center justify-center py-4">
                    <span class="material-symbols-outlined nx-animate-spin text-nx-text-muted text-xl">progress_activity</span>
                    <span class="ml-2 text-xs text-nx-text-muted">{t('common.loading')}</span>
                  </div>
                {:else if versionsMap[env.name] && versionsMap[env.name].length > 0}
                  <div class="space-y-1">
                    {#each versionsMap[env.name] as ver}
                      <div class="flex items-center justify-between rounded px-3 py-1.5 text-sm {ver.is_active ? 'bg-nx-accent/10 border border-nx-accent/30' : 'hover:bg-nx-bg border border-transparent'}">
                        <div class="flex items-center gap-3">
                          {#if ver.is_active}
                            <span class="material-symbols-outlined text-nx-accent text-base">check_circle</span>
                          {:else}
                            <span class="material-symbols-outlined text-nx-text-muted text-base">radio_button_unchecked</span>
                          {/if}
                          <span class="font-mono text-sm text-nx-text">{ver.version}</span>
                          {#if ver.path}
                            <span class="font-mono text-xs text-nx-text-muted">{ver.path}</span>
                          {/if}
                        </div>
                        <div class="flex items-center gap-2">
                          {#if ver.is_active}
                            <span class="text-xs text-nx-accent font-medium">{t("environments.active")}</span>
                          {:else}
                            <button
                              class="nx-btn nx-btn-primary px-2.5 py-1 text-xs"
                              onclick={() => switchVersion(env, ver)}
                              disabled={switchingVersion[env.name]}
                            >
                              {switchingVersion[env.name] ? '...' : t("environments.switch")}
                            </button>
                          {/if}
                        </div>
                      </div>
                    {/each}
                  </div>
                {:else}
                  <div class="py-3 text-center text-xs text-nx-text-muted">{t("environments.no_versions")}</div>
                {/if}
              </td>
            </tr>
          {/if}
        {/each}
      </tbody>
    </table>

    <!-- Footer -->
    <div class="flex items-center justify-between border-t border-nx-border px-4 py-3">
      <span class="text-xs text-nx-text-muted">{tFormat("environments.count", { count: environments.length })}</span>
      <div class="flex items-center gap-2 text-xs text-nx-text-muted">
        <span>1 of 1</span>
      </div>
    </div>
    {/if}
  </div>
</div>

<!-- Create Environment Modal -->
{#if showCreateModal}
  <!-- eslint-disable-next-line a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="nx-dialog-overlay" role="button" tabindex="-1" onclick={() => showCreateModal = false} onkeydown={(e) => { if (e.key === 'Escape' || e.key === 'Enter') showCreateModal = false; }}>
    <div class="nx-dialog" role="dialog" aria-modal="true" tabindex="-1" onkeydown={(e) => e.stopPropagation()} onclick={(e) => e.stopPropagation()}>
      <div class="nx-dialog-header">
        <h3 class="text-base font-semibold text-nx-text">{t("environments.title")} - {t("environments.new")}</h3>
      </div>
      <div class="nx-dialog-body space-y-2.5">
        <div>
          <label for="envName" class="block text-xs text-nx-text-muted mb-0.5">{t("environments.name")}</label>
          <input id="envName" type="text" bind:value={newEnvName} placeholder="e.g. Node.js v20" class="nx-input h-8 w-full text-xs" />
        </div>
        <div>
          <label for="envPath" class="block text-xs text-nx-text-muted mb-0.5">{t("environments.path")}</label>
          <input id="envPath" type="text" bind:value={newEnvPath} placeholder="e.g. /usr/local/bin" class="nx-input h-8 w-full text-xs" />
        </div>
      </div>
      <div class="nx-dialog-footer">
        <button
          class="nx-btn nx-btn-ghost"
          onclick={() => showCreateModal = false}
        >
          {t("common.cancel")}
        </button>
        <button
          class="nx-btn nx-btn-primary"
          onclick={createEnvironment}
          disabled={!newEnvName.trim() || !newEnvPath.trim() || creating}
        >
          {creating ? "..." : t("environments.new")}
        </button>
      </div>
    </div>
  </div>
{/if}
