<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.svelte.js";

  let activeTab = $state("stats");
  let providers = $state([]);
  let logs = $state([]);
  let stats = $state(null);
  let status = $state({ running: false, port: 3456 });
  let loading = $state(false);
  let error = $state(null);

  let showForm = $state(false);
  let editingId = $state(null);
  let form = $state({ name: "", protocol: "openai_chat", base_url: "", api_key: "", model_aliases: {} });
  let fetchingModels = $state(false);
  let fetchedModels = $state([]);
  let selectedModels = $state({});
  let manualModelId = $state("");
  let manualModelName = $state("");

  // 单一协议选项：同时决定品牌、线协议、端点与认证方式
  const protocolOptions = [
    { id: "openai_chat", label: "OpenAI · Chat Completions", defaultUrl: "https://api.openai.com", endpoint: "/v1/chat/completions", desc: "通用 OpenAI 格式，兼容大多数工具" },
    { id: "openai_responses", label: "OpenAI · Responses", defaultUrl: "https://api.openai.com", endpoint: "/v1/responses", desc: "OpenAI 新格式，Codex/OpenCode 使用" },
    { id: "anthropic", label: "Anthropic · Messages", defaultUrl: "https://api.anthropic.com", endpoint: "/v1/messages", desc: "Claude 原生格式，Anthropic SDK 直接使用" },
    { id: "gemini", label: "Google Gemini", defaultUrl: "https://generativelanguage.googleapis.com", endpoint: "/v1/models/{model}:generateContent", desc: "Gemini generateContent 端点" },
    { id: "ollama", label: "Ollama (Local)", defaultUrl: "http://localhost:11434", endpoint: "/v1/chat/completions", desc: "本地 OpenAI 兼容" },
  ];

  onMount(() => {
    loadData();
    const iv = setInterval(loadStats, 15000);
    return () => clearInterval(iv);
  });
  async function loadData() {
    loading = true; error = null;
    try {
      providers = await invoke("api_hub_list_providers");
      logs = await invoke("api_hub_get_logs", { limit: 100, offset: 0 });
      stats = await invoke("api_hub_get_usage_stats");
      status = await invoke("api_hub_status");
    } catch (err) { error = err.message || String(err); }
    finally { loading = false; }
  }
  async function loadStats() { try { stats = await invoke("api_hub_get_usage_stats"); logs = await invoke("api_hub_get_logs", { limit: 100, offset: 0 }); } catch {} }

  function beginAdd() {
    editingId = null;
    form = { name: "", protocol: "openai_chat", base_url: "https://api.openai.com", api_key: "", model_aliases: {} };
    fetchedModels = []; selectedModels = {}; showForm = true;
  }
  function beginEdit(p) {
    editingId = p.id;
    form = { name: p.name, protocol: p.protocol || "openai_chat", base_url: p.base_url, api_key: p.api_key, model_aliases: { ...(p.model_aliases || {}) } };
    fetchedModels = []; selectedModels = {};
    p.models.forEach(m => selectedModels[m] = true);
    showForm = true;
  }
  function cancelForm() { showForm = false; editingId = null; fetchedModels = []; selectedModels = {}; }
  async function saveForm() {
    try {
      const models = Object.keys(selectedModels).filter(m => selectedModels[m]);
      if (models.length === 0) { showToast("请至少选择一个模型", "error"); return; }
      const model_aliases = {};
      models.forEach(m => { model_aliases[m] = form.model_aliases[m] || m; });
      const data = {
        id: editingId || crypto.randomUUID(),
        name: form.name, protocol: form.protocol,
        base_url: form.base_url, api_key: form.api_key,
        models, model_aliases, enabled: true, created_at: Math.floor(Date.now() / 1000),
      };
      if (editingId) { await invoke("api_hub_update_provider", { id: editingId, provider: data }); showToast("已更新"); }
      else { await invoke("api_hub_add_provider", { provider: data }); showToast("已添加"); }
      showForm = false; editingId = null; providers = await invoke("api_hub_list_providers");
    } catch (err) { showToast(`错误: ${err.message}`, "error"); }
  }
  async function deleteProvider(id) { await invoke("api_hub_delete_provider", { id }); showToast("已删除"); providers = await invoke("api_hub_list_providers"); }

  async function fetchModels() {
    if (!form.base_url || !form.protocol) { showToast("请先填写 Base URL 和协议", "error"); return; }
    fetchingModels = true; fetchedModels = [];
    try {
      fetchedModels = await invoke("api_hub_fetch_models", { baseUrl: form.base_url, apiKey: form.api_key || "", protocol: form.protocol });
      fetchedModels.forEach(m => {
        if (!(m.id in selectedModels)) { selectedModels[m.id] = true; form.model_aliases[m.id] = m.name || m.id; }
      });
      showToast(`获取到 ${fetchedModels.length} 个模型`);
    } catch (err) { showToast(`获取失败: ${err.message}`, "error"); }
    finally { fetchingModels = false; }
  }
  function toggleModel(id) { selectedModels[id] = !selectedModels[id]; }
  function addManualModel() {
    const id = manualModelId.trim();
    if (!id) return;
    if (fetchedModels.find(m => m.id === id)) { showToast(`模型 ${id} 已存在`, "error"); return; }
    const model = { id, name: manualModelName.trim() || id, owned_by: "自定义", enabled: true };
    fetchedModels = [...fetchedModels, model];
    selectedModels[id] = true;
    form.model_aliases[id] = manualModelName.trim() || id;
    manualModelId = ""; manualModelName = "";
    showToast(`已添加模型 ${id}`);
  }
  function selectAll() { fetchedModels.forEach(m => selectedModels[m.id] = true); }
  function deselectAll() { fetchedModels.forEach(m => selectedModels[m.id] = false); }
  function protocolName(id) { return protocolOptions.find(p => p.id === id)?.label || id; }
  function onProtocolChange() {
    const t = protocolOptions.find(p => p.id === form.protocol);
    if (t && !editingId) form.base_url = t.defaultUrl;
  }

  function fmtTokens(n) { if (!n) return "0"; if (n >= 1e8) return (n/1e8).toFixed(1)+"亿"; if (n >= 1e4) return (n/1e4).toFixed(1)+"万"; if (n >= 1e3) return (n/1e3).toFixed(1)+"K"; return String(n); }
  function fmtTime(ts) { return ts ? new Date(ts*1000).toLocaleTimeString() : "-"; }
  function fmtDate(ts) { return ts ? new Date(ts*1000).toLocaleDateString("zh-CN") : "-"; }
  function fmtLatency(ms) { return !ms ? "-" : ms < 1000 ? ms+"ms" : (ms/1000).toFixed(1)+"s"; }
  function statusColor(c) { return c >= 200 && c < 300 ? "text-emerald-400" : c >= 400 ? "text-red-400" : "text-yellow-400"; }
  function getChartHours() { return stats?.by_hour ? Object.entries(stats.by_hour).sort((a,b) => Number(a[0]) - Number(b[0])) : []; }
  function getModelEntries() { return stats?.by_model ? Object.entries(stats.by_model).sort((a,b) => Number(b[1]?.requests) - Number(a[1]?.requests)) : []; }
  function heatmapColor(requests, max) { const t = Math.min(requests / Math.max(max, 1), 1); return `oklch(${50+t*35}% ${0.08+t*0.08} ${220})`; }
  function getAlias(p, id) { return p.model_aliases?.[id] || id; }
  function selectedCount() { return Object.values(selectedModels).filter(Boolean).length; }

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
      showToast("已复制网关端点");
    } catch {
      showToast("复制失败", "error");
    }
  }

  const tabs = [
    { id: "stats", label: "使用统计", icon: "bar_chart" },
    { id: "providers", label: "Provider", icon: "dns" },
    { id: "logs", label: "请求日志", icon: "article" },
  ];

  const metricCards = $derived(stats ? [
    { icon: "local_fire_department", label: "Tokens 用量", value: fmtTokens(stats.total_input_tokens + stats.total_output_tokens) },
    { icon: "forum", label: "总请求数", value: String(stats.total_requests) },
    { icon: "calendar_month", label: "活跃时段", value: String(Object.keys(stats.by_hour || {}).length) },
    { icon: "model_training", label: "最常用模型", value: getModelEntries()[0]?.[0] || "——" },
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
        <h2 class="text-sm font-medium text-nx-text">聚合网关</h2>
        <span class="nx-badge" style="background: var(--nx-accent-bg); color: var(--nx-accent);">localhost:{status?.port}</span>
      </div>
      <p class="text-[11px] text-nx-text-muted">统一入口 · 按模型名路由并在 OpenAI / Anthropic / Gemini 间转换</p>
    </div>

    <div class="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-3">
      {#each endpoints as ep}
        <button
          type="button"
          class="flex items-center gap-2 rounded-md border border-nx-border bg-nx-raised px-3 py-2 text-left text-[11px] font-mono text-nx-text-secondary transition-colors hover:bg-nx-hover hover:text-nx-text cursor-pointer"
          onclick={() => copyEndpoint(ep)}
          title="点击复制端点"
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
    <div class="flex items-center justify-center py-20">
      <span class="material-symbols-outlined nx-animate-spin text-2xl text-nx-text-muted">progress_activity</span>
    </div>

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
          <h3 class="text-sm font-medium text-nx-text">活跃热力图</h3>
          <div class="flex items-center gap-1.5 text-[10px] text-nx-text-muted">
            <span>少</span>
            {#each [0.15, 0.35, 0.55, 0.75, 0.95] as t}
              <span class="inline-block w-3 h-3 rounded-sm" style="background: {heatmapColor(t, 1)}"></span>
            {/each}
            <span>多</span>
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
                title="{fmtDate(Number(ts))}: {hd.requests} 请求"
              ></div>
            {/each}
          </div>
        {:else}
          <div class="py-8 text-center text-xs text-nx-text-muted">暂无数据</div>
        {/if}
      </div>

      <!-- Model Usage -->
      <div class="nx-card p-4">
        <h3 class="text-sm font-medium text-nx-text mb-4">模型用量排行</h3>
        {#if getModelEntries().length > 0}
          {@const models = getModelEntries()}
          {@const mr = models[0][1].requests}
          <div class="space-y-2">
            {#each models as [model, md], i}
              <div class="flex items-center gap-3 py-1.5">
                <span class="w-5 text-right text-[11px] text-nx-text-muted tabular-nums">{i + 1}</span>
                <div class="w-36 truncate text-xs text-nx-text-secondary font-mono" title={model}>{model}</div>
                <div class="flex-1 h-3 rounded-full bg-nx-bg overflow-hidden">
                  <div
                    class="h-full rounded-full transition-all duration-1000 ease-out"
                    style="width: {(md.requests / mr) * 100}%; background: var(--nx-accent);"
                  ></div>
                </div>
                <div class="w-24 text-right text-[11px] text-nx-text-muted tabular-nums">{fmtTokens(md.input_tokens + md.output_tokens)} tokens</div>
                <div class="w-16 text-right text-[11px] text-nx-text-muted tabular-nums">{md.requests} 次</div>
              </div>
            {/each}
          </div>
        {:else}
          <div class="py-8 text-center text-xs text-nx-text-muted">暂无数据</div>
        {/if}
      </div>
    {:else}
      <div class="nx-card p-10 text-center">
        <span class="material-symbols-outlined text-2xl text-nx-text-muted/40 mb-2">bar_chart</span>
        <div class="text-sm text-nx-text-muted">等待数据...</div>
      </div>
    {/if}

  <!-- ════════════════════════════════════════════════════
       PROVIDERS TAB
       ════════════════════════════════════════════════════ -->
  {:else if activeTab === "providers"}
    <!-- Toolbar -->
    <div class="mb-4 flex items-center justify-between">
      <span class="text-xs text-nx-text-muted uppercase tracking-wider">{providers.length} 个 Provider</span>
      {#if !showForm}
        <button class="nx-btn nx-btn-primary flex items-center gap-1.5 px-3 py-1.5 text-xs" onclick={beginAdd}>
          <span class="material-symbols-outlined text-sm">add</span>
          添加 Provider
        </button>
      {/if}
    </div>

    <!-- ════ Add/Edit Form ════ -->
    {#if showForm}
      <div class="nx-card p-5 mb-4">
        <!-- Header -->
        <div class="mb-4 flex items-center justify-between">
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-nx-accent text-lg">{editingId ? "edit" : "add_circle"}</span>
            <span class="text-sm font-medium text-nx-text">{editingId ? "编辑 Provider" : "添加 Provider"}</span>
          </div>
          <button class="nx-btn nx-btn-ghost p-1" onclick={cancelForm}>
            <span class="material-symbols-outlined text-base">close</span>
          </button>
        </div>

        <!-- Form fields -->
        <div class="grid grid-cols-2 gap-3 mb-4">
          <div>
            <label for="f-name" class="mb-1.5 block text-xs text-nx-text-muted">名称</label>
            <input id="f-name" bind:value={form.name} class="nx-input w-full" placeholder="My OpenAI" />
          </div>
          <div class="col-span-2">
            <label for="f-protocol" class="mb-1.5 block text-xs text-nx-text-muted">API 协议</label>
            <select id="f-protocol" bind:value={form.protocol} class="nx-input w-full" onchange={onProtocolChange}>
              {#each protocolOptions as pt}
                <option value={pt.id}>{pt.label}</option>
              {/each}
            </select>
            <p class="mt-1 text-[10px] text-nx-text-muted/70 font-mono">
              {protocolOptions.find(p => p.id === form.protocol)?.endpoint || ""}
              — {protocolOptions.find(p => p.id === form.protocol)?.desc || ""}
            </p>
          </div>
          <div class="col-span-2">
            <label for="f-base-url" class="mb-1.5 block text-xs text-nx-text-muted">Base URL</label>
            <input id="f-base-url" bind:value={form.base_url} class="nx-input w-full" placeholder="https://api.openai.com" />
          </div>
          <div class="col-span-2">
            <label for="f-api-key" class="mb-1.5 block text-xs text-nx-text-muted">API Key <span class="text-nx-text-muted/40">（可选）</span></label>
            <input id="f-api-key" type="password" bind:value={form.api_key} class="nx-input w-full" placeholder="sk-..." />
          </div>
        </div>

        <!-- Model fetching -->
        <div class="mb-3">
          <div class="flex items-center gap-3 mb-3">
            <button class="nx-btn nx-btn-primary flex items-center gap-1.5 px-3 py-1.5 text-xs" onclick={fetchModels} disabled={fetchingModels}>
              {#if fetchingModels}
                <span class="material-symbols-outlined text-sm nx-animate-spin">progress_activity</span>
                正在获取...
              {:else}
                <span class="material-symbols-outlined text-sm">download</span>
                获取模型列表
              {/if}
            </button>
            {#if fetchedModels.length > 0}
              <span class="text-xs text-nx-text-muted">
                已获取 <span class="font-mono text-nx-text-secondary">{fetchedModels.length}</span> 个模型，
                已选 <span class="font-mono text-nx-accent">{selectedCount()}</span>
              </span>
              <div class="ml-auto flex gap-1">
                <button class="nx-btn nx-btn-ghost text-xs px-2 py-1" onclick={selectAll}>全选</button>
                <button class="nx-btn nx-btn-ghost text-xs px-2 py-1" onclick={deselectAll}>全不选</button>
              </div>
            {/if}
          </div>

          <!-- Model list -->
          {#if fetchedModels.length > 0}
            <div class="max-h-60 overflow-y-auto rounded-md border border-nx-border bg-nx-bg/50">
              {#each fetchedModels as m}
                <div
                  class="flex items-center gap-3 px-3 py-2 border-b border-nx-border last:border-0 hover:bg-nx-hover transition-colors"
                  role="option"
                  aria-selected={selectedModels[m.id]}
                  onclick={() => toggleModel(m.id)}
                  onkeydown={(e) => e.key === 'Enter' && toggleModel(m.id)}
                  tabindex="0"
                >
                  <!-- Checkbox -->
                  <div class="w-5 flex justify-center shrink-0">
                    {#if selectedModels[m.id]}
                      <div class="w-4 h-4 rounded-sm bg-nx-accent flex items-center justify-center">
                        <span class="material-symbols-outlined text-white text-[11px]">check</span>
                      </div>
                    {:else}
                      <div class="w-4 h-4 rounded-sm border border-nx-border-light"></div>
                    {/if}
                  </div>
                  <!-- Model info -->
                  <div class="flex-1 min-w-0">
                    <div class="text-xs font-mono text-nx-text truncate">{m.id}</div>
                    {#if m.id !== m.name}
                      <div class="text-[10px] text-nx-text-muted">{m.name}</div>
                    {/if}
                  </div>
                  <!-- Alias input -->
                  {#if selectedModels[m.id]}
                    <input
                      type="text"
                      class="w-32 text-right nx-input py-0.5 text-[11px]"
                      bind:value={form.model_aliases[m.id]}
                      placeholder="别名"
                      onclick={(e) => e.stopPropagation()}
                      onkeydown={(e) => e.stopPropagation()}
                    />
                  {/if}
                  {#if m.owned_by}
                    <div class="text-[10px] text-nx-text-muted shrink-0">{m.owned_by}</div>
                  {/if}
                </div>
              {/each}
            </div>

            <!-- Manual model input -->
            <div class="mt-3 flex items-end gap-2">
              <div class="flex-1">
                <label for="f-manual-id" class="mb-1 block text-[10px] text-nx-text-muted">模型 ID</label>
                <input id="f-manual-id" type="text" bind:value={manualModelId} class="nx-input w-full py-1.5 text-xs" placeholder="gpt-4o, claude-sonnet-4..." onkeydown={(e) => { if (e.key === 'Enter') addManualModel(); }} />
              </div>
              <div class="w-36">
                <label for="f-manual-name" class="mb-1 block text-[10px] text-nx-text-muted">显示名称</label>
                <input id="f-manual-name" type="text" bind:value={manualModelName} class="nx-input w-full py-1.5 text-xs" placeholder="别名" onkeydown={(e) => { if (e.key === 'Enter') addManualModel(); }} />
              </div>
              <button class="nx-btn nx-btn-ghost flex items-center gap-1 px-3 py-1.5 text-xs shrink-0" onclick={addManualModel} disabled={!manualModelId.trim()}>
                <span class="material-symbols-outlined text-sm">add</span>
                添加
              </button>
            </div>
          {:else if !fetchingModels}
            <div class="nx-card p-6 text-center border-dashed" style="border-color: var(--nx-border-light);">
              <span class="material-symbols-outlined text-2xl text-nx-text-muted/40">download</span>
              <div class="mt-2 text-xs text-nx-text-muted">点击上方按钮从 API 获取可用模型，或手动添加</div>
            </div>
          {/if}
        </div>

        <!-- Action buttons -->
        <div class="mt-4 flex justify-end gap-2 pt-3 border-t border-nx-border">
          <button class="nx-btn nx-btn-ghost px-3 py-1.5 text-xs" onclick={cancelForm}>取消</button>
          <button class="nx-btn nx-btn-primary px-4 py-1.5 text-xs" onclick={saveForm} disabled={!form.name || !form.base_url || selectedCount() === 0}>
            {editingId ? "更新" : "添加"}
            <span class="opacity-60">({selectedCount()} 模型)</span>
          </button>
        </div>
      </div>
    {/if}

    <!-- ════ Provider List ════ -->
    {#each providers as p}
      {@const isEditing = showForm && editingId === p.id}
      <div class="nx-card mb-3 overflow-hidden">
        {#if isEditing}
          <!-- Inline edit mode -->
          <div class="p-5">
            <div class="mb-4 flex items-center justify-between">
              <div class="flex items-center gap-2">
                <span class="material-symbols-outlined text-nx-accent text-lg">edit</span>
                <span class="text-sm font-medium text-nx-text">编辑 — {p.name}</span>
              </div>
              <button class="nx-btn nx-btn-ghost p-1" onclick={cancelForm}>
                <span class="material-symbols-outlined text-base">close</span>
              </button>
            </div>
            <div class="grid grid-cols-2 gap-3 mb-4">
              <div><label for="e-name" class="mb-1.5 block text-xs text-nx-text-muted">名称</label><input id="e-name" bind:value={form.name} class="nx-input w-full" /></div>
              <div><label for="e-protocol" class="mb-1.5 block text-xs text-nx-text-muted">API 协议</label><select id="e-protocol" bind:value={form.protocol} class="nx-input w-full" disabled>{#each protocolOptions as pt}<option value={pt.id}>{pt.label}</option>{/each}</select></div>
              <div class="col-span-2"><label for="e-base-url" class="mb-1.5 block text-xs text-nx-text-muted">Base URL</label><input id="e-base-url" bind:value={form.base_url} class="nx-input w-full" /></div>
              <div class="col-span-2"><label for="e-api-key" class="mb-1.5 block text-xs text-nx-text-muted">API Key</label><input id="e-api-key" type="password" bind:value={form.api_key} class="nx-input w-full" /></div>
            </div>
            <div class="flex items-center gap-3 mb-3">
              <button class="nx-btn nx-btn-primary flex items-center gap-1.5 px-3 py-1.5 text-xs" onclick={fetchModels} disabled={fetchingModels}>
                {#if fetchingModels}
                  <span class="material-symbols-outlined text-sm nx-animate-spin">progress_activity</span> 获取中...
                {:else}
                  <span class="material-symbols-outlined text-sm">download</span> 刷新模型
                {/if}
              </button>
              {#if fetchedModels.length > 0}
                <span class="text-xs text-nx-text-muted">已选 {selectedCount()} / {fetchedModels.length}</span>
              {/if}
            </div>
            {#if fetchedModels.length > 0}
              <div class="max-h-48 overflow-y-auto rounded-md border border-nx-border bg-nx-bg/50 mb-3">
                {#each fetchedModels as m}
                  <div class="flex items-center gap-3 px-3 py-2 border-b border-nx-border last:border-0 hover:bg-nx-hover transition-colors cursor-pointer" role="option" aria-selected={selectedModels[m.id]} tabindex="0" onclick={() => toggleModel(m.id)} onkeydown={(e) => e.key === 'Enter' && toggleModel(m.id)}>
                    <div class="w-5 flex justify-center shrink-0">
                      {#if selectedModels[m.id]}
                        <div class="w-4 h-4 rounded-sm bg-nx-accent flex items-center justify-center"><span class="material-symbols-outlined text-white text-[11px]">check</span></div>
                      {:else}
                        <div class="w-4 h-4 rounded-sm border border-nx-border-light"></div>
                      {/if}
                    </div>
                    <div class="flex-1 text-xs font-mono text-nx-text truncate">{m.id}</div>
                    {#if selectedModels[m.id]}
                      <input type="text" class="w-32 text-right nx-input py-0.5 text-[11px]" bind:value={form.model_aliases[m.id]} placeholder="别名" onclick={(e) => e.stopPropagation()} />
                    {/if}
                  </div>
                {/each}
              </div>
            {:else}
              <div class="text-xs text-nx-text-muted mb-3">当前已有 {p.models.length} 个模型（点击上方按钮刷新）</div>
            {/if}
            <div class="flex justify-end gap-2 pt-3 border-t border-nx-border">
              <button class="nx-btn nx-btn-ghost px-3 py-1.5 text-xs" onclick={cancelForm}>取消</button>
              <button class="nx-btn nx-btn-primary px-3 py-1.5 text-xs" onclick={saveForm}>更新</button>
            </div>
          </div>
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
                    {p.enabled ? '活跃' : '禁用'}
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
              <button class="nx-btn nx-btn-ghost p-1.5" onclick={() => beginEdit(p)} title="编辑">
                <span class="material-symbols-outlined text-base">edit</span>
              </button>
              <button class="nx-btn nx-btn-ghost p-1.5" style="color: var(--nx-danger);" onclick={() => deleteProvider(p.id)} title="删除">
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
        <div class="mt-2 text-sm text-nx-text-muted">还没有配置 Provider</div>
        <p class="mt-1 text-xs text-nx-text-muted/60">添加一个 AI API Provider 来开始使用 API Hub</p>
        <button class="nx-btn nx-btn-primary mt-4 px-4 py-2 text-xs" onclick={beginAdd}>
          <span class="material-symbols-outlined text-sm">add</span>
          添加第一个 Provider
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
              <th class="px-3 py-2.5">时间</th>
              <th class="px-3 py-2.5">模型</th>
              <th class="px-3 py-2.5">Provider</th>
              <th class="px-3 py-2.5 text-right">Tokens</th>
              <th class="px-3 py-2.5 text-right">延迟</th>
              <th class="px-3 py-2.5 text-center">状态</th>
            </tr>
          </thead>
          <tbody>
            {#each logs as log}
              <tr>
                <td class="px-3 py-2.5 whitespace-nowrap font-mono text-[11px] text-nx-text-muted">{fmtTime(log.timestamp)}</td>
                <td class="px-3 py-2.5 font-mono text-xs font-medium text-nx-text">{log.model}</td>
                <td class="px-3 py-2.5 text-xs text-nx-text-muted">{log.provider_name}</td>
                <td class="px-3 py-2.5 text-right text-xs text-nx-text-muted tabular-nums">
                  <span class="text-nx-text-secondary">↑{fmtTokens(log.input_tokens)}</span>
                  <span class="mx-0.5 opacity-30">/</span>
                  <span class="text-nx-text-secondary">↓{fmtTokens(log.output_tokens)}</span>
                </td>
                <td class="px-3 py-2.5 text-right text-xs text-nx-text-muted tabular-nums">{fmtLatency(log.latency_ms)}</td>
                <td class="px-3 py-2.5 text-center">
                  <span class="inline-flex items-center gap-1 text-xs {statusColor(log.status_code)}">
                    <span class="inline-block h-1.5 w-1.5 rounded-full" style="background: currentColor"></span>
                    {log.status_code || "—"}
                  </span>
                </td>
              </tr>
            {:else}
              <tr>
                <td colspan="6" class="px-3 py-12 text-center">
                  <span class="material-symbols-outlined text-xl text-nx-text-muted/30 mb-1">article</span>
                  <div class="text-xs text-nx-text-muted/50">暂无请求日志</div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  {/if}
</div>
