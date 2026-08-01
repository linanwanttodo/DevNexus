<script>
  import { invoke } from "@tauri-apps/api/core";
  import TableSkeleton from "../components/skeleton/TableSkeleton.svelte";
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.svelte.js";
  import { showConfirm } from "../lib/confirm.svelte.js";
  import { t, tFormat, getLang } from "../lib/i18n.svelte.js";
  import { friendlyError } from "../lib/errors.svelte.js";
  import ProviderForm from "../components/hub/ProviderForm.svelte";

  let activeTab = $state("stats");
  let providers = $state([]);
  let logs = $state([]);
  let stats = $state(null);
  let status = $state({ running: false, port: 3456 });
  let loading = $state(false);
  let error = $state(null);

  let showForm = $state(false);
  let editingId = $state(null);

  // 单一协议选项：同时决定品牌、线协议、端点与认证方式
  const protocolOptions = $derived([
    { id: "openai_chat", label: t("apiHub.protocol.openai_chat.label"), defaultUrl: "https://api.openai.com", endpoint: "/v1/chat/completions", desc: t("apiHub.protocol.openai_chat.desc") },
    { id: "openai_responses", label: t("apiHub.protocol.openai_responses.label"), defaultUrl: "https://api.openai.com", endpoint: "/v1/responses", desc: t("apiHub.protocol.openai_responses.desc") },
    { id: "anthropic", label: t("apiHub.protocol.anthropic.label"), defaultUrl: "https://api.anthropic.com", endpoint: "/v1/messages", desc: t("apiHub.protocol.anthropic.desc") },
  ]);

  onMount(() => {
    loadData();
    const iv = setInterval(loadStats, 15000);
    return () => clearInterval(iv);
  });
  async function loadData() {
    loading = true; error = null;
    try {
      // 并行拉取，避免串行等待造成首屏卡顿
      const [p, l, s, st] = await Promise.all([
        invoke("api_hub_list_providers"),
        invoke("api_hub_get_logs", { limit: 100, offset: 0 }),
        invoke("api_hub_get_usage_stats"),
        invoke("api_hub_status"),
      ]);
      providers = p; logs = l; stats = s; status = st;
    } catch (err) { error = friendlyError(err); }
    finally { loading = false; }
  }
  async function loadStats() {
    // 窗口不可见时跳过轮询，减少后台开销
    if (document.hidden) return;
    try {
      const [s, l] = await Promise.all([
        invoke("api_hub_get_usage_stats"),
        invoke("api_hub_get_logs", { limit: 100, offset: 0 }),
      ]);
      stats = s; logs = l;
    } catch {}
  }

  function beginAdd() {
    editingId = null;
    showForm = true;
  }
  function beginEdit(p) {
    editingId = p.id;
    showForm = true;
  }
  function cancelForm() { showForm = false; editingId = null; }
  async function saveProvider(data, isEdit) {
    try {
      if (isEdit) { await invoke("api_hub_update_provider", { id: data.id, provider: data }); showToast(t("apiHub.toast.updated")); }
      else { await invoke("api_hub_add_provider", { provider: data }); showToast(t("apiHub.toast.added")); }
      showForm = false; editingId = null; providers = await invoke("api_hub_list_providers");
    } catch (err) { showToast(tFormat("apiHub.toast.error", { error: friendlyError(err) }), "error"); }
  }
  async function deleteProvider(id) {
    const p = providers.find(x => x.id === id);
    const ok = await showConfirm(tFormat("apiHub.confirmDelete", { name: p?.name || id }), t("apiHub.deleteProvider"));
    if (!ok) return;
    try {
      await invoke("api_hub_delete_provider", { id });
      showToast(t("apiHub.toast.deleted"));
      providers = await invoke("api_hub_list_providers");
    } catch (err) { showToast(tFormat("apiHub.toast.deleteFailed", { error: friendlyError(err) }), "error"); }
  }

  function protocolName(id) { return protocolOptions.find(p => p.id === id)?.label || id; }

  function fmtTokens(n) { if (!n) return "0"; return new Intl.NumberFormat(getLang(), { notation: "compact", maximumFractionDigits: 1 }).format(n); }
  const LOCALE_TAGS = { zh: "zh-CN", en: "en-US", ru: "ru-RU" };
  function localeTag() { return LOCALE_TAGS[getLang()] || "en-US"; }
  function fmtTime(ts) { return ts ? new Date(ts * 1000).toLocaleTimeString(localeTag()) : "-"; }
  function fmtDate(ts) { return ts ? new Date(ts * 1000).toLocaleDateString(localeTag()) : "-"; }
  function fmtLatency(ms) { return !ms ? "-" : ms < 1000 ? ms+"ms" : (ms/1000).toFixed(1)+"s"; }
  function statusColor(c) { return c >= 200 && c < 300 ? "text-emerald-400" : c >= 400 ? "text-red-400" : "text-yellow-400"; }
  function getChartHours() { return stats?.by_hour ? Object.entries(stats.by_hour).sort((a,b) => Number(a[0]) - Number(b[0])) : []; }
  function getModelEntries() { return stats?.by_model ? Object.entries(stats.by_model).sort((a,b) => Number(b[1]?.requests) - Number(a[1]?.requests)) : []; }
  function heatmapColor(requests, max) { const t = Math.min(requests / Math.max(max, 1), 1); return `oklch(${50+t*35}% ${0.08+t*0.08} ${220})`; }
  function getAlias(p, id) { return p.model_aliases?.[id] || id; }

  // 聚合网关对外暴露的统一端点（按模型名路由并在各 Provider 协议间转换）
  let endpoints = $derived(
    status
      ? [
          `http://localhost:${status.port}/v1/chat/completions`,
          `http://localhost:${status.port}/v1/responses`,
          `http://localhost:${status.port}/v1/messages`,
        ]
      : []
  );

  async function copyEndpoint(url) {
    try {
      await navigator.clipboard.writeText(url);
      showToast(t("apiHub.gateway.copied"));
    } catch {
      showToast(t("apiHub.gateway.copyFailed"), "error");
    }
  }

  const tabs = $derived([
    { id: "stats", label: t("apiHub.tabs.stats"), icon: "bar_chart" },
    { id: "providers", label: t("apiHub.tabs.providers"), icon: "dns" },
    { id: "logs", label: t("apiHub.tabs.logs"), icon: "article" },
  ]);

  const metricCards = $derived(stats ? [
    { icon: "local_fire_department", label: t("apiHub.metrics.tokens"), value: fmtTokens(stats.total_input_tokens + stats.total_output_tokens) },
    { icon: "forum", label: t("apiHub.metrics.requests"), value: fmtTokens(stats.total_requests) },
    { icon: "check_circle", label: t("apiHub.metrics.successRate"), value: stats.total_requests ? `${(100 * (1 - stats.total_errors / stats.total_requests)).toFixed(1)}%` : "——" },
    { icon: "speed", label: t("apiHub.metrics.avgLatency"), value: stats.total_requests ? fmtLatency(stats.avg_latency_ms) : "——" },
  ] : []);
</script>

<div class="nx-page mx-auto max-w-5xl p-6">
  <!-- ════ Header ════ -->
  <div class="mb-6 flex items-start justify-between">
    <div>
      <h1 class="text-xl font-semibold text-nx-text tracking-tight">API Hub</h1>
    </div>
  </div>

  <!-- ════ 聚合网关 (Gateway) ════ -->
  <div class="nx-card p-4 mb-6">
    <div class="flex items-center justify-between gap-3 flex-wrap">
      <div class="flex items-center gap-2 min-w-0">
        <span class="inline-block h-1.5 w-1.5 rounded-full {status?.running ? 'bg-nx-success' : 'bg-nx-text-muted'}"></span>
        <h2 class="text-sm font-medium text-nx-text">{t("apiHub.gateway.title")}</h2>
        <span class="nx-badge" style="background: var(--nx-accent-bg); color: var(--nx-accent);">localhost:{status?.port}</span>
      </div>
      <p class="text-[11px] text-nx-text-muted">{t("apiHub.gateway.desc")}</p>
    </div>

    <div class="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-3">
      {#each endpoints as ep}
        <button
          type="button"
          class="flex items-center gap-2 rounded-md border border-nx-border bg-nx-raised px-3 py-2 text-left text-[11px] font-mono text-nx-text-secondary transition-colors hover:bg-nx-hover hover:text-nx-text cursor-pointer"
          onclick={() => copyEndpoint(ep)}
          title={t("apiHub.gateway.copyTooltip")}
        >
          <span class="material-symbols-outlined text-sm opacity-50 flex-shrink-0">content_copy</span>
          <span class="truncate">{ep}</span>
        </button>
      {/each}
    </div>
  </div>

  <!-- ════ Tabs ════ -->
  <div class="mb-6 flex gap-0 border-b border-nx-border">
    {#each tabs as tab}
      <button
        class="relative flex items-center gap-1.5 px-4 py-2.5 text-sm font-medium transition-colors {activeTab === tab.id ? 'text-nx-accent' : 'text-nx-text-muted hover:text-nx-text'}"
        onclick={() => activeTab = tab.id}
      >
        <span class="material-symbols-outlined text-base">{tab.icon}</span>
        {tab.label}
        {#if activeTab === tab.id}
          <div class="absolute bottom-0 left-0 right-0 h-0.5 bg-nx-accent rounded-full"></div>
        {/if}
      </button>
    {/each}
  </div>

  {#if error}
    <div class="mb-4 nx-card p-3 text-sm text-nx-text-secondary" style="border-left: 3px solid var(--nx-danger);">
      <span class="material-symbols-outlined text-nx-danger text-sm align-middle mr-1">error</span>{error}
    </div>
  {/if}

  {#if loading && !providers.length}
    <TableSkeleton rows={5} />

  <!-- ════════════════════════════════════════════════════
       STATS TAB
       ════════════════════════════════════════════════════ -->
  {:else if activeTab === "stats"}
      {#if stats}
        <!-- Metric cards -->
      <div class="mb-5 grid grid-cols-4 gap-3">
        {#each metricCards as card}
          <div class="nx-card p-4">
            <div class="flex items-center gap-1.5 text-xs text-nx-text-muted mb-2">
              <span class="material-symbols-outlined text-sm opacity-60">{card.icon}</span>
              <span>{card.label}</span>
            </div>
            <div class="text-xl font-semibold text-nx-text tracking-tight truncate">{card.value}</div>
          </div>
        {/each}
      </div>

      <!-- Heatmap -->
      <div class="nx-card p-4 mb-4">
        <div class="flex items-center justify-between mb-4">
          <h3 class="text-sm font-medium text-nx-text">{t("apiHub.heatmap.title")}</h3>
          <div class="flex items-center gap-1.5 text-[10px] text-nx-text-muted">
            <span>{t("apiHub.heatmap.less")}</span>
            {#each [0.15, 0.35, 0.55, 0.75, 0.95] as hmv}
              <span class="inline-block w-3 h-3 rounded-sm" style="background: {heatmapColor(hmv, 1)}"></span>
            {/each}
            <span>{t("apiHub.heatmap.more")}</span>
          </div>
        </div>
        {#if getChartHours().length > 0}
          {@const hours = getChartHours()}
          {@const hmMax = Math.max(...hours.map(h => h[1].requests), 1)}
          <div class="grid grid-cols-12 gap-1.5">
            {#each hours as [ts, hd]}
              <div
                class="h-4 w-full rounded-sm transition-colors hover:brightness-110 cursor-pointer"
                style="background: {heatmapColor(hd.requests, hmMax)}"
                title={t("apiHub.heatmap.requestTitle").replace("{date}", fmtDate(Number(ts))).replace("{count}", hd.requests)}
              ></div>
            {/each}
          </div>
        {:else}
          <div class="py-8 text-center text-xs text-nx-text-muted">{t("apiHub.empty.noData")}</div>
        {/if}
      </div>

      <!-- Model Usage -->
      <div class="nx-card p-4">
        <h3 class="text-sm font-medium text-nx-text mb-4">{t("apiHub.models.usageRanking")}</h3>
        {#if getModelEntries().length > 0}
          {@const models = getModelEntries()}
          {@const mr = models[0][1].requests}
          <div class="space-y-2">
            {#each models.slice(0, 15) as [model, md], i}
              <div class="flex items-center gap-3 py-1.5">
                <span class="w-5 text-right text-[11px] text-nx-text-muted tabular-nums">{i + 1}</span>
                <div class="w-36 truncate text-xs text-nx-text-secondary font-mono" title={model}>{model}</div>
                <div class="flex-1 h-3 rounded-full bg-nx-bg overflow-hidden">
                  <div
                    class="h-full rounded-full transition-all duration-1000 ease-out"
                    style="width: {(md.requests / mr) * 100}%; background: var(--nx-accent);"
                  ></div>
                </div>
                <div class="w-24 text-right text-[11px] text-nx-text-muted tabular-nums">{fmtTokens(md.input_tokens + md.output_tokens)} {t("apiHub.models.tokens")}</div>
                <div class="w-16 text-right text-[11px] text-nx-text-muted tabular-nums">{md.requests} {t("apiHub.models.requestsSuffix")}</div>
              </div>
            {/each}
            {#if models.length > 15}
              <div class="pt-1 text-center text-[11px] text-nx-text-muted/60">{tFormat("apiHub.models.onlyTop15", { count: models.length })}</div>
            {/if}
          </div>
        {:else}
          <div class="py-8 text-center text-xs text-nx-text-muted">{t("apiHub.empty.noData")}</div>
        {/if}
      </div>
    {:else}
      <div class="nx-card p-10 text-center">
        <span class="material-symbols-outlined text-2xl text-nx-text-muted/40 mb-2">bar_chart</span>
        <div class="text-sm text-nx-text-muted">{t("apiHub.empty.waiting")}</div>
      </div>
    {/if}

  <!-- ════════════════════════════════════════════════════
       PROVIDERS TAB
       ════════════════════════════════════════════════════ -->
  {:else if activeTab === "providers"}
    <!-- Toolbar -->
    <div class="mb-4 flex items-center justify-between">
      <span class="text-xs text-nx-text-muted uppercase tracking-wider">{tFormat("apiHub.providerCount", { count: providers.length })}</span>
      {#if !showForm}
        <button class="nx-btn nx-btn-primary flex items-center gap-1.5 px-3 py-1.5 text-xs" onclick={beginAdd}>
          <span class="material-symbols-outlined text-sm">add</span>
          {t("apiHub.addProvider")}
        </button>
      {/if}
    </div>

    <!-- ════ Add/Edit Form ════ -->
    {#if showForm}
      {@const editingProvider = providers.find(p => p.id === editingId) || null}
      <ProviderForm
        mode={editingId ? "edit" : "add"}
        title={editingId ? t("apiHub.editProvider") : t("apiHub.addProvider")}
        subtitle={editingProvider?.name || ""}
        initial={editingProvider}
        {protocolOptions}
        onSave={saveProvider}
        onCancel={cancelForm}
      />
    {/if}

    <!-- ════ Provider List ════ -->
    {#each providers as p}
      {@const isEditing = showForm && editingId === p.id}
      <div class="nx-card mb-3 overflow-hidden">
        {#if isEditing}
          <ProviderForm mode="edit" title={t("apiHub.editProvider")} subtitle={p.name} initial={p} {protocolOptions} onSave={saveProvider} onCancel={cancelForm} />
        {:else}
          <!-- Provider card -->
          <div class="flex items-start justify-between p-4">
            <div class="flex items-start gap-3 min-w-0 flex-1">
              <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-nx-accent-bg">
                <span class="material-symbols-outlined text-nx-accent text-lg">dns</span>
              </div>
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2 flex-wrap">
                  <span class="text-sm font-medium text-nx-text">{p.name}</span>
                  <span class="nx-badge" style="background: var(--nx-accent-bg); color: var(--nx-accent);">
                    {protocolName(p.protocol)}
                  </span>
                  <span class="flex items-center gap-1 text-[10px] {p.enabled ? 'text-nx-success' : 'text-nx-text-muted'}">
                    <span class="inline-block h-1.5 w-1.5 rounded-full {p.enabled ? 'bg-nx-success' : 'bg-nx-text-muted'}"></span>
                    {p.enabled ? t("apiHub.status.active") : t("apiHub.status.disabled")}
                  </span>
                </div>
                <div class="mt-1 text-[11px] text-nx-text-muted truncate max-w-lg font-mono">{p.base_url}</div>
                <div class="mt-2 flex flex-wrap gap-1">
                  {#each p.models.slice(0, 8) as m}
                    <span class="nx-pill text-[10px]">{getAlias(p, m)}</span>
                  {/each}
                  {#if p.models.length > 8}
                    <span class="text-[10px] text-nx-text-muted self-center ml-0.5">+{p.models.length - 8}</span>
                  {/if}
                </div>
              </div>
            </div>
            <div class="flex shrink-0 gap-1 ml-3">
              <button class="nx-btn nx-btn-ghost p-1.5" onclick={() => beginEdit(p)} title={t("apiHub.edit")}>
                <span class="material-symbols-outlined text-base">edit</span>
              </button>
              <button class="nx-btn nx-btn-ghost p-1.5" style="color: var(--nx-danger);" onclick={() => deleteProvider(p.id)} title={t("apiHub.delete")}>
                <span class="material-symbols-outlined text-base">delete</span>
              </button>
            </div>
          </div>
        {/if}
      </div>
    {/each}

    <!-- Empty state -->
    {#if providers.length === 0 && !showForm}
      <div class="nx-card p-10 text-center border-dashed" style="border-color: var(--nx-border-light);">
        <span class="material-symbols-outlined text-3xl text-nx-text-muted/40">dns</span>
        <div class="mt-2 text-sm text-nx-text-muted">{t("apiHub.empty.noProviders")}</div>
        <p class="mt-1 text-xs text-nx-text-muted/60">{t("apiHub.empty.addHint")}</p>
        <button class="nx-btn nx-btn-primary mt-4 px-4 py-2 text-xs" onclick={beginAdd}>
          <span class="material-symbols-outlined text-sm">add</span>
          {t("apiHub.addFirstProvider")}
        </button>
      </div>
    {/if}

  <!-- ════════════════════════════════════════════════════
       LOGS TAB
       ════════════════════════════════════════════════════ -->
  {:else if activeTab === "logs"}
    <div class="nx-card overflow-hidden">
      <div class="max-h-[500px] overflow-y-auto">
        <table class="nx-table w-full">
          <thead>
            <tr>
              <th class="px-3 py-2.5">{t("apiHub.logs.time")}</th>
              <th class="px-3 py-2.5">{t("apiHub.logs.model")}</th>
              <th class="px-3 py-2.5">{t("apiHub.logs.provider")}</th>
              <th class="px-3 py-2.5 text-right">{t("apiHub.logs.tokens")}</th>
              <th class="px-3 py-2.5 text-right">{t("apiHub.logs.latency")}</th>
              <th class="px-3 py-2.5 text-center">{t("apiHub.logs.status")}</th>
            </tr>
          </thead>
          <tbody>
            {#each logs as log}
              <tr>
                <td class="px-3 py-2.5 whitespace-nowrap font-mono text-[11px] text-nx-text-muted">{fmtTime(log.timestamp)}</td>
                <td class="px-3 py-2.5 font-mono text-xs font-medium text-nx-text">
                  {log.model}
                  {#if log.is_streaming}<span class="material-symbols-outlined align-middle text-[12px] text-nx-text-muted/60 ml-0.5" title={t("apiHub.logs.streaming")}>water_drop</span>{/if}
                </td>
                <td class="px-3 py-2.5 text-xs text-nx-text-muted">{log.provider_name}</td>
                <td class="px-3 py-2.5 text-right text-xs text-nx-text-muted tabular-nums">
                  <span class="text-nx-text-secondary">↑{fmtTokens(log.input_tokens)}</span>
                  <span class="mx-0.5 opacity-30">/</span>
                  <span class="text-nx-text-secondary">↓{fmtTokens(log.output_tokens)}</span>
                </td>
                <td class="px-3 py-2.5 text-right text-xs text-nx-text-muted tabular-nums">{fmtLatency(log.latency_ms)}</td>
                <td class="px-3 py-2.5 text-center">
                  <span class="inline-flex items-center gap-1 text-xs {statusColor(log.status_code)}" title={log.error_message || ""}>
                    <span class="inline-block h-1.5 w-1.5 rounded-full" style="background: currentColor"></span>
                    {log.status_code || "—"}
                  </span>
                </td>
              </tr>
            {:else}
              <tr>
                <td colspan="6" class="px-3 py-12 text-center">
                  <span class="material-symbols-outlined text-xl text-nx-text-muted/30 mb-1">article</span>
                  <div class="text-xs text-nx-text-muted/50">{t("apiHub.logs.empty")}</div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  {/if}
</div>
