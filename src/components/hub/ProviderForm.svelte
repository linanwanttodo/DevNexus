<script>
  import { invoke } from "@tauri-apps/api/core";
  import { showToast } from "../../lib/toast.svelte.js";
  import { t, tFormat } from "../../lib/i18n.svelte.js";
  import ModelList from "./ModelList.svelte";

  let {
    mode = "add", // "add" | "edit"
    title,
    subtitle = "",
    initial = null, // provider object when editing
    protocolOptions = [],
    onSave, // (data, isEdit) => void
    onCancel, // () => void
  } = $props();

  let isEdit = $derived(mode === "edit");

  // 表单状态（每次打开组件时由 initial 初始化一次）
  function createForm() {
    return {
      name: initial?.name || "",
      protocol: initial?.protocol || "openai_chat",
      base_url: initial?.base_url || "https://api.openai.com",
      api_key: initial?.api_key || "",
      model_aliases: { ...(initial?.model_aliases || {}) },
      model_context_lengths: { ...(initial?.model_context_lengths || {}) },
    };
  }
  function createSelected() {
    const map = {};
    for (const m of initial?.models || []) map[m] = true;
    return map;
  }
  function createSeedModels() {
    return (initial?.models || []).slice();
  }

  let form = $state(createForm());
  let fetchedModels = $state([]);
  let selectedModels = $state(createSelected());
  let fetchingModels = $state(false);
  let addingManualModel = $state(false);
  let manualModelId = $state("");

  // 编辑时预选已关联的模型
  const seedModels = createSeedModels();

  function onProtocolChange() {
    const opt = protocolOptions.find(p => p.id === form.protocol);
    if (opt && !isEdit) form.base_url = opt.defaultUrl;
  }

  async function fetchModels() {
    if (!form.base_url || !form.protocol) { showToast(t("apiHub.errors.fillBaseUrl"), "error"); return; }
    fetchingModels = true; fetchedModels = [];
    try {
      fetchedModels = await invoke("api_hub_fetch_models", { baseUrl: form.base_url, apiKey: form.api_key || "", protocol: form.protocol, providerId: initial?.id });
      fetchedModels.forEach(m => {
        if (!(m.id in selectedModels)) { selectedModels[m.id] = true; form.model_aliases[m.id] = m.name || m.id; }
      });
      showToast(tFormat("apiHub.toast.fetchedModels", { count: fetchedModels.length }));
    } catch (err) { showToast(tFormat("apiHub.toast.fetchFailed", { error: err.message }), "error"); }
    finally { fetchingModels = false; }
  }

  function toggleModel(id) { selectedModels[id] = !selectedModels[id]; }

  function confirmManualAdd() {
    const id = manualModelId.trim();
    if (!id) return;
    if (fetchedModels.find(m => m.id === id)) { showToast(tFormat("apiHub.toast.modelExists", { id }), "error"); return; }
    const model = { id, name: id, owned_by: t("apiHub.custom"), enabled: true };
    fetchedModels = [...fetchedModels, model];
    selectedModels[id] = true;
    form.model_aliases[id] = id;
    manualModelId = "";
    showToast(tFormat("apiHub.toast.modelAdded", { id }));
  }

  function selectAll() { fetchedModels.forEach(m => selectedModels[m.id] = true); }
  function deselectAll() { fetchedModels.forEach(m => selectedModels[m.id] = false); }
  function selectedCount() { return Object.values(selectedModels).filter(Boolean).length; }

  function submit() {
    const models = Object.keys(selectedModels).filter(m => selectedModels[m]);
    if (models.length === 0) { showToast(t("apiHub.errors.selectModel"), "error"); return; }
    const model_aliases = {};
    models.forEach(m => { model_aliases[m] = form.model_aliases[m] || m; });
    const model_context_lengths = {};
    models.forEach(m => { if (form.model_context_lengths[m]) model_context_lengths[m] = Number(form.model_context_lengths[m]); });
    const data = {
      id: initial?.id || crypto.randomUUID(),
      name: form.name, protocol: form.protocol,
      base_url: form.base_url, api_key: form.api_key,
      models, model_aliases, model_context_lengths, enabled: true, created_at: Math.floor(Date.now() / 1000),
    };
    onSave(data, isEdit);
  }
</script>

<div
  class={isEdit ? "p-5" : "nx-card p-5 mb-4"}
  style={isEdit ? undefined : "box-shadow: none !important; border-color: var(--nx-border) !important;"}
>
  <!-- Header -->
  <div class="mb-4 flex items-center justify-between">
    <div class="flex items-center gap-2">
      <span class="material-symbols-outlined text-nx-accent text-lg">{isEdit ? "edit" : "add_circle"}</span>
      <span class="text-sm font-medium text-nx-text">{isEdit ? `${title} — ${subtitle}` : title}</span>
    </div>
    <button class="nx-btn nx-btn-ghost p-1" onclick={onCancel}>
      <span class="material-symbols-outlined text-base">close</span>
    </button>
  </div>

  <!-- Form fields -->
  <div class="grid grid-cols-2 gap-3 mb-4">
    <div>
      <label for="f-name" class="mb-1.5 block text-xs text-nx-text-muted">{t("apiHub.name")}</label>
      <input id="f-name" bind:value={form.name} class="nx-input w-full" placeholder="My OpenAI" />
    </div>
    <div class="col-span-2">
      <label for="f-protocol" class="mb-1.5 block text-xs text-nx-text-muted">{t("apiHub.protocolLabel")}</label>
      <select id="f-protocol" bind:value={form.protocol} class="nx-input w-full" disabled={isEdit} onchange={onProtocolChange}>
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
      <label for="f-base-url" class="mb-1.5 block text-xs text-nx-text-muted">{t("apiHub.baseUrl")}</label>
      <input id="f-base-url" bind:value={form.base_url} class="nx-input w-full" placeholder="https://api.openai.com" />
    </div>
    <div class="col-span-2">
      <label for="f-api-key" class="mb-1.5 block text-xs text-nx-text-muted">
        {t("apiHub.apiKey")} <span class="text-nx-text-muted/40">{isEdit ? t("apiHub.maskedHint") : t("apiHub.optional")}</span>
      </label>
      <input id="f-api-key" type="password" bind:value={form.api_key} class="nx-input w-full" placeholder={isEdit ? t("apiHub.apiKeyReplacePlaceholder") : "sk-..."} />
    </div>
  </div>

  <!-- Model fetching -->
  <div class="mb-3">
    <div class="flex items-center gap-3 mb-3">
      <button class="nx-btn nx-btn-primary flex items-center gap-1.5 px-3 py-1.5 text-xs" onclick={fetchModels} disabled={fetchingModels}>
        {#if fetchingModels}
          <span class="material-symbols-outlined text-sm nx-animate-spin">progress_activity</span>
          {t("apiHub.fetching")}
        {:else}
          <span class="material-symbols-outlined text-sm">download</span>
          {isEdit ? t("apiHub.refreshModels") : t("apiHub.fetchModels")}
        {/if}
      </button>
      {#if fetchedModels.length > 0}
        {#if isEdit}
          <span class="text-xs text-nx-text-muted">{t("apiHub.models.selected")} {selectedCount()} / {fetchedModels.length}</span>
        {:else}
          <span class="text-xs text-nx-text-muted">
            {tFormat("apiHub.models.fetched", { count: fetchedModels.length, selected: selectedCount() })}
          </span>
          <div class="ml-auto flex gap-1">
            <button class="nx-btn nx-btn-ghost text-xs px-2 py-1" onclick={selectAll}>{t("apiHub.selectAll")}</button>
            <button class="nx-btn nx-btn-ghost text-xs px-2 py-1" onclick={deselectAll}>{t("apiHub.deselectAll")}</button>
            <button class="nx-btn nx-btn-ghost text-xs px-2 py-1" onclick={() => addingManualModel = !addingManualModel} title={t("apiHub.manualAdd")}>
              <span class="material-symbols-outlined text-sm">add</span>
            </button>
          </div>
        {/if}
      {/if}
    </div>

    {#if fetchedModels.length > 0}
      {#if !isEdit && addingManualModel}
        <div class="flex items-center gap-2 px-3 py-2 border-b border-nx-border bg-nx-hover/50">
          <input type="text" class="flex-1 nx-input py-1 text-xs" bind:value={manualModelId} placeholder={t("apiHub.modelIdPlaceholder")} onkeydown={(e) => { if (e.key === 'Enter') confirmManualAdd(); if (e.key === 'Escape') { addingManualModel = false; } }} />
          <button class="nx-btn nx-btn-primary px-2 py-1 text-xs" onclick={confirmManualAdd} disabled={!manualModelId.trim()}>{t("apiHub.confirm")}</button>
          <button class="nx-btn nx-btn-ghost px-2 py-1 text-xs" onclick={() => addingManualModel = false}>{t("apiHub.cancel")}</button>
        </div>
      {/if}
      <ModelList
        models={fetchedModels}
        selected={selectedModels}
        aliases={form.model_aliases}
        contexts={form.model_context_lengths}
        onToggle={toggleModel}
        showCtx={!isEdit}
        maxH={isEdit ? "max-h-48" : "max-h-60"}
        extraClass={isEdit ? "mb-3" : ""}
      />
    {:else if !fetchingModels}
      {#if isEdit}
        <div class="text-xs text-nx-text-muted mb-3">{tFormat("apiHub.models.existing", { count: seedModels.length })}</div>
      {:else}
        <div class="nx-card p-6 text-center border-dashed" style="border-color: var(--nx-border-light);">
          <span class="material-symbols-outlined text-2xl text-nx-text-muted/40">download</span>
          <div class="mt-2 text-xs text-nx-text-muted">{t("apiHub.models.fetchHint")}</div>
          <button class="nx-btn nx-btn-ghost mt-2 text-xs" onclick={() => addingManualModel = true}>
            <span class="material-symbols-outlined text-sm">add</span>
            {t("apiHub.manualAddHint")}
          </button>
          {#if addingManualModel}
            <div class="mt-3 flex items-center gap-2 justify-center">
              <input type="text" class="nx-input py-1 text-xs w-56" bind:value={manualModelId} placeholder={t("apiHub.modelIdPlaceholder")} onkeydown={(e) => { if (e.key === 'Enter') confirmManualAdd(); if (e.key === 'Escape') { addingManualModel = false; } }} />
              <button class="nx-btn nx-btn-primary px-2 py-1 text-xs" onclick={confirmManualAdd} disabled={!manualModelId.trim()}>{t("apiHub.confirm")}</button>
            </div>
          {/if}
        </div>
      {/if}
    {/if}
  </div>

  <!-- Action buttons -->
  <div class="mt-4 flex justify-end gap-2 pt-3 border-t border-nx-border">
    <button class="nx-btn nx-btn-ghost px-3 py-1.5 text-xs" onclick={onCancel}>{t("apiHub.cancel")}</button>
    {#if isEdit}
      <button class="nx-btn nx-btn-primary px-3 py-1.5 text-xs" onclick={submit}>{t("apiHub.update")}</button>
    {:else}
      <button class="nx-btn nx-btn-primary px-4 py-1.5 text-xs" onclick={submit} disabled={!form.name || !form.base_url || selectedCount() === 0}>
        {t("apiHub.add")}
        <span class="opacity-60">({selectedCount()} {t("apiHub.models.countBadge")})</span>
      </button>
    {/if}
  </div>
</div>
