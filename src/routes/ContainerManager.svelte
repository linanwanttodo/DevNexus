<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.svelte.js";
  import { showConfirm } from "../lib/confirm.svelte.js";
  import { getSearchQuery, setSearchQuery, navigate } from "../lib/stores.svelte.js";
  import { t } from "../lib/i18n.svelte.js";
  import ContainerIcons from "../icons/ContainerIcons.svelte";

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

  let search = $derived(getSearchQuery());

  const tabs = [
    { id: "containers", label: t("docker.tab_containers"), icon: "container" },
    { id: "images", label: t("docker.tab_images"), icon: "image" },
    { id: "volumes", label: t("docker.tab_volumes"), icon: "volume" },
    { id: "networks", label: t("docker.tab_networks"), icon: "network" },
    { id: "compose", label: t("docker.tab_compose"), icon: "compose" },
  ];

  onMount(() => { checkDocker(); });

  async function checkDocker() {
    checking = true;
    try {
      dockerStatus = await invoke("check_docker");
      if (dockerStatus.installed && dockerStatus.running) loadTabData("containers");
    } catch {
      dockerStatus = { installed: false, version: "", running: false };
    } finally {
      checking = false;
    }
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
    setSearchQuery("");
    if (dockerStatus.installed && dockerStatus.running) await loadTabData(tab);
  }

  async function loadContainers() {
    containersLoading = true; containerError = null;
    try { containers = await invoke("list_containers", { all: showAll }); }
    catch (err) { containerError = err.message || String(err); }
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
      showToast(`${t("docker.action_failed")}: ${err.message || err}`, "error");
    } finally { actionLoading = ""; }
  }

  async function openLogs(name) {
    logContainer = name; logContent = ""; showLogs = true; logLoading = true;
    try { logContent = await invoke("get_container_logs", { name, tail: 200 }); }
    catch (err) { logContent = `Error: ${err.message || err}`; }
    finally { logLoading = false; }
  }

  async function openTerminal(name) {
    termContainer = name; termCommand = ""; termOutput = ""; showTerminal = true;
  }

  async function runTerminalCommand() {
    if (!termCommand.trim()) return;
    termLoading = true;
    try { termOutput = await invoke("exec_in_container", { name: termContainer, command: termCommand }); }
    catch (err) { termOutput = `Error: ${err.message || err}`; }
    finally { termLoading = false; }
  }

  let filteredContainers = $derived(
    search.trim()
      ? containers.filter(c =>
          c.name.toLowerCase().includes(search.toLowerCase()) ||
          c.image.toLowerCase().includes(search.toLowerCase()) ||
          c.id.toLowerCase().includes(search.toLowerCase()))
      : containers
  );

  async function loadImages() {
    imagesLoading = true; imageError = null;
    try { images = await invoke("list_images"); }
    catch (err) { imageError = err.message || String(err); }
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
      showToast(`${t("docker.pull_failed")}: ${err.message || err}`, "error");
    } finally { pullLoading = false; }
  }

  async function removeImageAction(id, name) {
    if (!(await showConfirm(t("docker.image_delete_confirm").replace("{name}", name)))) return;
    actionLoading = id;
    try {
      const result = await invoke("remove_image", { imageId: id, force: false });
      showToast(result || t("docker.action_done"), "success"); await loadImages();
    } catch (err) {
      showToast(`${t("docker.action_failed")}: ${err.message || err}`, "error");
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
      showToast(`${t("docker.build_failed")}: ${err.message || err}`, "error");
    } finally { buildLoading = false; }
  }

  async function openPush(img) {
    pushTarget = `${img.repository}:${img.tag}`;
    showPush = true;
  }
  async function pushImageAction() {
    if (!pushTarget.trim()) return;
    pushLoading = true;
    try {
      const result = await invoke("push_image", { tag: pushTarget.trim() });
      showToast(result || t("docker.push_done"), "success");
      showPush = false; pushTarget = "";
    } catch (err) {
      showToast(`${t("docker.push_failed")}: ${err.message || err}`, "error");
    } finally { pushLoading = false; }
  }

  async function openTag(img) {
    tagImageId = img.id;
    tagValue = "";
    showTag = true;
  }
  async function tagImageAction() {
    if (!tagValue.trim()) return;
    tagLoading = true;
    try {
      const result = await invoke("tag_image", { imageId: tagImageId, tag: tagValue.trim() });
      showToast(result || t("docker.tag_done"), "success");
      showTag = false; tagValue = ""; tagImageId = "";
      await loadImages();
    } catch (err) {
      showToast(`${t("docker.tag_failed")}: ${err.message || err}`, "error");
    } finally { tagLoading = false; }
  }

  let filteredImages = $derived(
    search.trim()
      ? images.filter(i =>
          i.repository.toLowerCase().includes(search.toLowerCase()) ||
          i.tag.toLowerCase().includes(search.toLowerCase()) ||
          i.id.toLowerCase().includes(search.toLowerCase()))
      : images
  );

  async function loadVolumes() {
    volumesLoading = true; volumeError = null;
    try { volumes = await invoke("list_volumes"); }
    catch (err) { volumeError = err.message || String(err); }
    finally { volumesLoading = false; }
  }

  async function createVolume() {
    if (!newVolumeName.trim()) return;
    try {
      await invoke("volume_action", { name: newVolumeName.trim(), action: "create" });
      showToast(t("docker.volume_created"), "success"); showCreateVolume = false; newVolumeName = "";
      await loadVolumes();
    } catch (err) { showToast(`${t("docker.action_failed")}: ${err.message || err}`, "error"); }
  }

  async function removeVolume(name) {
    if (!(await showConfirm(t("docker.volume_delete_confirm").replace("{name}", name)))) return;
    actionLoading = name;
    try {
      await invoke("volume_action", { name, action: "rm" });
      showToast(t("docker.volume_deleted"), "success"); await loadVolumes();
    } catch (err) {
      showToast(`${t("docker.action_failed")}: ${err.message || err}`, "error");
    } finally { actionLoading = ""; }
  }

  async function loadNetworks() {
    networksLoading = true; networkError = null;
    try { networks = await invoke("list_networks"); }
    catch (err) { networkError = err.message || String(err); }
    finally { networksLoading = false; }
  }

  async function createNetwork() {
    if (!newNetworkName.trim()) return;
    try {
      await invoke("network_action", { name: newNetworkName.trim(), action: "create" });
      showToast(t("docker.network_created"), "success"); showCreateNetwork = false; newNetworkName = "";
      await loadNetworks();
    } catch (err) { showToast(`${t("docker.action_failed")}: ${err.message || err}`, "error"); }
  }

  async function removeNetwork(name) {
    if (!(await showConfirm(t("docker.network_delete_confirm").replace("{name}", name)))) return;
    actionLoading = name;
    try {
      await invoke("network_action", { name, action: "rm" });
      showToast(t("docker.network_deleted"), "success"); await loadNetworks();
    } catch (err) {
      showToast(`${t("docker.action_failed")}: ${err.message || err}`, "error");
    } finally { actionLoading = ""; }
  }

  async function composeUp() {
    composeLoading = true; composeError = null; composeLogs = "";
    try {
      const result = await invoke("compose_up", { file: composeFile.trim() || null, projectName: composeProject.trim() || null });
      showToast(result || t("docker.compose_up_done"), "success");
    } catch (err) { composeError = err.message || String(err); }
    finally { composeLoading = false; }
  }

  async function composeDown() {
    if (!(await showConfirm(t("docker.compose_down_confirm")))) return;
    composeLoading = true; composeError = null;
    try {
      const result = await invoke("compose_down", { file: composeFile.trim() || null, projectName: composeProject.trim() || null });
      showToast(result || t("docker.compose_down_done"), "success");
    } catch (err) { composeError = err.message || String(err); }
    finally { composeLoading = false; }
  }

  async function composePs() {
    composeLoading = true; composeError = null;
    try { composeContainers = await invoke("compose_ps", { file: composeFile.trim() || null, projectName: composeProject.trim() || null }); }
    catch (err) { composeError = err.message || String(err); }
    finally { composeLoading = false; }
  }

  async function composeViewLogs() {
    composeLoading = true; composeError = null;
    try { composeLogs = await invoke("compose_logs", { file: composeFile.trim() || null, projectName: composeProject.trim() || null, tail: 100 }); }
    catch (err) { composeError = err.message || String(err); }
    finally { composeLoading = false; }
  }

  function statusLabel(status) {
    const map = {
      running: t("docker.status_running"), exited: t("docker.status_exited"),
      paused: t("docker.status_paused"), created: t("docker.status_created"),
    };
    return map[status] || status;
  }

  function shortId(id) { return id ? id.substring(0, 12) : ""; }
  function formatCreated(created) { return created || "-"; }
  function formatSize(size) { return size || "-"; }
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

      <!-- ── CONTAINERS ── -->
      {#if activeTab === "containers"}
        <div>
          <div class="mb-4 flex items-center justify-between">
            <div class="flex items-center gap-3">
              <label class="flex items-center gap-1.5 text-xs text-nx-text-muted cursor-pointer select-none">
                <input type="checkbox" bind:checked={showAll} onchange={loadContainers} class="rounded border-nx-border bg-nx-bg" />
                {t("docker.show_all")}
              </label>
            </div>
            <button class="nx-btn nx-btn-ghost px-2 py-1 text-xs" onclick={loadContainers} disabled={containersLoading}>
              <span class="material-symbols-outlined text-sm {containersLoading ? 'animate-spin' : ''}">refresh</span>
              {t("common.refresh")}
            </button>
          </div>

          <!-- Search -->
          <div class="relative mb-4">
            <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-nx-text-muted text-sm pointer-events-none">search</span>
            <input type="text" placeholder={t("docker.search_containers")}
              value={search} oninput={(e) => setSearchQuery(e.currentTarget.value)}
              class="nx-input w-full pl-9 pr-8 h-9 text-sm" />
            {#if search}
              <button class="absolute right-2 top-1/2 -translate-y-1/2 text-nx-text-muted" onclick={() => setSearchQuery("")}>
                <span class="material-symbols-outlined text-sm">close</span>
              </button>
            {/if}
          </div>

          <div class="nx-section">
            {#if containersLoading && containers.length === 0}
              <div class="flex items-center justify-center py-12">
                <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
              </div>
            {:else if containerError}
              <div class="p-6 text-center">
                <span class="material-symbols-outlined text-nx-danger text-3xl">error</span>
                <div class="mt-2 text-sm text-nx-danger">{containerError}</div>
                <button class="nx-btn nx-btn-primary mt-4" onclick={loadContainers}>{t("common.retry")}</button>
              </div>
            {:else if filteredContainers.length === 0}
              <div class="p-12 text-center">
                <ContainerIcons name="container" size={36} class="mx-auto text-nx-text-muted" />
                <div class="mt-3 text-sm text-nx-text-muted">{search ? t("docker.no_matching") : t("docker.no_containers")}</div>
              </div>
            {:else}
              <table class="nx-table">
                <thead>
                  <tr>
                    <th class="w-4"></th>
                    <th>{t("docker.name")}</th>
                    <th>{t("docker.image")}</th>
                    <th>{t("docker.ports")}</th>
                    <th class="w-28">{t("docker.created")}</th>
                    <th class="text-right w-80">{t("docker.actions")}</th>
                  </tr>
                </thead>
                <tbody>
                  {#each filteredContainers as c}
                    <tr>
                      <td class="!pr-0">
                        {#if c.status === "running"}
                          <ContainerIcons name="container-running" size={16} />
                        {:else if c.status === "paused"}
                          <ContainerIcons name="container-paused" size={16} />
                        {:else}
                          <ContainerIcons name="container-exited" size={16} />
                        {/if}
                      </td>
                      <td>
                        <div class="flex flex-col">
                          <span class="text-sm font-medium text-nx-text">{c.name}</span>
                          <span class="font-mono text-xs text-nx-text-muted">{shortId(c.id)}</span>
                        </div>
                      </td>
                      <td class="font-mono text-xs text-nx-text-secondary">{c.image}</td>
                      <td class="font-mono text-xs text-nx-text-muted max-w-[180px] truncate">{c.ports || "-"}</td>
                      <td class="text-xs text-nx-text-muted">{formatCreated(c.created)}</td>
                      <td class="text-right">
                        <span class="flex items-center justify-end gap-1">
                          {#if c.status === "running"}
                            <button class="nx-btn text-xs h-7 px-2 text-nx-warning" onclick={() => containerAction(c.name, "pause")} disabled={actionLoading === c.name}>{t("docker.pause")}</button>
                            <button class="nx-btn text-xs h-7 px-2 text-nx-danger" onclick={() => containerAction(c.name, "stop")} disabled={actionLoading === c.name}>{t("docker.stop")}</button>
                          {:else if c.status === "paused"}
                            <button class="nx-btn text-xs h-7 px-2" onclick={() => containerAction(c.name, "unpause")} disabled={actionLoading === c.name}>{t("docker.unpause")}</button>
                          {:else}
                            <button class="nx-btn text-xs h-7 px-2 text-nx-success" onclick={() => containerAction(c.name, "start")} disabled={actionLoading === c.name}>{t("docker.start")}</button>
                          {/if}
                          <button class="nx-btn text-xs h-7 px-2" onclick={() => containerAction(c.name, "restart")} disabled={actionLoading === c.name}>{t("docker.restart")}</button>
                          <button class="nx-btn text-xs h-7 px-2" onclick={() => openLogs(c.name)}><span class="material-symbols-outlined text-sm">list_alt</span></button>
                          <button class="nx-btn text-xs h-7 px-2" onclick={() => openTerminal(c.name)}><span class="material-symbols-outlined text-sm">terminal</span></button>
                          <button class="nx-btn text-xs h-7 px-2 text-nx-danger" onclick={() => containerAction(c.name, "rm")} disabled={actionLoading === c.name}>{t("docker.delete")}</button>
                        </span>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
              <div class="flex items-center justify-between border-t border-nx-border px-4 py-2">
                <span class="text-xs text-nx-text-muted">{filteredContainers.length} {t("docker.containers_count")}</span>
              </div>
            {/if}
          </div>
        </div>

      {:else if activeTab === "images"}
        <div>
          <div class="mb-4 flex items-center justify-between">
            <div class="flex items-center gap-2">
              <button class="nx-btn text-xs h-7" onclick={() => { showPull = true; }}>
                <span class="material-symbols-outlined text-sm">download</span>{t("docker.pull")}
              </button>
              <button class="nx-btn text-xs h-7" onclick={() => { showBuild = true; }}>
                <span class="material-symbols-outlined text-sm">construction</span>{t("docker.build")}
              </button>
            </div>
            <button class="nx-btn nx-btn-ghost px-2 text-xs h-7" onclick={loadImages} disabled={imagesLoading}>
              <span class="material-symbols-outlined text-sm {imagesLoading ? 'animate-spin' : ''}">refresh</span>
              {t("common.refresh")}
            </button>
          </div>

          <div class="relative mb-4">
            <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-nx-text-muted text-sm pointer-events-none">search</span>
            <input type="text" placeholder={t("docker.search_images")}
              value={search} oninput={(e) => setSearchQuery(e.currentTarget.value)}
              class="nx-input w-full pl-9 pr-8 h-9 text-sm" />
            {#if search}
              <button class="absolute right-2 top-1/2 -translate-y-1/2 text-nx-text-muted" onclick={() => setSearchQuery("")}>
                <span class="material-symbols-outlined text-sm">close</span>
              </button>
            {/if}
          </div>

          <div class="nx-section">
            {#if imagesLoading && images.length === 0}
              <div class="flex items-center justify-center py-12">
                <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
              </div>
            {:else if imageError}
              <div class="p-6 text-center">
                <span class="material-symbols-outlined text-nx-danger text-3xl">error</span>
                <div class="mt-2 text-sm text-nx-danger">{imageError}</div>
                <button class="nx-btn nx-btn-primary mt-4" onclick={loadImages}>{t("common.retry")}</button>
              </div>
            {:else if filteredImages.length === 0}
              <div class="p-12 text-center">
                <ContainerIcons name="image" size={36} class="mx-auto text-nx-text-muted" />
                <div class="mt-3 text-sm text-nx-text-muted">{search ? t("docker.no_matching") : t("docker.no_images")}</div>
              </div>
            {:else}
              <table class="nx-table">
                <thead>
                  <tr>
                    <th>{t("docker.repository")}</th>
                    <th>{t("docker.tag")}</th>
                    <th>{t("docker.image_id")}</th>
                    <th>{t("docker.created")}</th>
                    <th class="text-right">{t("docker.size")}</th>
                    <th class="text-right w-24">{t("docker.actions")}</th>
                  </tr>
                </thead>
                <tbody>
                  {#each filteredImages as img}
                    <tr>
                      <td class="text-sm font-medium text-nx-text">{img.repository}</td>
                      <td><span class="nx-pill font-mono text-[10px]">{img.tag}</span></td>
                      <td class="font-mono text-xs text-nx-text-muted">{shortId(img.id)}</td>
                      <td class="text-xs text-nx-text-muted">{formatCreated(img.created)}</td>
                      <td class="text-right text-xs text-nx-text-secondary">{formatSize(img.size)}</td>
                      <td class="text-right">
                        <span class="flex items-center justify-end gap-1">
                          <button class="nx-btn text-xs h-7 px-2" onclick={() => openPush(img)}>{t("docker.push")}</button>
                          <button class="nx-btn text-xs h-7 px-2" onclick={() => openTag(img)}>{t("docker.tag")}</button>
                          <button class="nx-btn text-xs h-7 px-2 text-nx-danger"
                            onclick={() => removeImageAction(img.id, `${img.repository}:${img.tag}`)} disabled={actionLoading === img.id}>
                            {t("docker.delete")}
                          </button>
                        </span>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
              <div class="flex items-center justify-between border-t border-nx-border px-4 py-2">
                <span class="text-xs text-nx-text-muted">{filteredImages.length} {t("docker.images_count")}</span>
              </div>
            {/if}
          </div>
        </div>

      {:else if activeTab === "volumes"}
        <div>
          <div class="mb-4 flex items-center justify-between">
            <div class="flex items-center gap-2">
              <button class="nx-btn text-xs h-7" onclick={() => { showCreateVolume = true; }}>
                <span class="material-symbols-outlined text-sm">add</span>{t("docker.create")}
              </button>
            </div>
            <button class="nx-btn nx-btn-ghost px-2 text-xs h-7" onclick={loadVolumes} disabled={volumesLoading}>
              <span class="material-symbols-outlined text-sm {volumesLoading ? 'animate-spin' : ''}">refresh</span>
              {t("common.refresh")}
            </button>
          </div>

          <div class="nx-section">
            {#if volumesLoading && volumes.length === 0}
              <div class="flex items-center justify-center py-12">
                <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
              </div>
            {:else if volumeError}
              <div class="p-6 text-center">
                <span class="material-symbols-outlined text-nx-danger text-3xl">error</span>
                <div class="mt-2 text-sm text-nx-danger">{volumeError}</div>
                <button class="nx-btn nx-btn-primary mt-4" onclick={loadVolumes}>{t("common.retry")}</button>
              </div>
            {:else if volumes.length === 0}
              <div class="p-12 text-center">
                <ContainerIcons name="volume" size={36} class="mx-auto text-nx-text-muted" />
                <div class="mt-3 text-sm text-nx-text-muted">{t("docker.no_volumes")}</div>
              </div>
            {:else}
              <table class="nx-table">
                <thead>
                  <tr>
                    <th>{t("docker.name")}</th>
                    <th>{t("docker.driver")}</th>
                    <th>{t("docker.mountpoint")}</th>
                    <th>{t("docker.created")}</th>
                    <th class="text-right w-24">{t("docker.actions")}</th>
                  </tr>
                </thead>
                <tbody>
                  {#each volumes as v}
                    <tr>
                      <td class="text-sm font-medium text-nx-text">{v.name}</td>
                      <td class="text-xs text-nx-text-secondary">{v.driver}</td>
                      <td class="font-mono text-xs text-nx-text-muted max-w-[280px] truncate" title={v.mountpoint}>{v.mountpoint}</td>
                      <td class="text-xs text-nx-text-muted">{formatCreated(v.created)}</td>
                      <td class="text-right">
                        <button class="nx-btn text-xs h-7 px-2 text-nx-danger"
                          onclick={() => removeVolume(v.name)} disabled={actionLoading === v.name}>
                          {t("docker.delete")}
                        </button>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            {/if}
          </div>
        </div>

      {:else if activeTab === "networks"}
        <div>
          <div class="mb-4 flex items-center justify-between">
            <div class="flex items-center gap-2">
              <button class="nx-btn text-xs h-7" onclick={() => { showCreateNetwork = true; }}>
                <span class="material-symbols-outlined text-sm">add</span>{t("docker.create")}
              </button>
            </div>
            <button class="nx-btn nx-btn-ghost px-2 text-xs h-7" onclick={loadNetworks} disabled={networksLoading}>
              <span class="material-symbols-outlined text-sm {networksLoading ? 'animate-spin' : ''}">refresh</span>
              {t("common.refresh")}
            </button>
          </div>

          <div class="nx-section">
            {#if networksLoading && networks.length === 0}
              <div class="flex items-center justify-center py-12">
                <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
              </div>
            {:else if networkError}
              <div class="p-6 text-center">
                <span class="material-symbols-outlined text-nx-danger text-3xl">error</span>
                <div class="mt-2 text-sm text-nx-danger">{networkError}</div>
                <button class="nx-btn nx-btn-primary mt-4" onclick={loadNetworks}>{t("common.retry")}</button>
              </div>
            {:else if networks.length === 0}
              <div class="p-12 text-center">
                <ContainerIcons name="network" size={36} class="mx-auto text-nx-text-muted" />
                <div class="mt-3 text-sm text-nx-text-muted">{t("docker.no_networks")}</div>
              </div>
            {:else}
              <table class="nx-table">
                <thead>
                  <tr>
                    <th>{t("docker.name")}</th>
                    <th>{t("docker.driver")}</th>
                    <th>{t("docker.scope")}</th>
                    <th class="text-right w-24">{t("docker.actions")}</th>
                  </tr>
                </thead>
                <tbody>
                  {#each networks as n}
                    <tr>
                      <td class="text-sm font-medium text-nx-text">{n.name}</td>
                      <td class="text-xs text-nx-text-secondary">{n.driver}</td>
                      <td class="text-xs text-nx-text-muted">{n.scope}</td>
                      <td class="text-right">
                        <button class="nx-btn text-xs h-7 px-2 text-nx-danger"
                          onclick={() => removeNetwork(n.name)} disabled={actionLoading === n.name}>
                          {t("docker.delete")}
                        </button>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            {/if}
          </div>
        </div>

      {:else if activeTab === "compose"}
        <div>
          <div class="mb-4 grid grid-cols-2 gap-3">
            <div>
              <label for="compose-file" class="mb-1.5 block text-xs text-nx-text-muted">{t("docker.compose_file")}</label>
              <input id="compose-file" type="text" bind:value={composeFile} placeholder="docker-compose.yml"
                class="nx-input w-full h-9 text-sm" />
            </div>
            <div>
              <label for="compose-project" class="mb-1.5 block text-xs text-nx-text-muted">{t("docker.compose_project")}</label>
              <input id="compose-project" type="text" bind:value={composeProject} placeholder={t("docker.compose_project_ph")}
                class="nx-input w-full h-9 text-sm" />
            </div>
          </div>

          <div class="mb-4 flex items-center gap-2">
            <button class="nx-btn text-xs h-8 text-nx-success" onclick={composeUp} disabled={composeLoading}>
              <span class="material-symbols-outlined text-sm">play_arrow</span>{t("docker.compose_up")}
            </button>
            <button class="nx-btn text-xs h-8 text-nx-danger" onclick={composeDown} disabled={composeLoading}>
              <span class="material-symbols-outlined text-sm">stop</span>{t("docker.compose_down")}
            </button>
            <button class="nx-btn text-xs h-8" onclick={composePs} disabled={composeLoading}>
              <span class="material-symbols-outlined text-sm">list</span>{t("docker.compose_ps")}
            </button>
            <button class="nx-btn text-xs h-8" onclick={composeViewLogs} disabled={composeLoading}>
              <span class="material-symbols-outlined text-sm">list_alt</span>{t("docker.compose_logs")}
            </button>
          </div>

          {#if composeError}
            <div class="mb-4 nx-section">
              <div class="nx-section-body">
                <pre class="font-mono text-xs text-nx-danger whitespace-pre-wrap">{composeError}</pre>
              </div>
            </div>
          {/if}

          {#if composeContainers.length > 0}
            <div class="mb-4 nx-section">
              <div class="nx-section-header">
                <span class="text-xs font-medium uppercase tracking-wider text-nx-text-muted">{t("docker.compose_services")}</span>
              </div>
              <table class="nx-table">
                <thead>
                  <tr>
                    <th>{t("docker.name")}</th>
                    <th>{t("docker.image")}</th>
                    <th>{t("docker.status")}</th>
                    <th>{t("docker.ports")}</th>
                  </tr>
                </thead>
                <tbody>
                  {#each composeContainers as c}
                    <tr>
                      <td class="text-sm text-nx-text">{c.name}</td>
                      <td class="font-mono text-xs text-nx-text-secondary">{c.image}</td>
                      <td>
                        <span class="inline-flex items-center gap-1 text-xs {c.status === 'running' ? 'text-nx-success' : 'text-nx-text-muted'}">
                          <ContainerIcons name={c.status === 'running' ? 'container-running' : 'container-exited'} size={12} />
                          {statusLabel(c.status)}
                        </span>
                      </td>
                      <td class="font-mono text-xs text-nx-text-muted">{c.ports || "-"}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}

          {#if composeLogs}
            <div class="nx-section">
              <div class="nx-section-header">
                <span class="text-xs font-medium uppercase tracking-wider text-nx-text-muted">{t("docker.logs")}</span>
                <button class="text-nx-text-muted hover:text-nx-text" onclick={() => { composeLogs = ""; }}>
                  <span class="material-symbols-outlined text-sm">close</span>
                </button>
              </div>
              <pre class="max-h-[400px] overflow-auto p-4 font-mono text-xs text-nx-text-secondary whitespace-pre-wrap">{composeLogs}</pre>
            </div>
          {/if}
        </div>
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

{#if showPull}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
    <div class="w-full max-w-[300px] bg-nx-surface border border-nx-border rounded-lg shadow-xl overflow-hidden">
      <div class="flex items-center justify-between px-3 py-2 border-b border-nx-border">
        <h2 class="text-sm font-semibold text-nx-text">{t("docker.pull_image")}</h2>
        <button class="text-nx-text-muted hover:text-nx-text" onclick={() => { showPull = false; pullImageName = ""; }}>
          <span class="material-symbols-outlined text-lg">close</span>
        </button>
      </div>
      <div class="p-3">
        <input id="pull-image" type="text" bind:value={pullImageName} placeholder="nginx:latest"
          class="nx-input w-full h-8 text-xs"
          onkeydown={(e) => { if (e.key === 'Enter') pullImageAction(); }} />
        <div class="mt-3 flex justify-end gap-2">
          <button class="nx-btn h-7 text-xs" onclick={() => { showPull = false; pullImageName = ""; }}>{t("common.cancel")}</button>
          <button class="nx-btn nx-btn-primary h-7 text-xs" onclick={pullImageAction} disabled={pullLoading || !pullImageName.trim()}>
            {pullLoading ? t("docker.pulling") : t("docker.pull")}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if showBuild}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
    <div class="w-full max-w-[300px] bg-nx-surface border border-nx-border rounded-lg shadow-xl overflow-hidden">
      <div class="flex items-center justify-between px-3 py-2 border-b border-nx-border">
        <h2 class="text-sm font-semibold text-nx-text">{t("docker.build_image")}</h2>
        <button class="text-nx-text-muted hover:text-nx-text" onclick={() => { showBuild = false; buildTag = ""; buildPath = ""; }}>
          <span class="material-symbols-outlined text-lg">close</span>
        </button>
      </div>
      <div class="p-3 space-y-2">
        <input id="build-tag" type="text" bind:value={buildTag} placeholder={t("docker.build_tag") + " (myapp:latest)"}
          class="nx-input w-full h-8 text-xs" />
        <input id="build-path" type="text" bind:value={buildPath} placeholder={t("docker.build_path") + " (.)"}
          class="nx-input w-full h-8 text-xs" />
        <div class="flex justify-end gap-2 pt-1">
          <button class="nx-btn h-7 text-xs" onclick={() => { showBuild = false; buildTag = ""; buildPath = ""; }}>{t("common.cancel")}</button>
          <button class="nx-btn nx-btn-primary h-7 text-xs" onclick={buildImageAction} disabled={buildLoading || !buildTag.trim() || !buildPath.trim()}>
            {buildLoading ? t("docker.building") : t("docker.build")}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if showPush}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
    <div class="w-full max-w-[300px] bg-nx-surface border border-nx-border rounded-lg shadow-xl overflow-hidden">
      <div class="flex items-center justify-between px-3 py-2 border-b border-nx-border">
        <h2 class="text-sm font-semibold text-nx-text">{t("docker.push_image")}</h2>
        <button class="text-nx-text-muted hover:text-nx-text" onclick={() => { showPush = false; pushTarget = ""; }}>
          <span class="material-symbols-outlined text-lg">close</span>
        </button>
      </div>
      <div class="p-3">
        <input id="push-target" type="text" bind:value={pushTarget} placeholder="registry/user/repo:tag"
          class="nx-input w-full h-8 text-xs"
          onkeydown={(e) => { if (e.key === 'Enter') pushImageAction(); }} />
        <div class="mt-3 flex justify-end gap-2">
          <button class="nx-btn h-7 text-xs" onclick={() => { showPush = false; pushTarget = ""; }}>{t("common.cancel")}</button>
          <button class="nx-btn nx-btn-primary h-7 text-xs" onclick={pushImageAction} disabled={pushLoading || !pushTarget.trim()}>
            {pushLoading ? t("docker.pushing") : t("docker.push")}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if showTag}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
    <div class="w-full max-w-[300px] bg-nx-surface border border-nx-border rounded-lg shadow-xl overflow-hidden">
      <div class="flex items-center justify-between px-3 py-2 border-b border-nx-border">
        <h2 class="text-sm font-semibold text-nx-text">{t("docker.tag_image")}</h2>
        <button class="text-nx-text-muted hover:text-nx-text" onclick={() => { showTag = false; tagValue = ""; }}>
          <span class="material-symbols-outlined text-lg">close</span>
        </button>
      </div>
      <div class="p-3">
        <input id="tag-value" type="text" bind:value={tagValue} placeholder="registry/user/repo:tag"
          class="nx-input w-full h-8 text-xs"
          onkeydown={(e) => { if (e.key === 'Enter') tagImageAction(); }} />
        <div class="mt-3 flex justify-end gap-2">
          <button class="nx-btn h-7 text-xs" onclick={() => { showTag = false; tagValue = ""; }}>{t("common.cancel")}</button>
          <button class="nx-btn nx-btn-primary h-7 text-xs" onclick={tagImageAction} disabled={tagLoading || !tagValue.trim()}>
            {tagLoading ? t("docker.tagging") : t("docker.tag")}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if showCreateVolume}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
    <div class="w-full max-w-[300px] bg-nx-surface border border-nx-border rounded-lg shadow-xl overflow-hidden">
      <div class="flex items-center justify-between px-3 py-2 border-b border-nx-border">
        <h2 class="text-sm font-semibold text-nx-text">{t("docker.create_volume")}</h2>
        <button class="text-nx-text-muted hover:text-nx-text" onclick={() => { showCreateVolume = false; newVolumeName = ""; }}>
          <span class="material-symbols-outlined text-lg">close</span>
        </button>
      </div>
      <div class="p-3">
        <input id="volume-name" type="text" bind:value={newVolumeName} placeholder="my_volume"
          class="nx-input w-full h-8 text-xs"
          onkeydown={(e) => { if (e.key === 'Enter') createVolume(); }} />
        <div class="mt-3 flex justify-end gap-2">
          <button class="nx-btn h-7 text-xs" onclick={() => { showCreateVolume = false; newVolumeName = ""; }}>{t("common.cancel")}</button>
          <button class="nx-btn nx-btn-primary h-7 text-xs" onclick={createVolume} disabled={!newVolumeName.trim()}>{t("docker.create")}</button>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if showCreateNetwork}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
    <div class="w-full max-w-[300px] bg-nx-surface border border-nx-border rounded-lg shadow-xl overflow-hidden">
      <div class="flex items-center justify-between px-3 py-2 border-b border-nx-border">
        <h2 class="text-sm font-semibold text-nx-text">{t("docker.create_network")}</h2>
        <button class="text-nx-text-muted hover:text-nx-text" onclick={() => { showCreateNetwork = false; newNetworkName = ""; }}>
          <span class="material-symbols-outlined text-lg">close</span>
        </button>
      </div>
      <div class="p-3">
        <input id="network-name" type="text" bind:value={newNetworkName} placeholder="my_network"
          class="nx-input w-full h-8 text-xs"
          onkeydown={(e) => { if (e.key === 'Enter') createNetwork(); }} />
        <div class="mt-3 flex justify-end gap-2">
          <button class="nx-btn h-7 text-xs" onclick={() => { showCreateNetwork = false; newNetworkName = ""; }}>{t("common.cancel")}</button>
          <button class="nx-btn nx-btn-primary h-7 text-xs" onclick={createNetwork} disabled={!newNetworkName.trim()}>{t("docker.create")}</button>
        </div>
      </div>
    </div>
  </div>
{/if}
