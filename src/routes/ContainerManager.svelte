<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.svelte.js";
  import { showConfirm } from "../lib/confirm.svelte.js";
  import { navigate } from "../lib/stores.svelte.js";
  import { t } from "../lib/i18n.svelte.js";
  import { friendlyError } from "../lib/errors.svelte.js";
  import ContainerIcons from "../icons/ContainerIcons.svelte";
  import ContainerDialog from "../components/containers/ContainerDialog.svelte";
  import ContainersTab from "../components/containers/ContainersTab.svelte";
  import ImagesTab from "../components/containers/ImagesTab.svelte";
  import VolumesTab from "../components/containers/VolumesTab.svelte";
  import NetworksTab from "../components/containers/NetworksTab.svelte";
  import ComposeTab from "../components/containers/ComposeTab.svelte";

  // ── State ──
  let activeTab = $state("containers");
  let dockerStatus = $state({ installed: false, version: "", running: false });
  let checking = $state(true);
  let containers = $state([]);
  let containersLoading = $state(false);
  let containerError = $state(null);
  let showAll = $state(false);
  let images = $state([]);
  let imagesLoading = $state(false);
  let imageError = $state(null);
  let volumes = $state([]);
  let volumesLoading = $state(false);
  let volumeError = $state(null);
  let networks = $state([]);
  let networksLoading = $state(false);
  let networkError = $state(null);
  let composeFile = $state("");
  let composeProject = $state("");
  let composeContainers = $state([]);
  let composeLoading = $state(false);
  let composeLogs = $state("");
  let composeError = $state(null);
  let showLogs = $state(false);
  let logContainer = $state("");
  let logContent = $state("");
  let logLoading = $state(false);
  let showTerminal = $state(false);
  let termContainer = $state("");
  let termCommand = $state("");
  let termOutput = $state("");
  let termLoading = $state(false);
  let showPull = $state(false);
  let pullImageName = $state("");
  let pullLoading = $state(false);
  let showBuild = $state(false);
  let buildTag = $state("");
  let buildPath = $state("");
  let buildLoading = $state(false);
  let showPush = $state(false);
  let pushTarget = $state("");
  let pushLoading = $state(false);
  let showTag = $state(false);
  let tagValue = $state("");
  let tagImageId = $state("");
  let tagLoading = $state(false);
  let showCreateVolume = $state(false);
  let newVolumeName = $state("");
  let showCreateNetwork = $state(false);
  let newNetworkName = $state("");
  let actionLoading = $state("");
  let search = $state("");

  const tabs = $derived([
    { id: "containers", label: t("docker.tab_containers"), icon: "container" },
    { id: "images", label: t("docker.tab_images"), icon: "image" },
    { id: "volumes", label: t("docker.tab_volumes"), icon: "volume" },
    { id: "networks", label: t("docker.tab_networks"), icon: "network" },
    { id: "compose", label: t("docker.tab_compose"), icon: "compose" }]);

  onMount(() => { checkDocker(); });

  async function checkDocker() {
    checking = true;
    try {
      dockerStatus = await invoke("check_docker");
      if (dockerStatus.installed && dockerStatus.running) loadTabData("containers");
    } catch {
      dockerStatus = { installed: false, version: "", running: false };
    } finally { checking = false; }
  }

  async function loadTabData(tab) {
    switch (tab) {
      case "containers": return loadContainers();
      case "images": return loadImages();
      case "volumes": return loadVolumes();
      case "networks": return loadNetworks();
    }
  }

  async function onTabChange(tab) {
    activeTab = tab;
    search = "";
    if (dockerStatus.installed && dockerStatus.running) await loadTabData(tab);
  }

  async function loadContainers() {
    containersLoading = true; containerError = null;
    try { containers = await invoke("list_containers", { all: showAll }); }
    catch (err) { containerError = friendlyError(err); }
    finally { containersLoading = false; }
  }

  async function containerAction(name, action) {
    if (action === "rm" || action === "stop") {
      const label = action === "rm" ? t("docker.delete_confirm") : t("docker.stop_confirm");
      if (!(await showConfirm(label.replace("{name}", name)))) return;
    }
    actionLoading = name;
    try {
      const result = await invoke("container_action", { name, action });
      showToast(result || t("docker.action_done"), "success");
      await loadContainers();
    } catch (err) {
      showToast(`${t("docker.action_failed")}: ${friendlyError(err)}`, "error");
    } finally { actionLoading = ""; }
  }

  async function openLogs(name) {
    logContainer = name; logContent = ""; showLogs = true; logLoading = true;
    try { logContent = await invoke("get_container_logs", { name, tail: 200 }); }
    catch (err) { logContent = `Error: ${friendlyError(err)}`; }
    finally { logLoading = false; }
  }

  async function openTerminal(name) {
    termContainer = name; termCommand = ""; termOutput = ""; showTerminal = true;
  }

  async function runTerminalCommand() {
    if (!termCommand.trim()) return;
    termLoading = true;
    try { termOutput = await invoke("exec_in_container", { name: termContainer, command: termCommand }); }
    catch (err) { termOutput = `Error: ${friendlyError(err)}`; }
    finally { termLoading = false; }
  }

  let filteredContainers = $derived(search.trim()
    ? containers.filter(c => c.name.toLowerCase().includes(search.toLowerCase()) || c.image.toLowerCase().includes(search.toLowerCase()) || c.id.toLowerCase().includes(search.toLowerCase()))
    : containers);

  async function loadImages() {
    imagesLoading = true; imageError = null;
    try { images = await invoke("list_images"); }
    catch (err) { imageError = friendlyError(err); }
    finally { imagesLoading = false; }
  }

  async function pullImageAction() {
    if (!pullImageName.trim()) return;
    pullLoading = true;
    try {
      const result = await invoke("pull_image", { image: pullImageName.trim() });
      showToast(result, "success"); showPull = false; pullImageName = "";
      await loadImages();
    } catch (err) {
      showToast(`${t("docker.pull_failed")}: ${friendlyError(err)}`, "error");
    } finally { pullLoading = false; }
  }

  async function removeImageAction(id, name) {
    if (!(await showConfirm(t("docker.image_delete_confirm").replace("{name}", name)))) return;
    actionLoading = id;
    try {
      const result = await invoke("remove_image", { imageId: id, force: false });
      showToast(result || t("docker.action_done"), "success"); await loadImages();
    } catch (err) {
      showToast(`${t("docker.action_failed")}: ${friendlyError(err)}`, "error");
    } finally { actionLoading = ""; }
  }

  async function buildImageAction() {
    if (!buildTag.trim() || !buildPath.trim()) return;
    buildLoading = true;
    try {
      const result = await invoke("build_image", { tag: buildTag.trim(), path: buildPath.trim() });
      showToast(result || t("docker.build_done"), "success"); showBuild = false; buildTag = ""; buildPath = "";
      await loadImages();
    } catch (err) {
      showToast(`${t("docker.build_failed")}: ${friendlyError(err)}`, "error");
    } finally { buildLoading = false; }
  }

  async function openPush(img) { pushTarget = `${img.repository}:${img.tag}`; showPush = true; }
  async function pushImageAction() {
    if (!pushTarget.trim()) return;
    pushLoading = true;
    try {
      const result = await invoke("push_image", { tag: pushTarget.trim() });
      showToast(result || t("docker.push_done"), "success");
      showPush = false; pushTarget = "";
    } catch (err) {
      showToast(`${t("docker.push_failed")}: ${friendlyError(err)}`, "error");
    } finally { pushLoading = false; }
  }

  async function openTag(img) { tagImageId = img.id; tagValue = ""; showTag = true; }
  async function tagImageAction() {
    if (!tagValue.trim()) return;
    tagLoading = true;
    try {
      const result = await invoke("tag_image", { imageId: tagImageId, tag: tagValue.trim() });
      showToast(result || t("docker.tag_done"), "success");
      showTag = false; tagValue = ""; tagImageId = "";
      await loadImages();
    } catch (err) {
      showToast(`${t("docker.tag_failed")}: ${friendlyError(err)}`, "error");
    } finally { tagLoading = false; }
  }

  let filteredImages = $derived(search.trim()
    ? images.filter(i => i.repository.toLowerCase().includes(search.toLowerCase()) || i.tag.toLowerCase().includes(search.toLowerCase()) || i.id.toLowerCase().includes(search.toLowerCase()))
    : images);

  async function loadVolumes() {
    volumesLoading = true; volumeError = null;
    try { volumes = await invoke("list_volumes"); }
    catch (err) { volumeError = friendlyError(err); }
    finally { volumesLoading = false; }
  }

  async function createVolume() {
    if (!newVolumeName.trim()) return;
    try {
      await invoke("volume_action", { name: newVolumeName.trim(), action: "create" });
      showToast(t("docker.volume_created"), "success"); showCreateVolume = false; newVolumeName = "";
      await loadVolumes();
    } catch (err) { showToast(`${t("docker.action_failed")}: ${friendlyError(err)}`, "error"); }
  }

  async function removeVolume(name) {
    if (!(await showConfirm(t("docker.volume_delete_confirm").replace("{name}", name)))) return;
    actionLoading = name;
    try {
      await invoke("volume_action", { name, action: "rm" });
      showToast(t("docker.volume_deleted"), "success"); await loadVolumes();
    } catch (err) {
      showToast(`${t("docker.action_failed")}: ${friendlyError(err)}`, "error");
    } finally { actionLoading = ""; }
  }

  async function loadNetworks() {
    networksLoading = true; networkError = null;
    try { networks = await invoke("list_networks"); }
    catch (err) { networkError = friendlyError(err); }
    finally { networksLoading = false; }
  }

  async function createNetwork() {
    if (!newNetworkName.trim()) return;
    try {
      await invoke("network_action", { name: newNetworkName.trim(), action: "create" });
      showToast(t("docker.network_created"), "success"); showCreateNetwork = false; newNetworkName = "";
      await loadNetworks();
    } catch (err) { showToast(`${t("docker.action_failed")}: ${friendlyError(err)}`, "error"); }
  }

  async function removeNetwork(name) {
    if (!(await showConfirm(t("docker.network_delete_confirm").replace("{name}", name)))) return;
    actionLoading = name;
    try {
      await invoke("network_action", { name, action: "rm" });
      showToast(t("docker.network_deleted"), "success"); await loadNetworks();
    } catch (err) {
      showToast(`${t("docker.action_failed")}: ${friendlyError(err)}`, "error");
    } finally { actionLoading = ""; }
  }

  async function composeUp() {
    composeLoading = true; composeError = null; composeLogs = "";
    try {
      const result = await invoke("compose_up", { file: composeFile.trim() || null, projectName: composeProject.trim() || null });
      showToast(result || t("docker.compose_up_done"), "success");
    } catch (err) { composeError = friendlyError(err); }
    finally { composeLoading = false; }
  }

  async function composeDown() {
    if (!(await showConfirm(t("docker.compose_down_confirm")))) return;
    composeLoading = true; composeError = null;
    try {
      const result = await invoke("compose_down", { file: composeFile.trim() || null, projectName: composeProject.trim() || null });
      showToast(result || t("docker.compose_down_done"), "success");
    } catch (err) { composeError = friendlyError(err); }
    finally { composeLoading = false; }
  }

  async function composePs() {
    composeLoading = true; composeError = null;
    try { composeContainers = await invoke("compose_ps", { file: composeFile.trim() || null, projectName: composeProject.trim() || null }); }
    catch (err) { composeError = friendlyError(err); }
    finally { composeLoading = false; }
  }

  async function composeViewLogs() {
    composeLoading = true; composeError = null;
    try { composeLogs = await invoke("compose_logs", { file: composeFile.trim() || null, projectName: composeProject.trim() || null, tail: 100 }); }
    catch (err) { composeError = friendlyError(err); }
    finally { composeLoading = false; }
  }

  // ── Dialog configs ──
  let pullConfig = $derived({ title: t("docker.pull_image"), icon: "download", width: "max-w-[300px]",
    fields: [{ id: "pull-image", placeholder: "nginx:latest", value: pullImageName, onInput: (v) => pullImageName = v }],
    loading: pullLoading, submitLabel: t("docker.pull"), loadingLabel: t("docker.pulling"),
    canSubmit: pullImageName.trim() !== "", onSubmit: pullImageAction, onClose: () => { showPull = false; pullImageName = ""; } });
  let buildConfig = $derived({ title: t("docker.build_image"), icon: "construction", width: "max-w-[300px]",
    fields: [
      { id: "build-tag", placeholder: `${t("docker.build_tag")} (myapp:latest)`, value: buildTag, onInput: (v) => buildTag = v, enterSubmit: false },
      { id: "build-path", placeholder: `${t("docker.build_path")} (.)`, value: buildPath, onInput: (v) => buildPath = v, enterSubmit: false },
    ],
    loading: buildLoading, submitLabel: t("docker.build"), loadingLabel: t("docker.building"),
    canSubmit: buildTag.trim() !== "" && buildPath.trim() !== "", onSubmit: buildImageAction, onClose: () => { showBuild = false; buildTag = ""; buildPath = ""; } });
  let pushConfig = $derived({ title: t("docker.push_image"), icon: "upload", width: "max-w-[300px]",
    fields: [{ id: "push-target", placeholder: "registry/user/repo:tag", value: pushTarget, onInput: (v) => pushTarget = v }],
    loading: pushLoading, submitLabel: t("docker.push"), loadingLabel: t("docker.pushing"),
    canSubmit: pushTarget.trim() !== "", onSubmit: pushImageAction, onClose: () => { showPush = false; pushTarget = ""; } });
  let tagConfig = $derived({ title: t("docker.tag_image"), icon: "sell", width: "max-w-[300px]",
    fields: [{ id: "tag-value", placeholder: "registry/user/repo:tag", value: tagValue, onInput: (v) => tagValue = v }],
    loading: tagLoading, submitLabel: t("docker.tag"), loadingLabel: t("docker.tagging"),
    canSubmit: tagValue.trim() !== "", onSubmit: tagImageAction, onClose: () => { showTag = false; tagValue = ""; } });
  let createVolumeConfig = $derived({ title: t("docker.create_volume"), icon: "add", width: "max-w-[300px]",
    fields: [{ id: "volume-name", placeholder: "my_volume", value: newVolumeName, onInput: (v) => newVolumeName = v }],
    loading: false, submitLabel: t("docker.create"), loadingLabel: t("docker.create"),
    canSubmit: newVolumeName.trim() !== "", onSubmit: createVolume, onClose: () => { showCreateVolume = false; newVolumeName = ""; } });
  let createNetworkConfig = $derived({ title: t("docker.create_network"), icon: "add", width: "max-w-[300px]",
    fields: [{ id: "network-name", placeholder: "my_network", value: newNetworkName, onInput: (v) => newNetworkName = v }],
    loading: false, submitLabel: t("docker.create"), loadingLabel: t("docker.create"),
    canSubmit: newNetworkName.trim() !== "", onSubmit: createNetwork, onClose: () => { showCreateNetwork = false; newNetworkName = ""; } });
</script>

<div class="flex h-full flex-col">
  <!-- Header with back button -->
  <div class="flex items-center gap-2 border-b border-nx-border px-5 py-2.5">
    <button class="nx-back-btn" onclick={() => navigate("/dashboard")}>
      <span class="material-symbols-outlined text-lg">arrow_back</span>
      {t("nav.dashboard")}
    </button>
    <span class="text-xs text-nx-text-muted">/</span>
    <ContainerIcons name="docker-logo" size={16} />
    <h1 class="text-sm font-medium text-nx-text">{t("nav.containers")}</h1>
  </div>

  <!-- Tab pills -->
  <div class="flex items-center gap-1 border-b border-nx-border px-5 py-2.5">
    {#each tabs as tab}
      <button
        class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-md transition-colors
          {activeTab === tab.id
            ? 'bg-nx-accent-bg text-nx-accent'
            : 'text-nx-text-secondary hover:text-nx-text hover:bg-nx-hover'}"
        onclick={() => onTabChange(tab.id)}>
        <ContainerIcons name={tab.icon} size={14} />
        {tab.label}
      </button>
    {/each}
  </div>

  <!-- Content -->
  <div class="flex-1 overflow-y-auto p-5">
    {#if checking}
      <div class="flex items-center justify-center py-16">
        <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
      </div>
    {:else if !dockerStatus.installed}
      <div class="nx-card p-8 text-center">
        <ContainerIcons name="docker-logo" size={48} class="mx-auto text-nx-text-muted" />
        <h2 class="mt-4 text-base font-semibold text-nx-text">{t("docker.not_installed_title")}</h2>
        <p class="mt-2 max-w-md mx-auto text-sm text-nx-text-secondary">{t("docker.not_installed_desc")}</p>
        <div class="mt-6 text-left inline-block text-sm text-nx-text-secondary">
          <p class="font-medium mb-2">{t("docker.install_guide")}</p>
          <ul class="list-disc list-inside space-y-1">
            <li><span class="font-medium">macOS:</span> <a href="https://docs.docker.com/desktop/install/mac-install/" target="_blank" class="text-nx-accent underline">Docker Desktop</a></li>
            <li><span class="font-medium">Linux:</span> <code class="text-nx-accent">curl -fsSL https://get.docker.com | sh</code></li>
            <li><span class="font-medium">Windows:</span> <a href="https://docs.docker.com/desktop/install/windows-install/" target="_blank" class="text-nx-accent underline">Docker Desktop</a></li>
          </ul>
        </div>
      </div>
    {:else if !dockerStatus.running}
      <div class="nx-card p-8 text-center">
        <ContainerIcons name="container-exited" size={48} class="mx-auto" />
        <h2 class="mt-4 text-base font-semibold text-nx-text">{t("docker.not_running_title")}</h2>
        <p class="mt-2 text-sm text-nx-text-secondary">{t("docker.not_running_desc")}</p>
        <div class="mt-4 text-xs text-nx-text-muted">{dockerStatus.version}</div>
      </div>
    {:else}
      <!-- Search -->
      {#if activeTab === "containers" || activeTab === "images"}
        <div class="relative mb-4">
          <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-nx-text-muted text-sm pointer-events-none">search</span>
          <input type="text" placeholder={activeTab === "containers" ? t("docker.search_containers") : t("docker.search_images")}
            value={search} oninput={(e) => search = e.currentTarget.value}
            class="nx-input w-full pl-9 pr-8 h-9 text-sm" />
          {#if search}
            <button class="absolute right-2 top-1/2 -translate-y-1/2 text-nx-text-muted" onclick={() => search = ""}>
              <span class="material-symbols-outlined text-sm">close</span>
            </button>
          {/if}
        </div>
      {/if}

      {#if activeTab === "containers"}
        <ContainersTab items={filteredContainers} loading={containersLoading} error={containerError} search={search} showAll={showAll} actionLoading={actionLoading}
          onShowAllChange={(c) => { showAll = c; loadContainers(); }} onRefresh={loadContainers} onAction={containerAction} onLogs={openLogs} onTerminal={openTerminal} />
      {:else if activeTab === "images"}
        <ImagesTab items={filteredImages} loading={imagesLoading} error={imageError} search={search} actionLoading={actionLoading}
          onPull={() => { showPull = true; }} onBuild={() => { showBuild = true; }} onRefresh={loadImages} onPush={openPush} onTag={openTag} onRemove={removeImageAction} />
      {:else if activeTab === "volumes"}
        <VolumesTab items={volumes} loading={volumesLoading} error={volumeError} actionLoading={actionLoading}
          onCreate={() => { showCreateVolume = true; }} onRefresh={loadVolumes} onRemove={removeVolume} />
      {:else if activeTab === "networks"}
        <NetworksTab items={networks} loading={networksLoading} error={networkError} actionLoading={actionLoading}
          onCreate={() => { showCreateNetwork = true; }} onRefresh={loadNetworks} onRemove={removeNetwork} />
      {:else if activeTab === "compose"}
        <ComposeTab file={composeFile} project={composeProject} loading={composeLoading} error={composeError} containers={composeContainers} logs={composeLogs}
          onFileInput={(v) => composeFile = v} onProjectInput={(v) => composeProject = v}
          onUp={composeUp} onDown={composeDown} onPs={composePs} onLogs={composeViewLogs} onClearLogs={() => { composeLogs = ""; }} />
      {/if}
    {/if}
  </div>
</div>

<!-- ── Dialogs (same as before, style updated) ── -->

{#if showLogs}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
    <div class="w-full max-w-3xl bg-nx-surface border border-nx-border rounded-lg shadow-xl overflow-hidden">
      <div class="flex items-center justify-between px-4 py-3 border-b border-nx-border">
        <h2 class="text-sm font-semibold text-nx-text flex items-center gap-1.5">
          <span class="material-symbols-outlined text-sm">list_alt</span>
          {t("docker.logs")}: {logContainer}
        </h2>
        <button class="text-nx-text-muted hover:text-nx-text" onclick={() => { showLogs = false; }}>
          <span class="material-symbols-outlined">close</span>
        </button>
      </div>
      <div class="max-h-[500px] overflow-auto p-4 bg-nx-deep">
        {#if logLoading}
          <div class="flex items-center justify-center py-8">
            <span class="material-symbols-outlined animate-spin text-nx-text-muted">progress_activity</span>
          </div>
        {:else}
          <pre class="font-mono text-xs text-nx-text-secondary whitespace-pre-wrap">{logContent || t("docker.no_logs")}</pre>
        {/if}
      </div>
    </div>
  </div>
{/if}

{#if showTerminal}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
    <div class="w-full max-w-2xl bg-nx-surface border border-nx-border rounded-lg shadow-xl overflow-hidden">
      <div class="flex items-center justify-between px-4 py-3 border-b border-nx-border">
        <h2 class="text-sm font-semibold text-nx-text flex items-center gap-1.5">
          <span class="material-symbols-outlined text-sm">terminal</span>
          {t("docker.terminal")}: {termContainer}
        </h2>
        <button class="text-nx-text-muted hover:text-nx-text" onclick={() => { showTerminal = false; }}>
          <span class="material-symbols-outlined">close</span>
        </button>
      </div>
      <div class="p-4">
        <div class="flex gap-2 mb-3">
          <input type="text" bind:value={termCommand} placeholder={t("docker.terminal_ph")}
            class="nx-input flex-1 h-9 font-mono text-sm"
            onkeydown={(e) => { if (e.key === 'Enter') runTerminalCommand(); }} />
          <button class="nx-btn h-9" onclick={runTerminalCommand} disabled={termLoading}>
            {t("docker.run")}
          </button>
        </div>
        {#if termLoading}
          <div class="flex items-center justify-center py-4">
            <span class="material-symbols-outlined animate-spin text-nx-text-muted">progress_activity</span>
          </div>
        {:else if termOutput}
          <pre class="max-h-[300px] overflow-auto bg-nx-deep border border-nx-border rounded-lg p-3 font-mono text-xs text-nx-text-secondary whitespace-pre-wrap">{termOutput}</pre>
        {/if}
      </div>
    </div>
  </div>
{/if}

{#if showPull}<ContainerDialog config={pullConfig} />{/if}
{#if showBuild}<ContainerDialog config={buildConfig} />{/if}
{#if showPush}<ContainerDialog config={pushConfig} />{/if}
{#if showTag}<ContainerDialog config={tagConfig} />{/if}
{#if showCreateVolume}<ContainerDialog config={createVolumeConfig} />{/if}
{#if showCreateNetwork}<ContainerDialog config={createNetworkConfig} />{/if}
