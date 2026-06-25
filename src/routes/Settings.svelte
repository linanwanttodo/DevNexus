<script>
  import { onMount } from "svelte";
  import { t, initI18n, getLang } from "../lib/i18n.svelte.js";
  import { invoke } from "@tauri-apps/api/core";

  let theme = $state("dark");
  let lang = $state(getLang());
  let compactMode = $state(false);
  let buildAlerts = $state(true);
  let securityNotices = $state(true);
  let proxyEnabled = $state(false);
  let proxyAddress = $state("");
  let proxyPort = $state("");
  let appVersion = $state("");

  // Update state
  let updateState = $state("idle");
  // Helper to avoid Svelte 5 template type narrowing (TypeScript narrows inside {#if} blocks)
  function isState(v) { return updateState === v; }
  let updateInfo = $state(null);
  let updateError = $state("");
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

    // 直接使用 GitHub API 检测更新（Tauri 插件需要 valid 签名文件，容易静默失败）
    try {
      const result = await invoke("check_for_updates_github");
      if (result.has_update) {
        updateInfo = result;
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
      // 优先使用 Tauri updater 插件（需要 CI 构建时生成有效的 updates.json）
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();

      if (update) {
        let totalBytes = 0;
        let downloadedBytes = 0;
        await update.downloadAndInstall((event) => {
          if (event.event === "Started" && event.data.contentLength) {
            totalBytes = event.data.contentLength;
          } else if (event.event === "Progress") {
            downloadedBytes += event.data.chunkLength;
            downloadProgress = totalBytes > 0
              ? Math.round((downloadedBytes / totalBytes) * 100)
              : 0;
          } else if (event.event === "Finished") {
            downloadProgress = 100;
          }
        });
        updateState = "installed";
        return;
      }
    } catch (_) {
      // 插件失败（签名未配置、updates.json 缺失等）— 走 fallback
    }

    // Fallback: 调用后端获取当前平台下载链接，用浏览器打开
    try {
      const url = await invoke("get_download_url", { version: updateInfo?.latest_version || "" });
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(url);
      updateState = "opened";
    } catch (fallbackErr) {
      // 最后的兜底：打开 GitHub Release 页
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
    } catch (e) {
      updateError = e.message || String(e);
    }
  }

  onMount(() => {
    const saved = localStorage.getItem("devnexus-theme") || "dark";
    theme = saved === "light" || saved === "dark" ? saved : "system";
    setTheme(theme);
    // 动态获取应用版本号
    invoke("get_app_version").then(v => appVersion = v).catch(() => appVersion = "1.0.2");
  });
</script>

<div class="mx-auto max-w-2xl">
  <h1 class="mb-6 text-xl font-semibold text-nx-text">{t("settings.title")}</h1>

  <!-- Appearance -->
  <div class="mb-6 border border-nx-border bg-nx-surface">
    <div class="border-b border-nx-border px-5 py-3">
      <h3 class="text-sm font-medium text-nx-text">{t("settings.appearance")}</h3>
    </div>
    <div class="p-5 space-y-5">
      <!-- Theme -->
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm text-nx-text">{t("settings.theme")}</div>
          <div class="text-xs text-nx-text-muted">{t("settings.theme_desc")}</div>
        </div>
        <div class="flex border border-nx-border">
          <button
            class="px-3 py-1.5 text-xs font-medium {theme === 'light' ? 'bg-nx-raised text-nx-text' : 'text-nx-text-secondary'}"
            onclick={() => setTheme("light")}>
            {t("settings.light")}
          </button>
          <button
            class="border-l border-r border-nx-border px-3 py-1.5 text-xs font-medium {theme === 'dark' ? 'bg-nx-raised text-nx-text' : 'text-nx-text-secondary'}"
            onclick={() => setTheme("dark")}>
            {t("settings.dark")}
          </button>
          <button
            class="px-3 py-1.5 text-xs font-medium {theme === 'system' ? 'bg-nx-raised text-nx-text' : 'text-nx-text-secondary'}"
            onclick={() => setTheme("system")}>
            {t("settings.system")}
          </button>
        </div>
      </div>

      <div class="h-px bg-nx-border"></div>

      <!-- Compact Mode -->
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm text-nx-text">{t("settings.compact_mode")}</div>
          <div class="text-xs text-nx-text-muted">{t("settings.compact_mode_desc")}</div>
        </div>
        <button
          class="relative h-5 w-9 rounded-full transition-colors {compactMode ? 'bg-nx-accent' : 'bg-nx-border'}"
          onclick={() => compactMode = !compactMode}
          aria-label={t("settings.compact_mode")}>
          <span class="absolute top-0.5 h-4 w-4 rounded-full bg-white transition-all {compactMode ? 'left-[18px]' : 'left-0.5'}"></span>
        </button>
      </div>
    </div>
  </div>

  <!-- Language -->
  <div class="mb-6 border border-nx-border bg-nx-surface">
    <div class="border-b border-nx-border px-5 py-3">
      <h3 class="text-sm font-medium text-nx-text">{t("settings.language")}</h3>
    </div>
    <div class="p-5">
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm text-nx-text">{t("settings.language_desc")}</div>
        </div>
        <select
          value={lang}
          onchange={(e) => setLang(e.currentTarget.value)}
          class="border border-nx-border bg-nx-surface px-3 py-1.5 text-sm text-nx-text outline-none"
        >
          <option value="en">English</option>
          <option value="zh">中文</option>
          <option value="ru">Русский</option>
        </select>
      </div>
    </div>
  </div>

  <!-- Notifications -->
  <div class="mb-6 border border-nx-border bg-nx-surface">
    <div class="border-b border-nx-border px-5 py-3">
      <h3 class="text-sm font-medium text-nx-text">{t("settings.notifications")}</h3>
    </div>
    <div class="p-5 space-y-5">
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm text-nx-text">{t("settings.build_alerts")}</div>
          <div class="text-xs text-nx-text-muted">{t("settings.build_alerts_desc")}</div>
        </div>
        <button
          class="relative h-5 w-9 rounded-full transition-colors {buildAlerts ? 'bg-nx-accent' : 'bg-nx-border'}"
          onclick={() => buildAlerts = !buildAlerts}
          aria-label={t("settings.build_alerts")}>
          <span class="absolute top-0.5 h-4 w-4 rounded-full bg-white transition-all {buildAlerts ? 'left-[18px]' : 'left-0.5'}"></span>
        </button>
      </div>

      <div class="h-px bg-nx-border"></div>

      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm text-nx-text">{t("settings.security_notices")}</div>
          <div class="text-xs text-nx-text-muted">{t("settings.security_notices_desc")}</div>
        </div>
        <button
          class="relative h-5 w-9 rounded-full transition-colors {securityNotices ? 'bg-nx-accent' : 'bg-nx-border'}"
          onclick={() => securityNotices = !securityNotices}
          aria-label={t("settings.security_notices")}>
          <span class="absolute top-0.5 h-4 w-4 rounded-full bg-white transition-all {securityNotices ? 'left-[18px]' : 'left-0.5'}"></span>
        </button>
      </div>
    </div>
  </div>

  <!-- Network Proxy -->
  <div class="mb-6 border border-nx-border bg-nx-surface">
    <div class="border-b border-nx-border px-5 py-3">
      <h3 class="text-sm font-medium text-nx-text">{t("settings.network_proxy")}</h3>
    </div>
    <div class="p-5 space-y-5">
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm text-nx-text">{t("settings.enable_proxy")}</div>
          <div class="text-xs text-nx-text-muted">{t("settings.enable_proxy_desc")}</div>
        </div>
        <button
          class="relative h-5 w-9 rounded-full transition-colors {proxyEnabled ? 'bg-nx-accent' : 'bg-nx-border'}"
          onclick={() => proxyEnabled = !proxyEnabled}
          aria-label={t("settings.enable_proxy")}>
          <span class="absolute top-0.5 h-4 w-4 rounded-full bg-white transition-all {proxyEnabled ? 'left-[18px]' : 'left-0.5'}"></span>
        </button>
      </div>

      {#if proxyEnabled}
        <div class="grid grid-cols-3 gap-3">
          <div class="col-span-2">
            <label class="mb-1 block text-xs text-nx-text-muted" for="proxy-address">{t("settings.proxy_address")}</label>
            <input
              id="proxy-address"
              type="text"
              bind:value={proxyAddress}
              placeholder="127.0.0.1"
              class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary"
            />
          </div>
          <div>
            <label class="mb-1 block text-xs text-nx-text-muted" for="proxy-port">{t("settings.port")}</label>
            <input
              id="proxy-port"
              type="text"
              bind:value={proxyPort}
              placeholder="7890"
              class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary"
            />
          </div>
        </div>
      {/if}
    </div>
  </div>

  <!-- Updates -->
  <div class="mb-6 border border-nx-border bg-nx-surface">
    <div class="border-b border-nx-border px-5 py-3">
      <h3 class="text-sm font-medium text-nx-text">{t("settings.updates")}</h3>
    </div>
    <div class="p-5">
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm text-nx-text">{t("settings.current_version")}</div>
          <div class="font-mono text-xs text-nx-text-secondary">v{appVersion || "—"}</div>
        </div>
        <button
          class="border border-nx-border bg-nx-bg px-4 py-2 text-xs font-medium text-nx-text-secondary transition-colors hover:bg-nx-raised disabled:opacity-50"
          onclick={checkForUpdates}
          disabled={isState('checking') || isState('downloading')}
        >
          {#if isState('checking')}
            <span class="flex items-center gap-2">
              <span class="material-symbols-outlined text-sm animate-spin">refresh</span>
              {t("settings.checking")}
            </span>
          {:else}
            {t("settings.check_updates")}
          {/if}
        </button>
      </div>

      <!-- Update Status -->
      <div class="mt-3">
        {#if isState('checking')}
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-nx-text-secondary text-sm animate-spin">refresh</span>
            <span class="text-xs text-nx-text-secondary">{t("settings.checking")}...</span>
          </div>
        {:else if isState('up_to_date')}
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-nx-success text-sm">check_circle</span>
            <span class="text-xs text-nx-text-secondary">{t("settings.up_to_date")}</span>
          </div>
        {:else if isState('available')}
          <div class="mt-2 rounded border border-nx-border bg-nx-bg p-3">
            <div class="flex items-start gap-2">
              <span class="material-symbols-outlined text-nx-accent text-sm mt-0.5">system_update</span>
              <div class="flex-1 min-w-0">
                <div class="text-sm font-medium text-nx-text">
                  {t("settings.update_available")} {updateInfo?.latest_version}
                </div>
                {#if updateInfo?.release_notes}
                  <div class="mt-1 max-h-20 overflow-y-auto text-xs text-nx-text-secondary whitespace-pre-wrap">
                    {updateInfo.release_notes}
                  </div>
                {/if}
                {#if updateInfo?.published_at}
                  <div class="mt-1 text-xs text-nx-text-muted">
                    {t("settings.released")}: {new Date(updateInfo.published_at).toLocaleDateString()}
                  </div>
                {/if}
              </div>
            </div>
            <button
              class="mt-3 w-full border border-nx-accent bg-nx-accent/10 px-4 py-2 text-xs font-medium text-nx-accent transition-colors hover:bg-nx-accent/20"
              onclick={downloadAndInstall}
            >
              <span class="flex items-center justify-center gap-2">
                <span class="material-symbols-outlined text-sm">download</span>
                {t("settings.download_update")}
              </span>
            </button>
          </div>
        {:else if isState('downloading')}
          <div class="flex flex-col gap-2">
            <div class="flex items-center gap-2">
              <span class="material-symbols-outlined text-nx-accent text-sm animate-spin">progress_activity</span>
              <span class="text-xs text-nx-text-secondary">{t("settings.downloading")}...</span>
              {#if downloadProgress > 0}
                <span class="text-[10px] text-nx-text-muted">{downloadProgress}%</span>
              {/if}
            </div>
            <div class="w-full bg-nx-bg-tertiary rounded-full h-1.5 overflow-hidden">
              <div
                class="bg-nx-accent h-1.5 rounded-full transition-all duration-300 ease-out"
                style="width: {downloadProgress}%"
              ></div>
            </div>
          </div>
        {:else if isState('installed')}
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-nx-success text-sm">check_circle</span>
            <span class="text-xs text-nx-text-secondary">{t("settings.update_installed")}</span>
            <button
              class="border border-nx-success bg-nx-success/10 px-2 py-0.5 text-[10px] font-medium text-nx-success transition-colors hover:bg-nx-success/20"
              onclick={restartApp}
            >
              {t("settings.restart_now")}
            </button>
          </div>
        {:else if isState('error')}
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-red-500 text-sm">error</span>
            <span class="text-xs text-red-500">{t("settings.update_error")}: {updateError}</span>
          </div>
        {:else if isState('opened')}
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-nx-success text-sm">check_circle</span>
            <span class="text-xs text-nx-text-secondary">{t("settings.download_opened")}</span>
          </div>
        {:else if isState('idle')}
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-nx-text-muted text-sm">info</span>
            <span class="text-xs text-nx-text-muted">{t("settings.click_to_check")}</span>
          </div>
        {:else}
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-nx-text-secondary text-sm">check_circle</span>
            <span class="text-xs text-nx-text-secondary">{t("settings.up_to_date")}</span>
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>
