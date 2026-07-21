<script>
  import { onMount } from "svelte";
  import { t, initI18n, getLang } from "../lib/i18n.svelte.js";
  import { invoke } from "@tauri-apps/api/core";
  import { navigate } from "../lib/stores.svelte.js";

  let theme = $state("dark");
  let lang = $state(getLang());
  let compactMode = $state(false);
  let buildAlerts = $state(true);
  let securityNotices = $state(true);
  let proxyEnabled = $state(false);
  let proxyAddress = $state("");
  let proxyPort = $state("");
  let appVersion = $state("");

  let updateState = $state("idle");
  function isState(v) { return updateState === v; }
  let updateInfo = $state(null);
  let updateError = $state("");
  let changelogEn = $state("");
  let changelogZh = $state("");
  let downloadProgress = $state(0);

  function applyTheme(t) {
    document.documentElement.setAttribute("data-theme", t);
    localStorage.setItem("devnexus-theme", t);
  }

  function setTheme(t) {
    theme = t;
    if (t === "system") {
      const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      applyTheme(prefersDark ? "dark" : "light");
    } else {
      applyTheme(t);
    }
  }

  async function setLang(l) {
    lang = l;
    await initI18n(l);
  }

  async function checkForUpdates() {
    updateState = "checking";
    updateError = "";
    updateInfo = null;
    changelogEn = "";
    changelogZh = "";
    try {
      const result = await invoke("check_for_updates_github");
      if (result.has_update) {
        updateInfo = result;
        const cl = await invoke("get_changelog", { version: null });
        if (cl) {
          changelogEn = cl.en;
          changelogZh = cl.zh;
        }
        updateState = "available";
      } else {
        updateState = "up_to_date";
      }
    } catch (err) {
      updateError = err.message || String(err);
      updateState = "error";
    }
  }

  async function downloadAndInstall() {
    updateState = "downloading";
    downloadProgress = 0;
    updateError = "";
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      if (update) {
        let totalBytes = 0;
        let downloadedBytes = 0;
        await update.downloadAndInstall((event) => {
          if (event.event === "Started" && event.data.contentLength) totalBytes = event.data.contentLength;
          else if (event.event === "Progress") { downloadedBytes += event.data.chunkLength; downloadProgress = totalBytes > 0 ? Math.round((downloadedBytes / totalBytes) * 100) : 0; }
          else if (event.event === "Finished") downloadProgress = 100;
        });
        updateState = "installed";
        return;
      }
    } catch (_) {}

    try {
      const url = await invoke("get_download_url", { version: updateInfo?.latest_version || "" });
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(url);
      updateState = "opened";
    } catch (fallbackErr) {
      try {
        const { open } = await import("@tauri-apps/plugin-shell");
        await open(updateInfo?.html_url || `https://github.com/linanwanttodo/DevNexus/releases/latest`);
        updateState = "opened";
      } catch (e) {
        updateError = e.message || String(e);
        updateState = "error";
      }
    }
  }

  async function restartApp() {
    try {
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch (e) { updateError = e.message || String(e); }
  }

  onMount(() => {
    const saved = localStorage.getItem("devnexus-theme") || "dark";
    theme = saved === "light" || saved === "dark" ? saved : "system";
    setTheme(theme);
    invoke("get_app_version").then(v => appVersion = v).catch(() => appVersion = "1.1.1");
  });
</script>

<div class="nx-page flex h-full flex-col">
  <!-- Header with back button -->
  <div class="flex items-center gap-2 border-b border-nx-border px-5 py-2.5">
    <button class="nx-back-btn" onclick={() => navigate("/dashboard")}>
      <span class="material-symbols-outlined text-lg">arrow_back</span>
      {t("nav.dashboard")}
    </button>
    <span class="text-xs text-nx-text-muted">/</span>
    <h1 class="text-sm font-medium text-nx-text">{t("settings.title")}</h1>
  </div>

  <!-- Content -->
  <div class="flex-1 overflow-y-auto">
    <div class="mx-auto max-w-2xl px-5 py-6 space-y-4">
      <!-- Appearance -->
      <div class="nx-section">
        <div class="nx-section-header">
          <h3 class="text-sm font-medium text-nx-text">{t("settings.appearance")}</h3>
        </div>
        <div class="nx-section-body space-y-4">
          <!-- Theme -->
          <div class="flex items-center justify-between">
            <div>
              <div class="text-sm text-nx-text">{t("settings.theme")}</div>
              <div class="text-xs text-nx-text-muted">{t("settings.theme_desc")}</div>
            </div>
            <div class="flex rounded-md border border-nx-border overflow-hidden bg-nx-raised">
              <button
                class="px-3 py-1.5 text-xs font-medium transition-colors {theme === 'light' ? 'bg-nx-overlay text-nx-text shadow-sm' : 'text-nx-text-secondary hover:text-nx-text'}"
                onclick={() => setTheme("light")}>{t("settings.light")}</button>
              <button
                class="border-x border-nx-border px-3 py-1.5 text-xs font-medium transition-colors {theme === 'dark' ? 'bg-nx-overlay text-nx-text shadow-sm' : 'text-nx-text-secondary hover:text-nx-text'}"
                onclick={() => setTheme("dark")}>{t("settings.dark")}</button>
              <button
                class="px-3 py-1.5 text-xs font-medium transition-colors {theme === 'system' ? 'bg-nx-overlay text-nx-text shadow-sm' : 'text-nx-text-secondary hover:text-nx-text'}"
                onclick={() => setTheme("system")}>{t("settings.system")}</button>
            </div>
          </div>

          <div class="nx-divider"></div>

          <!-- Compact Mode -->
          <div class="flex items-center justify-between">
            <div>
              <div class="text-sm text-nx-text">{t("settings.compact_mode")}</div>
              <div class="text-xs text-nx-text-muted">{t("settings.compact_mode_desc")}</div>
            </div>
            <button
              class="nx-toggle"
              role="switch"
              aria-label={t("settings.compact_mode")}
              aria-checked={compactMode}
              onclick={() => compactMode = !compactMode}>
            </button>
          </div>
        </div>
      </div>

      <!-- Language -->
      <div class="nx-section">
        <div class="nx-section-header">
          <h3 class="text-sm font-medium text-nx-text">{t("settings.language")}</h3>
        </div>
        <div class="nx-section-body">
          <div class="flex items-center justify-between">
            <div>
              <div class="text-sm text-nx-text">{t("settings.language_desc")}</div>
            </div>
            <select
              value={lang}
              onchange={(e) => setLang(e.currentTarget.value)}
              class="nx-input h-9 text-sm min-w-[130px]"
            >
              <option value="en">English</option>
              <option value="zh">中文</option>
              <option value="ru">Русский</option>
            </select>
          </div>
        </div>
      </div>

      <!-- Notifications -->
      <div class="nx-section">
        <div class="nx-section-header">
          <h3 class="text-sm font-medium text-nx-text">{t("settings.notifications")}</h3>
        </div>
        <div class="nx-section-body space-y-4">
          <div class="flex items-center justify-between">
            <div>
              <div class="text-sm text-nx-text">{t("settings.build_alerts")}</div>
              <div class="text-xs text-nx-text-muted">{t("settings.build_alerts_desc")}</div>
            </div>
            <button class="nx-toggle" role="switch" aria-label={t("settings.build_alerts")} aria-checked={buildAlerts} onclick={() => buildAlerts = !buildAlerts}></button>
          </div>
          <div class="nx-divider"></div>
          <div class="flex items-center justify-between">
            <div>
              <div class="text-sm text-nx-text">{t("settings.security_notices")}</div>
              <div class="text-xs text-nx-text-muted">{t("settings.security_notices_desc")}</div>
            </div>
            <button class="nx-toggle" role="switch" aria-label={t("settings.security_notices")} aria-checked={securityNotices} onclick={() => securityNotices = !securityNotices}></button>
          </div>
        </div>
      </div>

      <!-- Network Proxy -->
      <div class="nx-section">
        <div class="nx-section-header">
          <h3 class="text-sm font-medium text-nx-text">{t("settings.network_proxy")}</h3>
        </div>
        <div class="nx-section-body space-y-4">
          <div class="flex items-center justify-between">
            <div>
              <div class="text-sm text-nx-text">{t("settings.enable_proxy")}</div>
              <div class="text-xs text-nx-text-muted">{t("settings.enable_proxy_desc")}</div>
            </div>
            <button class="nx-toggle" role="switch" aria-label={t("settings.enable_proxy")} aria-checked={proxyEnabled} onclick={() => proxyEnabled = !proxyEnabled}></button>
          </div>

          {#if proxyEnabled}
            <div class="grid grid-cols-3 gap-3">
              <div class="col-span-2">
                <label class="mb-1.5 block text-xs text-nx-text-muted" for="proxy-address">{t("settings.proxy_address")}</label>
                <input id="proxy-address" type="text" bind:value={proxyAddress} placeholder="127.0.0.1"
                  class="nx-input w-full h-9 text-sm" />
              </div>
              <div>
                <label class="mb-1.5 block text-xs text-nx-text-muted" for="proxy-port">{t("settings.port")}</label>
                <input id="proxy-port" type="text" bind:value={proxyPort} placeholder="7890"
                  class="nx-input w-full h-9 text-sm" />
              </div>
            </div>
          {/if}
        </div>
      </div>

      <!-- Updates -->
      <div class="nx-section">
        <div class="nx-section-header">
          <h3 class="text-sm font-medium text-nx-text">{t("settings.updates")}</h3>
        </div>
        <div class="nx-section-body">
          <div class="flex items-center justify-between">
            <div>
              <div class="text-sm text-nx-text">{t("settings.current_version")}</div>
              <div class="font-mono text-xs text-nx-text-secondary">v{appVersion || "—"}</div>
            </div>
            <button class="nx-btn nx-btn-ghost h-8 text-xs"
              onclick={checkForUpdates}
              disabled={isState('checking') || isState('downloading')}>
              {#if isState('checking')}
                <span class="material-symbols-outlined text-sm nx-animate-spin">refresh</span>
                {t("settings.checking")}
              {:else}
                {t("settings.check_updates")}
              {/if}
            </button>
          </div>

          <div class="mt-3">
            {#if isState('checking')}
              <div class="flex items-center gap-2"><span class="material-symbols-outlined text-nx-text-secondary text-sm nx-animate-spin">refresh</span><span class="text-xs text-nx-text-secondary">{t("settings.checking")}...</span></div>
            {:else if isState('up_to_date')}
              <div class="flex items-center gap-2"><span class="material-symbols-outlined text-nx-success text-sm">check_circle</span><span class="text-xs text-nx-text-secondary">{t("settings.up_to_date")}</span></div>
            {:else if isState('available')}
              <div class="mt-2 rounded-lg border border-nx-border bg-nx-bg p-3">
                <div class="flex items-start gap-2">
                  <span class="material-symbols-outlined text-nx-accent text-sm mt-0.5">system_update</span>
                  <div class="flex-1 min-w-0">
                    <div class="text-sm font-medium text-nx-text">{t("settings.update_available")} {updateInfo?.latest_version}</div>
                    {#if changelogEn || changelogZh}
                      <div class="mt-1 space-y-1">
                        {#if changelogEn}
                          <div class="text-xs font-medium text-nx-text mt-1">English</div>
                          <div class="max-h-24 overflow-y-auto text-xs text-nx-text-secondary whitespace-pre-wrap leading-relaxed">{changelogEn}</div>
                        {/if}
                        {#if changelogZh}
                          <div class="text-xs font-medium text-nx-text mt-2">中文</div>
                          <div class="max-h-24 overflow-y-auto text-xs text-nx-text-secondary whitespace-pre-wrap leading-relaxed">{changelogZh}</div>
                        {/if}
                      </div>
                    {:else if updateInfo?.release_notes}
                      <div class="mt-1 max-h-20 overflow-y-auto text-xs text-nx-text-secondary whitespace-pre-wrap">{updateInfo.release_notes}</div>
                    {/if}
                    {#if updateInfo?.published_at}<div class="mt-1 text-xs text-nx-text-muted">{t("settings.released")}: {new Date(updateInfo.published_at).toLocaleDateString()}</div>{/if}
                  </div>
                </div>
                <button class="nx-btn nx-btn-primary w-full mt-3 h-8 text-xs" onclick={downloadAndInstall}>
                  <span class="material-symbols-outlined text-sm">download</span>{t("settings.download_update")}
                </button>
              </div>
            {:else if isState('downloading')}
              <div class="flex flex-col gap-2">
                <div class="flex items-center gap-2">
                  <span class="material-symbols-outlined text-nx-accent text-sm nx-animate-spin">progress_activity</span>
                  <span class="text-xs text-nx-text-secondary">{t("settings.downloading")}...</span>
                  {#if downloadProgress > 0}<span class="text-[10px] text-nx-text-muted tabular-nums">{downloadProgress}%</span>{/if}
                </div>
                <div class="nx-progress">
                  <div class="nx-progress-bar accent transition-all" style="width: {downloadProgress}%"></div>
                </div>
              </div>
            {:else if isState('installed')}
              <div class="flex items-center gap-2">
                <span class="material-symbols-outlined text-nx-success text-sm">check_circle</span>
                <span class="text-xs text-nx-text-secondary">{t("settings.update_installed")}</span>
                <button class="nx-btn text-xs h-6 px-2 text-nx-success" onclick={restartApp}>{t("settings.restart_now")}</button>
              </div>
            {:else if isState('error')}
              <div class="flex items-center gap-2"><span class="material-symbols-outlined text-nx-danger text-sm">error</span><span class="text-xs text-nx-danger">{t("settings.update_error")}: {updateError}</span></div>
            {:else if isState('opened')}
              <div class="flex items-center gap-2"><span class="material-symbols-outlined text-nx-success text-sm">check_circle</span><span class="text-xs text-nx-text-secondary">{t("settings.download_opened")}</span></div>
            {:else}
              <div class="flex items-center gap-2"><span class="material-symbols-outlined text-nx-text-muted text-sm">info</span><span class="text-xs text-nx-text-muted">{t("settings.click_to_check")}</span></div>
            {/if}
          </div>
        </div>
      </div>
    </div>
  </div>
</div>
