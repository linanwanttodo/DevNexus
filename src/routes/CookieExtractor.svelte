<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let browsers = $state([]);
  let selectedBrowser = $state(null);
  let domainFilter = $state("");
  let cookies = $state([]);
  let loading = $state(false);
  let extracting = $state(false);

  async function loadBrowsers() {
    try {
      browsers = await invoke("get_supported_browsers");
      if (browsers.length > 0) {
        selectedBrowser = browsers[0].name;
      }
    } catch (err) {
      console.error("Failed to load browsers:", err);
      alert("Failed to detect browsers");
    }
  }

  async function extractCookies() {
    if (!selectedBrowser) {
      alert("Please select a browser");
      return;
    }

    extracting = true;
    try {
      const filter = domainFilter.trim() || null;
      cookies = await invoke("extract_cookies", {
        browserName: selectedBrowser,
        domainFilter: filter,
      });
    } catch (err) {
      alert(`Extraction failed: ${err.message || err}`);
      cookies = [];
    } finally {
      extracting = false;
    }
  }

  async function exportNetscape() {
    try {
      const filter = domainFilter.trim() || null;
      const content = await invoke("export_as_netscape", {
        browserName: selectedBrowser,
        domainFilter: filter,
      });
      
      downloadFile(content, `cookies_${selectedBrowser.toLowerCase()}.txt`, 'text/plain');
      alert("Exported as Netscape format");
    } catch (err) {
      alert(`Export failed: ${err.message || err}`);
    }
  }

  async function exportJSON() {
    try {
      const filter = domainFilter.trim() || null;
      const content = await invoke("export_as_json", {
        browserName: selectedBrowser,
        domainFilter: filter,
      });
      
      downloadFile(content, `cookies_${selectedBrowser.toLowerCase()}.json`, 'application/json');
      alert("Exported as JSON format");
    } catch (err) {
      alert(`Export failed: ${err.message || err}`);
    }
  }

  function downloadFile(content, filename, mimeType) {
    const blob = new Blob([content], { type: mimeType });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }

  function copyCookie(cookie) {
    const cookieString = `${cookie.name}=${cookie.value}`;
    navigator.clipboard.writeText(cookieString).then(() => {
      alert("Cookie copied to clipboard");
    });
  }

  function copyAllCookies() {
    const cookieStrings = cookies.map(c => `${c.name}=${c.value}`).join('; ');
    navigator.clipboard.writeText(cookieStrings).then(() => {
      alert("All cookies copied to clipboard");
    });
  }

  function formatDate(timestamp) {
    if (timestamp === 0 || timestamp === null) return "Session";
    const date = new Date((timestamp - 116444736000000000) / 10000); // Chrome epoch
    return date.toLocaleDateString();
  }

  onMount(() => {
    loadBrowsers();
  });
</script>

<div class="mx-auto max-w-6xl">
  <!-- Header -->
  <div class="mb-6">
    <h1 class="text-xl font-semibold text-nx-text">Cookie Extractor</h1>
    <p class="mt-1 text-xs text-nx-text-muted">Extract and export cookies from your web browsers</p>
  </div>

  <!-- Browser Selection -->
  <div class="mb-6 border border-nx-border bg-nx-surface p-5">
    <div class="mb-4 flex items-center justify-between">
      <h2 class="text-sm font-medium text-nx-text">Select Browser</h2>
      {#if selectedBrowser}
        <span class="text-xs text-nx-text-muted">
          {browsers.find(b => b.name === selectedBrowser)?.cookie_count || 0} cookies found
        </span>
      {/if}
    </div>

    <div class="grid grid-cols-3 gap-3">
      {#each browsers as browser}
        <button
          class="border px-4 py-3 text-left {selectedBrowser === browser.name
            ? 'border-nx-accent bg-nx-accent/10'
            : 'border-nx-border bg-nx-bg'}"
          onclick={() => selectedBrowser = browser.name}>
          <div class="flex items-center gap-3">
            <span class="material-symbols-outlined text-2xl {selectedBrowser === browser.name ? 'text-nx-accent' : 'text-nx-text-secondary'}">
              {browser.name === 'Chrome' ? 'chrome' : browser.name === 'Firefox' ? 'firefox' : 'edge'}
            </span>
            <div>
              <div class="text-sm font-medium text-nx-text">{browser.name}</div>
              <div class="text-xs text-nx-text-muted">{browser.cookie_count} cookies</div>
            </div>
          </div>
        </button>
      {/each}
    </div>

    {#if browsers.length === 0}
      <div class="mt-4 border border-nx-border bg-nx-text-secondary/10 p-3 text-xs text-nx-text-secondary">
        No browsers detected. Make sure Chrome, Edge, or Firefox is installed.
      </div>
    {/if}
  </div>

  <!-- Filter and Extract -->
  <div class="mb-6 border border-nx-border bg-nx-surface p-5">
    <div class="flex items-end gap-3">
      <div class="flex-1">
        <label class="mb-1 block text-xs text-nx-text-muted">Domain Filter (optional)</label>
        <input
          type="text"
          bind:value={domainFilter}
          placeholder="e.g., github.com, google.com"
          class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary"
        />
      </div>
      <button
        class="bg-nx-text px-6 py-2 text-sm font-medium text-nx-deep disabled:opacity-50 disabled:cursor-not-allowed"
        onclick={extractCookies}
        disabled={!selectedBrowser || extracting}>
        {#if extracting}
          <span class="flex items-center gap-2">
            <span class="material-symbols-outlined animate-spin text-lg">progress_activity</span>
            Extracting...
          </span>
        {:else}
          <span class="flex items-center gap-2">
            <span class="material-symbols-outlined text-lg">download</span>
            Extract Cookies
          </span>
        {/if}
      </button>
    </div>
  </div>

  <!-- Results -->
  {#if cookies.length > 0}
    <div class="border border-nx-border bg-nx-surface">
      <!-- Toolbar -->
      <div class="flex items-center justify-between border-b border-nx-border px-4 py-3">
        <div class="text-sm text-nx-text">
          <span class="font-medium">{cookies.length}</span> cookies extracted
        </div>
        <div class="flex gap-2">
          <button
            class="border border-nx-border bg-nx-bg px-3 py-1.5 text-xs font-medium text-nx-text-secondary"
            onclick={copyAllCookies}>
            Copy All
          </button>
          <button
            class="border border-nx-border bg-nx-bg px-3 py-1.5 text-xs font-medium text-nx-text-secondary"
            onclick={exportJSON}>
            Export JSON
          </button>
          <button
            class="border border-nx-border bg-nx-bg px-3 py-1.5 text-xs font-medium text-nx-text-secondary"
            onclick={exportNetscape}>
            Export Netscape
          </button>
        </div>
      </div>

      <!-- Cookie List -->
      <div class="max-h-96 overflow-y-auto">
        <table class="w-full">
          <thead class="sticky top-0 bg-nx-surface">
            <tr class="border-b border-nx-border text-xs text-nx-text-muted">
              <th class="px-4 py-2 text-left font-medium">Name</th>
              <th class="px-4 py-2 text-left font-medium">Value</th>
              <th class="px-4 py-2 text-left font-medium">Domain</th>
              <th class="px-4 py-2 text-left font-medium">Expires</th>
              <th class="px-4 py-2 text-right font-medium">Actions</th>
            </tr>
          </thead>
          <tbody>
            {#each cookies as cookie}
              <tr class="group border-b border-nx-border last:border-0">
                <td class="max-w-[200px] px-4 py-2">
                  <div class="truncate text-sm font-medium text-nx-text" title={cookie.name}>
                    {cookie.name}
                  </div>
                </td>
                <td class="max-w-[300px] px-4 py-2">
                  <div class="truncate font-mono text-xs text-nx-text-secondary" title={cookie.value}>
                    {cookie.value}
                  </div>
                </td>
                <td class="px-4 py-2 text-xs text-nx-text-muted">
                  {cookie.domain}
                </td>
                <td class="px-4 py-2 text-xs text-nx-text-muted">
                  {formatDate(cookie.expires)}
                </td>
                <td class="px-4 py-2">
                  <div class="flex items-center justify-end gap-1">
                    <button
                      class="p-1 text-nx-text-secondary"
                      title="Copy Cookie"
                      onclick={() => copyCookie(cookie)}>
                      <span class="material-symbols-outlined text-base">content_copy</span>
                    </button>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  {:else if extracting === false && cookies.length === 0 && selectedBrowser}
    <div class="border border-nx-border bg-nx-surface p-12 text-center">
      <span class="material-symbols-outlined text-nx-text-muted text-4xl">cookie</span>
      <div class="mt-4 text-sm text-nx-text-muted">No cookies extracted</div>
      <div class="mt-1 text-xs text-nx-text-muted">Click "Extract Cookies" to begin</div>
    </div>
  {/if}
</div>
