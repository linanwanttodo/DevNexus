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

  async function loadProcesses() {
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
        showToast(`Failed to kill PID(s): ${killErrors.join(", ")}`, "warning");
      } else {
        showToast(t("process.kill_success").replace("{name}", name), "success");
      }
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
          g.entries.some(e => e.pid.toString().includes(search))
        )
      : groups
  );

  let sorted = $derived(() => {
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

<div class="mx-auto max-w-5xl">
  <!-- 标题栏 -->
  <div class="mb-6 flex items-center justify-between">
    <div>
      <h1 class="text-xl font-semibold text-nx-text">{t("process.title")}</h1>
      <p class="mt-1 text-xs text-nx-text-muted">{t("process.desc")}</p>
    </div>
    <div class="flex items-center gap-2">
      <button
        class="flex items-center gap-1.5 border border-nx-border px-3 py-1.5 text-xs font-medium text-nx-text-secondary"
        onclick={toggleAutoRefresh}
      >
        <span class="material-symbols-outlined text-sm {autoRefresh ? 'text-nx-accent' : ''}">
          {autoRefresh ? 'pause' : 'play_arrow'}
        </span>
        {t("process.auto_refresh")}
      </button>
      <button
        class="flex items-center gap-2 border border-nx-border px-4 py-2 text-sm font-medium text-nx-text-secondary disabled:opacity-40"
        onclick={loadProcesses}
        disabled={loading}
      >
        <span class="material-symbols-outlined text-lg {loading ? 'animate-spin' : ''}">refresh</span>
        {t("process.refresh")}
      </button>
    </div>
  </div>

  <!-- 搜索栏 + 排序 -->
  <div class="mb-4 flex items-center gap-3">
    <div class="relative flex-1">
      <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-nx-text-muted">search</span>
      <input
        type="text"
        placeholder={t("process.search_placeholder")}
        value={search}
        oninput={(e) => { setSearchQuery(e.currentTarget.value); }}
        class="w-full border border-nx-border bg-nx-surface px-10 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-accent"
      />
      {#if search}
        <button
          class="absolute right-3 top-1/2 -translate-y-1/2 text-nx-text-muted"
          onclick={() => { setSearchQuery(""); }}
        >
          <span class="material-symbols-outlined text-sm">close</span>
        </button>
      {/if}
    </div>
    <select
      bind:value={sortBy}
      class="border border-nx-border bg-nx-surface px-3 py-2 text-sm text-nx-text-secondary outline-none focus:border-nx-accent"
    >
      <option value="memory">{t("process.sort_memory")}</option>
      <option value="cpu">{t("process.sort_cpu")}</option>
      <option value="count">{t("process.sort_count")}</option>
      <option value="name">{t("process.sort_name")}</option>
    </select>
    <button
      class="flex items-center gap-1 border border-nx-border px-2 py-2 text-xs text-nx-text-secondary"
      onclick={() => { sortAsc = !sortAsc; }}
    >
      <span class="material-symbols-outlined text-sm">
        {sortAsc ? 'arrow_upward' : 'arrow_downward'}
      </span>
    </button>
  </div>

  <!-- 进程表格 -->
  <div class="border border-nx-border bg-nx-surface">
    {#if loading && groups.length === 0}
      <div class="flex items-center justify-center py-12">
        <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
      </div>
    {:else if error}
      <div class="p-6 text-center">
        <span class="material-symbols-outlined text-nx-danger text-3xl">error</span>
        <div class="mt-2 text-sm text-nx-danger">{error}</div>
        <button class="mt-4 bg-nx-accent px-4 py-2 text-sm font-medium text-white" onclick={loadProcesses}>{t("common.retry")}</button>
      </div>
    {:else if filtered.length === 0}
      <div class="p-12 text-center">
        <span class="material-symbols-outlined text-nx-text-muted text-4xl">memory</span>
        <div class="mt-4 text-sm text-nx-text-muted">
          {search ? t("process.no_matching") : t("process.no_processes")}
        </div>
      </div>
    {:else}
      <table class="w-full">
        <thead>
          <tr class="border-b border-nx-border text-xs text-nx-text-muted">
            <th class="w-8"></th>
            <th class="px-4 py-3 text-left font-medium">{t("process.name")}</th>
            <th class="px-4 py-3 text-right font-medium w-16">{t("process.instances")}</th>
            <th class="px-4 py-3 text-right font-medium w-20">CPU</th>
            <th class="px-4 py-3 text-right font-medium w-24">{t("process.memory")}</th>
            <th class="px-4 py-3 text-right font-medium w-20">{t("process.elapsed")}</th>
            <th class="px-4 py-3 text-right font-medium w-20">{t("process.actions")}</th>
          </tr>
        </thead>
        <tbody>
          {#each sorted() as group}
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
              <td class="px-4 py-3 text-right">
                <span class="text-xs text-nx-text-muted">{formatTime(group.earliest_start)}</span>
              </td>
              <td class="px-4 py-3 text-right">
                <button
                  class="px-2 py-1 text-xs font-medium text-nx-danger border border-nx-border hover:bg-nx-danger/10"
                  onclick={(e) => { e.stopPropagation(); killGroup(group.name, group.count); }}
                >
                  {t("process.kill_all")}
                </button>
              </td>
            </tr>

            <!-- 展开行：子进程列表 -->
            {#if expanded.has(group.name)}
              {#each group.entries as entry}
                <tr class="border-b border-nx-border/50 bg-nx-bg/50">
                  <td class="w-8"></td>
                  <td class="px-4 py-2 pl-8">
                    <span class="font-mono text-xs text-nx-text-muted">PID {entry.pid}</span>
                  </td>
                  <td class="px-4 py-2 text-right text-xs text-nx-text-muted">1</td>
                  <td class="px-4 py-2 text-right">
                    <span class="font-mono text-xs text-nx-text-muted">{entry.cpu_usage.toFixed(1)}%</span>
                  </td>
                  <td class="px-4 py-2 text-right">
                    <span class="font-mono text-xs text-nx-text-muted">{formatMemory(entry.memory_bytes)}</span>
                  </td>
                  <td class="px-4 py-2 text-right">
                    <span class="text-xs text-nx-text-muted">{formatTime(entry.start_time_secs)}</span>
                  </td>
                  <td class="px-4 py-2 text-right">
                    <button
                      class="px-2 py-1 text-xs font-medium text-nx-danger border border-nx-border hover:bg-nx-danger/10 disabled:opacity-30"
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
        <button class="text-xs text-nx-text-muted" onclick={loadProcesses}>{t("process.refresh")}</button>
      </div>
    {/if}
  </div>
</div>
