<script>
  import { onMount } from "svelte";
  import { t, initI18n, getLang } from "../lib/i18n.js";

  let theme = $state("dark");
  let lang = $state(getLang());
  let compactMode = $state(false);
  let buildAlerts = $state(true);
  let securityNotices = $state(true);
  let proxyEnabled = $state(false);
  let proxyAddress = $state("");
  let proxyPort = $state("");

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

  onMount(() => {
    const saved = localStorage.getItem("devnexus-theme") || "dark";
    theme = saved === "light" || saved === "dark" ? saved : "system";
    setTheme(theme);
  });
</script>

<div class="mx-auto max-w-2xl">
  <h1 class="mb-6 text-xl font-semibold text-nx-text">Settings</h1>

  <!-- Appearance -->
  <div class="mb-6 border border-nx-border bg-nx-surface">
    <div class="border-b border-nx-border px-5 py-3">
      <h3 class="text-sm font-medium text-nx-text">Appearance</h3>
    </div>
    <div class="p-5 space-y-5">
      <!-- Theme -->
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm text-nx-text">Theme</div>
          <div class="text-xs text-nx-text-muted">Select your preferred theme</div>
        </div>
        <div class="flex border border-nx-border">
          <button
            class="px-3 py-1.5 text-xs font-medium {theme === 'light' ? 'bg-nx-raised text-nx-text' : 'text-nx-text-secondary'}"
            onclick={() => setTheme("light")}>
            Light
          </button>
          <button
            class="border-l border-r border-nx-border px-3 py-1.5 text-xs font-medium {theme === 'dark' ? 'bg-nx-raised text-nx-text' : 'text-nx-text-secondary'}"
            onclick={() => setTheme("dark")}>
            Dark
          </button>
          <button
            class="px-3 py-1.5 text-xs font-medium {theme === 'system' ? 'bg-nx-raised text-nx-text' : 'text-nx-text-secondary'}"
            onclick={() => setTheme("system")}>
            System
          </button>
        </div>
      </div>

      <div class="h-px bg-nx-border"></div>

      <!-- Compact Mode -->
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm text-nx-text">Compact Mode</div>
          <div class="text-xs text-nx-text-muted">Reduce padding and spacing</div>
        </div>
        <button
          class="relative h-5 w-9 {compactMode ? 'bg-nx-success' : 'bg-nx-border'}"
          onclick={() => compactMode = !compactMode}>
          <span class="absolute top-0.5 h-4 w-4 bg-white {compactMode ? 'left-[18px]' : 'left-0.5'}"></span>
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
          onchange={(e) => setLang(e.target.value)}
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
      <h3 class="text-sm font-medium text-nx-text">Notifications</h3>
    </div>
    <div class="p-5 space-y-5">
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm text-nx-text">Build Alerts</div>
          <div class="text-xs text-nx-text-muted">Notify on build failures</div>
        </div>
        <button
          class="relative h-5 w-9 {buildAlerts ? 'bg-nx-text' : 'bg-nx-border'}"
          onclick={() => buildAlerts = !buildAlerts}>
          <span class="absolute top-0.5 h-4 w-4 bg-nx-text-secondary {buildAlerts ? 'left-[18px]' : 'left-0.5'}"></span>
        </button>
      </div>

      <div class="h-px bg-nx-border"></div>

      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm text-nx-text">Security Notices</div>
          <div class="text-xs text-nx-text-muted">Vulnerability alerts</div>
        </div>
        <button
          class="relative h-5 w-9 {securityNotices ? 'bg-nx-text' : 'bg-nx-border'}"
          onclick={() => securityNotices = !securityNotices}>
          <span class="absolute top-0.5 h-4 w-4 bg-nx-text-secondary {securityNotices ? 'left-[18px]' : 'left-0.5'}"></span>
        </button>
      </div>
    </div>
  </div>

  <!-- Network Proxy -->
  <div class="mb-6 border border-nx-border bg-nx-surface">
    <div class="border-b border-nx-border px-5 py-3">
      <h3 class="text-sm font-medium text-nx-text">Network Proxy</h3>
    </div>
    <div class="p-5 space-y-5">
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm text-nx-text">Enable Proxy</div>
          <div class="text-xs text-nx-text-muted">Route traffic through a proxy server</div>
        </div>
        <button
          class="relative h-5 w-9 {proxyEnabled ? 'bg-nx-text' : 'bg-nx-border'}"
          onclick={() => proxyEnabled = !proxyEnabled}>
          <span class="absolute top-0.5 h-4 w-4 bg-nx-text-secondary {proxyEnabled ? 'left-[18px]' : 'left-0.5'}"></span>
        </button>
      </div>

      {#if proxyEnabled}
        <div class="grid grid-cols-3 gap-3">
          <div class="col-span-2">
            <label class="mb-1 block text-xs text-nx-text-muted">Proxy Address</label>
            <input
              type="text"
              bind:value={proxyAddress}
              placeholder="127.0.0.1"
              class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary"
            />
          </div>
          <div>
            <label class="mb-1 block text-xs text-nx-text-muted">Port</label>
            <input
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
      <h3 class="text-sm font-medium text-nx-text">Updates</h3>
    </div>
    <div class="p-5">
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm text-nx-text">Current Version</div>
          <div class="font-mono text-xs text-nx-text-secondary">v1.0.0</div>
        </div>
        <button class="border border-nx-border bg-nx-bg px-4 py-2 text-xs font-medium text-nx-text-secondary">
          Check for Updates
        </button>
      </div>
      <div class="mt-3 flex items-center gap-2">
        <span class="material-symbols-outlined text-nx-text-secondary text-sm">check_circle</span>
        <span class="text-xs text-nx-text-secondary">System is up to date</span>
      </div>
    </div>
  </div>
</div>
