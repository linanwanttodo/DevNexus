<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, onDestroy } from "svelte";
  import { showToast } from "../lib/toast.svelte.js";
  import { showConfirm } from "../lib/confirm.svelte.js";
  import { getSearchQuery, setSearchQuery } from "../lib/stores.svelte.js";
  import { t } from "../lib/i18n.svelte.js";

  let groups = $state([]);
  let total = $state(0);
  let loading = $state(true);
  let error = $state(null);
  let search = $derived(getSearchQuery());
  let sortBy = $state("memory"); // memory | cpu | name | count
  let sortAsc = $state(false);
  let autoRefresh = $state(false);
  let refreshInterval = $state(null);
  let killing = $state(null); // PID being killed
  let expanded = $state(new Set());
  let loadInProgress = false;

  async function loadProcesses() {
    if (loadInProgress) return;
    loadInProgress = true;
    try {
      loading = true;
      error = null;
      const result = await invoke("list_processes");
      groups = result.groups;
      total = result.total;
    } catch (err) {
      error = err.message || "Failed to list processes";
    } finally {
      loading = false;
      loadInProgress = false;
    }
  }

  async function killProcess(pid) {
    if (!(await showConfirm(t("process.kill_force_confirm").replace("{pid}", pid)))) return;
    killing = pid;
    try {
      const msg = await invoke("kill_process_force", { pid });
      showToast(msg, "success");
      await loadProcesses();
    } catch (err) {
      showToast(`${t("process.kill_failed")}: ${err.message || err}`, "error");
    } finally {
      killing = null;
    }
  }

  async function killGroup(name, count) {
    if (!(await showConfirm(t("process.kill_confirm").replace("{name}", name).replace("{count}", count)))) return;
    try {
      // 找出该组所有 PID，逐个终止
      const group = groups.find(g => g.name === name);
      if (!group) return;
      let killErrors = [];
      for (const entry of group.entries) {
        try {
          await invoke("kill_process_force", { pid: entry.pid });
        } catch {
          killErrors.push(entry.pid);
        }
      }
      if (killErrors.length > 0) {
        showToast(t('common.error_msg').replace('{error}', `PID(s): ${killErrors.join(", ")}`), "warning");
      } else {
        showToast(t("process.kill_success").replace("{name}", name), "success");
      }
      await loadProcesses();
    } catch (err) {
      showToast(`${t("process.kill_failed")}: ${err.message || err}`, "error");
    }
  }

  async function killPortAction(port) {
    if (!(await showConfirm(t("ports.kill_confirm").replace("{port}", port)))) return;
    try {
      const result = await invoke("kill_port", { port });
      showToast(result, "success");
      await loadProcesses();
    } catch (err) {
      showToast(`${t("process.kill_failed")}: ${err.message || err}`, "error");
    }
  }

  function toggleExpand(name) {
    const next = new Set(expanded);
    if (next.has(name)) {
      next.delete(name);
    } else {
      next.add(name);
    }
    expanded = next;
  }

  function toggleSort(field) {
    if (sortBy === field) {
      sortAsc = !sortAsc;
    } else {
      sortBy = field;
      sortAsc = field === "name";
    }
  }

  function toggleAutoRefresh() {
    autoRefresh = !autoRefresh;
    if (autoRefresh) {
      refreshInterval = setInterval(loadProcesses, 3000);
    } else if (refreshInterval) {
      clearInterval(refreshInterval);
      refreshInterval = null;
    }
  }

  function formatTime(secs) {
    if (!secs) return "—";
    const now = Date.now() / 1000;
    const diff = now - secs;
    if (diff < 60) return `${Math.floor(diff)}s`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h`;
    return `${Math.floor(diff / 86400)}d`;
  }

  function formatMemory(bytes) {
    if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(1)} GB`;
    if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(1)} MB`;
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  let filtered = $derived(
    search.trim()
      ? groups.filter(g =>
          g.name.toLowerCase().includes(search.toLowerCase()) ||
          g.ports.some(p => p.toString().includes(search)) ||
          g.entries.some(e => e.pid.toString().includes(search))
        )
      : groups
  );

  let sorted = $derived.by(() => {
    const arr = [...filtered];
    const cmp = (a, b) => {
      let va, vb;
      switch (sortBy) {
        case "memory": va = a.total_memory_bytes; vb = b.total_memory_bytes; break;
        case "cpu": va = a.total_cpu; vb = b.total_cpu; break;
        case "name": va = a.name; vb = b.name; break;
        case "count": va = a.count; vb = b.count; break;
        default: va = a.total_memory_bytes; vb = b.total_memory_bytes;
      }
      if (typeof va === "string") return sortAsc ? va.localeCompare(vb) : vb.localeCompare(va);
      return sortAsc ? va - vb : vb - va;
    };
    return arr.sort(cmp);
  });

  onMount(() => {
    loadProcesses();
  });

  onDestroy(() => {
    if (refreshInterval) clearInterval(refreshInterval);
  });
</script>

<div class="mx-auto max-w-5xl p-5">
  <!-- 标题栏 -->
  <div class="mb-6 flex items-center justify-between">
    <div>
      <h1 class="text-xl font-semibold text-nx-text">{t("process.title")}</h1>
      <p class="mt-1 text-xs text-nx-text-muted">{t("process.desc")}</p>
    </div>
    <div class="flex items-center gap-2">
      <button
        class="nx-btn nx-btn-ghost flex items-center gap-1.5 px-3 py-1.5 text-xs"
        onclick={toggleAutoRefresh}
      >
        <span class="material-symbols-outlined text-sm {autoRefresh ? 'text-nx-accent' : ''}">
          {autoRefresh ? 'pause' : 'play_arrow'}
        </span>
        {t("process.auto_refresh")}
      </button>
      <button
        class="nx-btn nx-btn-ghost flex items-center gap-2"
        onclick={loadProcesses}
        disabled={loading}
      >
        <span class="material-symbols-outlined text-lg {loading ? 'nx-animate-spin' : ''}">refresh</span>
        {t("process.refresh")}
      </button>
    </div>
  </div>

  <!-- 搜索栏 + 排序 -->
  <div class="mb-4 flex items-center gap-3">
    <div class="nx-search flex-1">
      <span class="nx-search-icon material-symbols-outlined">search</span>
      <input
        class="nx-input"
        type="text"
        placeholder={t("process.search_placeholder")}
        value={search}
        oninput={(e) => { setSearchQuery(e.currentTarget.value); }}
      />
      {#if search}
        <button
          class="nx-search-clear material-symbols-outlined"
          onclick={() => { setSearchQuery(""); }}
        >
          close
        </button>
      {/if}
    </div>
    <select
      bind:value={sortBy}
      class="nx-input px-3 py-2 text-sm"
    >
      <option value="memory">{t("process.sort_memory")}</option>
      <option value="cpu">{t("process.sort_cpu")}</option>
      <option value="count">{t("process.sort_count")}</option>
      <option value="name">{t("process.sort_name")}</option>
    </select>
    <button
      class="nx-btn nx-btn-ghost flex items-center gap-1 px-2 py-2 text-xs"
      onclick={() => { sortAsc = !sortAsc; }}
    >
      <span class="material-symbols-outlined text-sm">
        {sortAsc ? 'arrow_upward' : 'arrow_downward'}
      </span>
    </button>
  </div>

  <!-- 进程表格 -->
  <div class="nx-section">
    {#if loading && groups.length === 0}
      <div class="flex items-center justify-center py-12">
        <span class="material-symbols-outlined nx-animate-spin text-nx-text-muted text-3xl">progress_activity</span>
      </div>
    {:else if error}
      <div class="nx-empty">
        <span class="material-symbols-outlined text-nx-danger text-3xl">error</span>
        <div class="mt-2 text-sm text-nx-danger">{error}</div>
        <button class="nx-btn nx-btn-primary mt-4" onclick={loadProcesses}>{t("common.retry")}</button>
      </div>
    {:else if filtered.length === 0}
      <div class="nx-empty p-12">
        <span class="material-symbols-outlined text-nx-text-muted text-4xl">memory</span>
        <div class="mt-4 text-sm text-nx-text-muted">
          {search ? t("process.no_matching") : t("process.no_processes")}
        </div>
      </div>
    {:else}
      <table class="nx-table w-full" style="table-layout: fixed;">
        <colgroup>
          <col class="w-8" />
          <col style="width: 22%;" />
          <col style="width: 6%;" />
          <col style="width: 7%;" />
          <col style="width: 10%;" />
          <col style="width: 14%;" />
          <col style="width: 7%;" />
          <col style="width: 14%;" />
        </colgroup>
        <thead>
          <tr class="text-xs text-nx-text-muted">
            <th></th>
            <th class="text-left font-medium">{t("process.name")}</th>
            <th class="text-right font-medium">{t("process.instances")}</th>
            <th class="text-right font-medium">CPU</th>
            <th class="text-right font-medium">{t("process.memory")}</th>
            <th class="text-left font-medium">{t("process.ports")}</th>
            <th class="text-right font-medium">{t("process.elapsed")}</th>
            <th class="text-right font-medium">{t("process.actions")}</th>
          </tr>
        </thead>
        <tbody>
          {#each sorted as group}
            <!-- 分组行 -->
            <tr
              class="border-b border-nx-border cursor-pointer hover:bg-nx-surface/80 transition-colors"
              onclick={() => toggleExpand(group.name)}
            >
              <td class="px-2 py-3 text-center">
                <span class="material-symbols-outlined text-sm text-nx-text-muted transition-transform {expanded.has(group.name) ? 'rotate-90' : ''}">
                  chevron_right
                </span>
              </td>
              <td class="px-4 py-3">
                <div class="flex items-center gap-2">
                  <span class="font-medium text-sm text-nx-text">{group.name}</span>
                  {#if group.count > 1}
                    <span class="text-xs text-nx-text-muted bg-nx-bg px-1.5 py-0.5">
                      ×{group.count}
                    </span>
                  {/if}
                </div>
              </td>
              <td class="px-4 py-3 text-right text-sm text-nx-text-secondary">{group.count}</td>
              <td class="px-4 py-3 text-right">
                <span class="font-mono text-sm {group.total_cpu > 50 ? 'text-nx-danger' : group.total_cpu > 20 ? 'text-nx-warning' : 'text-nx-text-secondary'}">
                  {group.total_cpu.toFixed(1)}%
                </span>
              </td>
              <td class="px-4 py-3 text-right">
                <span class="font-mono text-sm text-nx-text-secondary" title="{t('process.memory_total')} {formatMemory(group.total_memory_bytes)}">
                  {formatMemory(Math.round(group.total_memory_bytes / group.count))}
                </span>
              </td>
              <td class="px-4 py-3">
                {#if group.ports.length > 0}
                  <div class="flex flex-wrap gap-1">
                    {#each group.ports as port}
                      <span class="nx-pill inline-flex items-center gap-0.5 font-mono text-xs text-nx-accent bg-nx-accent/10 px-1.5 py-0.5">
                        {port}
                        <button class="text-nx-danger hover:text-nx-danger/80 leading-none" title={t("ports.kill")}
                          onclick={(e) => { e.stopPropagation(); killPortAction(port); }}>
                          ×
                        </button>
                      </span>
                    {/each}
                  </div>
                {:else}
                  <span class="text-xs text-nx-text-muted">—</span>
                {/if}
              </td>
              <td class="px-4 py-3 text-right">
                <span class="text-xs text-nx-text-muted">{formatTime(group.earliest_start)}</span>
              </td>
              <td class="px-4 py-3 text-right">
                <button
                  class="nx-btn text-xs text-nx-danger hover:bg-nx-danger/10 px-2.5 py-1"
                  onclick={(e) => { e.stopPropagation(); killGroup(group.name, group.count); }}
                >
                  {t("process.kill_all")}
                </button>
              </td>
            </tr>

            <!-- 展开行：子进程列表 -->
            {#if expanded.has(group.name)}
              {#each group.entries as entry}
                <tr class="bg-nx-bg/30">
                  <td class="w-8"></td>
                  <td class="pl-8">
                    <span class="font-mono text-xs text-nx-text-muted">PID {entry.pid}</span>
                  </td>
                  <td class="text-right text-xs text-nx-text-muted">1</td>
                  <td class="text-right">
                    <span class="font-mono text-xs text-nx-text-muted">{entry.cpu_usage.toFixed(1)}%</span>
                  </td>
                  <td class="text-right">
                    <span class="font-mono text-xs text-nx-text-muted">{formatMemory(entry.memory_bytes)}</span>
                  </td>
                  <td class="text-xs text-nx-text-muted">—</td>
                  <td class="text-right">
                    <span class="text-xs text-nx-text-muted">{formatTime(entry.start_time_secs)}</span>
                  </td>
                  <td class="text-right">
                    <button
                      class="nx-btn text-xs text-nx-danger hover:bg-nx-danger/10 px-2 py-1 disabled:opacity-30"
                      onclick={(e) => { e.stopPropagation(); killProcess(entry.pid); }}
                      disabled={killing === entry.pid}
                    >
                      {killing === entry.pid ? '...' : t("process.kill_force")}
                    </button>
                  </td>
                </tr>
              {/each}
            {/if}
          {/each}
        </tbody>
      </table>

      <!-- 底部统计 -->
      <div class="flex items-center justify-between border-t border-nx-border px-4 py-2">
        <span class="text-xs text-nx-text-muted">
          {filtered.length} {t("process.groups")} · {total} {t("process.total_processes")}
        </span>
        <button class="nx-btn nx-btn-ghost text-xs" onclick={loadProcesses}>{t("process.refresh")}</button>
      </div>
    {/if}
  </div>
</div>
