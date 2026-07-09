<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.svelte.js";
  import { showConfirm } from "../lib/confirm.svelte.js";
  import { navigate } from "../lib/stores.svelte.js";
  import { t } from "../lib/i18n.svelte.js";

  let activeTab = $state("dns");

  const tabs = [
    { id: "dns", label: "DNS 优化", icon: "dns" },
    { id: "github", label: "GitHub 加速", icon: "code" },
    { id: "cdn", label: "CDN 加速", icon: "cloud" },
    { id: "proxy", label: "代理设置", icon: "settings_ethernet" },
  ];

  // ── DNS ──
  let adapters = $state([]);
  let selectedAdapter = $state("");
  let currentDns = $state({ adapter: "", current_primary: "", current_secondary: "" });
  let dnsServers = $state([]);
  let testingDns = $state("");
  let selectedDns = $state(null);
  let dnsLoading = $state(false);

  // ── GitHub ──
  let githubEntries = $state([]);
  let githubIp = $state("20.205.243.166");
  let githubLoading = $state(false);

  // ── Proxy ──
  let proxyConfig = $state({ http_proxy: "", https_proxy: "", all_proxy: "", no_proxy: "", enabled: false });
  let proxyLoading = $state(false);
  let proxyForm = $state({ http: "", https: "", socks5: "", no: "" });

  // ── CDN ──
  let cdnTestUrls = $state([
    { name: "GitHub", url: "https://github.com", latency: null, testing: false },
    { name: "npm", url: "https://registry.npmjs.org", latency: null, testing: false },
    { name: "PyPI", url: "https://pypi.org", latency: null, testing: false },
    { name: "Docker Hub", url: "https://registry-1.docker.io", latency: null, testing: false },
    { name: "Stack Overflow", url: "https://stackoverflow.com", latency: null, testing: false },
    { name: "Google", url: "https://www.google.com", latency: null, testing: false },
  ]);

  onMount(async () => {
    await loadDnsData();
    await loadProxyConfig();
    await loadGithubHosts();
  });

  // ── DNS Functions ──

  async function loadDnsData() {
    dnsLoading = true;
    try {
      adapters = await invoke("get_network_adapters");
      dnsServers = await invoke("get_dns_servers");
      if (adapters.length > 0) {
        selectedAdapter = adapters[0];
        await loadCurrentDns();
      }
    } catch (err) {
      showToast(`加载 DNS 数据失败: ${err}`, "error");
    } finally {
      dnsLoading = false;
    }
  }

  async function loadCurrentDns() {
    try {
      currentDns = await invoke("get_current_dns", { adapter: selectedAdapter });
    } catch (err) {
      console.error("Failed to load current DNS:", err);
    }
  }

  async function testAllDns() {
    for (let server of dnsServers) {
      testingDns = server.name;
      try {
        const latency = await invoke("test_dns_latency", { dnsServer: server.primary });
        server.latency = latency;
        dnsServers = [...dnsServers]; // trigger reactivity
      } catch {
        server.latency = -1;
        dnsServers = [...dnsServers];
      }
    }
    testingDns = "";
    // Sort by latency
    dnsServers.sort((a, b) => {
      if (a.latency === null) return 1;
      if (b.latency === null) return -1;
      if (a.latency < 0) return 1;
      if (b.latency < 0) return -1;
      return a.latency - b.latency;
    });
    dnsServers = [...dnsServers];
    showToast("DNS 测速完成", "success");
  }

  async function applyDns(server) {
    if (!(await showConfirm(`确定要将 DNS 设置为 ${server.name}（${server.primary} / ${server.secondary}）？`))) return;
    try {
      const result = await invoke("set_dns", {
        adapter: selectedAdapter,
        primary: server.primary,
        secondary: server.secondary,
      });
      showToast(result, "success");
      await loadCurrentDns();
    } catch (err) {
      showToast(String(err), "error");
    }
  }

  // ── GitHub Functions ──

  async function loadGithubHosts() {
    try {
      githubEntries = await invoke("get_github_hosts");
    } catch (err) {
      console.error("Failed to load GitHub hosts:", err);
    }
  }

  async function applyGithubHosts() {
    if (!githubIp.trim()) {
      showToast("请输入 GitHub 加速 IP", "error");
      return;
    }
    githubLoading = true;
    try {
      const entries = githubEntries.map(e => ({
        ...e,
        ip: githubIp.trim(),
        enabled: true,
      }));
      const result = await invoke("set_github_hosts", { entries });
      showToast(result, "success");
      await loadGithubHosts();
    } catch (err) {
      showToast(`设置失败: ${err}`, "error");
    } finally {
      githubLoading = false;
    }
  }

  async function clearGithubHosts() {
    if (!(await showConfirm("确定要清除所有 GitHub 加速 hosts？"))) return;
    try {
      const entries = githubEntries.map(e => ({ ...e, ip: "", enabled: false }));
      const result = await invoke("set_github_hosts", { entries });
      showToast(result, "success");
      await loadGithubHosts();
    } catch (err) {
      showToast(`清除失败: ${err}`, "error");
    }
  }

  // ── CDN Functions ──

  async function testCdnUrl(item) {
    item.testing = true;
    item.latency = null;
    cdnTestUrls = [...cdnTestUrls];
    try {
      const result = await invoke("test_url_latency", { url: item.url });
      item.latency = result.latency_ms;
    } catch {
      item.latency = -1;
    }
    item.testing = false;
    cdnTestUrls = [...cdnTestUrls];
  }

  async function testAllCdn() {
    for (let item of cdnTestUrls) {
      await testCdnUrl(item);
    }
    showToast("CDN 测速完成", "success");
  }

  // ── Proxy Functions ──

  async function loadProxyConfig() {
    try {
      proxyConfig = await invoke("get_system_proxy");
      proxyForm = {
        http: proxyConfig.http_proxy || "",
        https: proxyConfig.https_proxy || "",
        socks5: proxyConfig.all_proxy || "",
        no: proxyConfig.no_proxy || "",
      };
    } catch (err) {
      console.error("Failed to load proxy config:", err);
    }
  }

  async function applyProxy() {
    proxyLoading = true;
    try {
      const result = await invoke("set_system_proxy", {
        httpProxy: proxyForm.http,
        httpsProxy: proxyForm.https,
        allProxy: proxyForm.socks5,
        noProxy: proxyForm.no,
      });
      showToast(result, "success");
      await loadProxyConfig();
    } catch (err) {
      showToast(`设置代理失败: ${err}`, "error");
    } finally {
      proxyLoading = false;
    }
  }

  async function removeProxy() {
    if (!(await showConfirm("确定要清除所有代理设置？"))) return;
    proxyLoading = true;
    try {
      const result = await invoke("remove_system_proxy");
      showToast(result, "success");
      proxyForm = { http: "", https: "", socks5: "", no: "" };
      await loadProxyConfig();
    } catch (err) {
      showToast(`清除代理失败: ${err}`, "error");
    } finally {
      proxyLoading = false;
    }
  }

  function latencyColor(ms) {
    if (ms === null) return "";
    if (ms < 0) return "text-nx-danger";
    if (ms < 50) return "text-nx-success";
    if (ms < 100) return "text-nx-warning";
    return "text-nx-danger";
  }
</script>

<div class="flex h-full flex-col">
  <!-- Header -->
  <div class="flex items-center gap-2 border-b border-nx-border px-5 py-2.5">
    <button class="nx-back-btn" onclick={() => navigate("/dashboard")}>
      <span class="material-symbols-outlined text-lg">arrow_back</span>
      {t("nav.dashboard")}
    </button>
    <span class="text-xs text-nx-text-muted">/</span>
    <h1 class="text-sm font-medium text-nx-text">网络加速</h1>
  </div>

  <!-- Tab pills -->
  <div class="flex items-center gap-1 border-b border-nx-border px-5 py-2.5">
    {#each tabs as tab}
      <button
        class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-md transition-colors
          {activeTab === tab.id
            ? 'bg-nx-accent-bg text-nx-accent'
            : 'text-nx-text-secondary hover:text-nx-text hover:bg-nx-hover'}"
        onclick={() => activeTab = tab.id}>
        <span class="material-symbols-outlined text-sm">{tab.icon}</span>
        {tab.label}
      </button>
    {/each}
  </div>

  <!-- Content -->
  <div class="flex-1 overflow-y-auto p-6">

    <!-- ════ DNS 优化 ════ -->
    {#if activeTab === "dns"}
      <div class="max-w-4xl">
        <div class="mb-6">
          <h2 class="text-lg font-semibold text-nx-text">DNS 优化</h2>
          <p class="mt-1 text-xs text-nx-text-muted">选择最快的 DNS 服务器，加速域名解析</p>
        </div>

        <!-- Adapter selector + Current DNS -->
        <div class="nx-section mb-5">
          <div class="nx-section-header">
            <span class="text-sm font-medium text-nx-text">当前设置</span>
          </div>
          <div class="nx-section-body">
            <div class="flex items-center gap-4">
              <div>
                <label class="mb-1 block text-xs text-nx-text-muted">网络适配器</label>
                <select class="nx-input h-9 text-sm min-w-[150px]"
                  value={selectedAdapter}
                  onchange={(e) => { selectedAdapter = e.currentTarget.value; loadCurrentDns(); }}>
                  {#each adapters as a}
                    <option value={a}>{a}</option>
                  {/each}
                </select>
              </div>
              <div class="flex-1">
                <label class="mb-1 block text-xs text-nx-text-muted">当前 DNS</label>
                <div class="flex items-center gap-3">
                  <span class="text-sm text-nx-text font-mono">{currentDns.current_primary || "未设置"}</span>
                  {#if currentDns.current_secondary}
                    <span class="text-xs text-nx-text-muted">/</span>
                    <span class="text-sm text-nx-text-secondary font-mono">{currentDns.current_secondary}</span>
                  {/if}
                </div>
              </div>
              <button class="nx-btn nx-btn-primary h-9" onclick={testAllDns} disabled={testingDns !== ""}>
                {#if testingDns}
                  <span class="material-symbols-outlined text-sm nx-animate-spin">progress_activity</span>
                  测速中...
                {:else}
                  <span class="material-symbols-outlined text-sm">speed</span>
                  全部测速
                {/if}
              </button>
            </div>
          </div>
        </div>

        <!-- DNS List -->
        <div class="nx-section">
          <div class="nx-section-header">
            <span class="text-sm font-medium text-nx-text">DNS 服务器列表</span>
            {#if testingDns}
              <span class="text-xs text-nx-accent">正在测试: {testingDns}</span>
            {/if}
          </div>
          <table class="nx-table" style="table-layout: fixed;">
            <colgroup>
              <col style="width: 5%;" />
              <col style="width: 25%;" />
              <col style="width: 22%;" />
              <col style="width: 22%;" />
              <col style="width: 12%;" />
              <col style="width: 14%;" />
            </colgroup>
            <thead>
              <tr class="text-xs text-nx-text-muted">
                <th>序号</th>
                <th class="text-left">DNS 服务商</th>
                <th class="text-left">首选</th>
                <th class="text-left">备选</th>
                <th class="text-right">延迟</th>
                <th class="text-right">操作</th>
              </tr>
            </thead>
            <tbody>
              {#each dnsServers as server}
                <tr>
                  <td class="text-xs text-nx-text-muted">{server.id}</td>
                  <td class="text-sm font-medium text-nx-text">{server.name}</td>
                  <td class="font-mono text-xs text-nx-text-secondary">{server.primary}</td>
                  <td class="font-mono text-xs text-nx-text-secondary">{server.secondary}</td>
                  <td class="text-right">
                    {#if server.latency !== null}
                      {#if server.latency < 0}
                        <span class="text-xs text-nx-danger">超时</span>
                      {:else}
                        <span class="text-xs {latencyColor(server.latency)}">{server.latency} ms</span>
                      {/if}
                    {:else}
                      <span class="text-xs text-nx-text-muted">—</span>
                    {/if}
                  </td>
                  <td class="text-right">
                    <button
                      class="nx-btn text-xs text-nx-accent hover:bg-nx-accent/10 px-2 py-1"
                      onclick={() => applyDns(server)}>
                      应用
                    </button>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>

    <!-- ════ GitHub 加速 ════ -->
    {:else if activeTab === "github"}
      <div class="max-w-4xl">
        <div class="mb-6">
          <h2 class="text-lg font-semibold text-nx-text">GitHub 加速</h2>
          <p class="mt-1 text-xs text-nx-text-muted">通过修改 hosts 文件加速 GitHub 访问</p>
        </div>

        <!-- GitHub IP input -->
        <div class="nx-section mb-5">
          <div class="nx-section-header">
            <span class="text-sm font-medium text-nx-text">加速 IP</span>
          </div>
          <div class="nx-section-body">
            <p class="mb-3 text-xs text-nx-text-muted">
              输入可用的 GitHub 加速 IP（可从
              <a href="https://github.com/Leon406/SubCrawler" target="_blank" class="text-nx-accent underline">SubCrawler</a>
              或
              <a href="https://ipaddress.app" target="_blank" class="text-nx-accent underline">ipaddress.app</a>
              获取）
            </p>
            <div class="flex items-center gap-3">
              <input class="nx-input flex-1 h-9 font-mono text-sm"
                type="text"
                placeholder="20.205.243.166"
                bind:value={githubIp} />
              <button class="nx-btn nx-btn-primary h-9" onclick={applyGithubHosts} disabled={githubLoading}>
                <span class="material-symbols-outlined text-sm">check</span>
                应用
              </button>
              <button class="nx-btn h-9" onclick={clearGithubHosts}>
                <span class="material-symbols-outlined text-sm">delete</span>
                清除
              </button>
            </div>
          </div>
        </div>

        <!-- Current hosts -->
        <div class="nx-section">
          <div class="nx-section-header">
            <span class="text-sm font-medium text-nx-text">当前 hosts 配置</span>
          </div>
          <table class="nx-table" style="table-layout: fixed;">
            <colgroup>
              <col style="width: 40%;" />
              <col style="width: 30%;" />
              <col style="width: 30%;" />
            </colgroup>
            <thead>
              <tr class="text-xs text-nx-text-muted">
                <th class="text-left">域名</th>
                <th class="text-left">IP 地址</th>
                <th class="text-left">状态</th>
              </tr>
            </thead>
            <tbody>
              {#each githubEntries as entry}
                <tr>
                  <td class="font-mono text-sm text-nx-text">{entry.domain}</td>
                  <td class="font-mono text-xs text-nx-text-secondary">{entry.ip || "未配置"}</td>
                  <td>
                    {#if entry.enabled && entry.ip}
                      <span class="nx-pill text-[10px] text-nx-success bg-nx-success-bg">已启用</span>
                    {:else}
                      <span class="nx-pill text-[10px]">未启用</span>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>

    <!-- ════ CDN 加速 ════ -->
    {:else if activeTab === "cdn"}
      <div class="max-w-4xl">
        <div class="mb-6 flex items-center justify-between">
          <div>
            <h2 class="text-lg font-semibold text-nx-text">CDN 加速</h2>
            <p class="mt-1 text-xs text-nx-text-muted">测试常用开发服务的访问速度</p>
          </div>
          <button class="nx-btn h-8" onclick={testAllCdn}>
            <span class="material-symbols-outlined text-sm">speed</span>
            全部测速
          </button>
        </div>

        <div class="grid grid-cols-2 gap-4">
          {#each cdnTestUrls as item}
            <div class="nx-card p-4">
              <div class="flex items-center justify-between mb-3">
                <span class="text-sm font-medium text-nx-text">{item.name}</span>
                {#if item.testing}
                  <span class="material-symbols-outlined text-sm nx-animate-spin text-nx-accent">progress_activity</span>
                {:else if item.latency !== null}
                  <span class="text-xs {latencyColor(item.latency)}">
                    {item.latency < 0 ? "超时" : `${item.latency} ms`}
                  </span>
                {/if}
              </div>
              <p class="font-mono text-xs text-nx-text-muted mb-3 truncate">{item.url}</p>
              <div class="flex items-center justify-between">
                <div class="h-1.5 flex-1 bg-nx-border rounded-full overflow-hidden mr-3">
                  {#if item.latency !== null && item.latency > 0}
                    <div class="h-full rounded-full transition-all {item.latency < 50 ? 'bg-nx-success' : item.latency < 100 ? 'bg-nx-warning' : 'bg-nx-danger'}"
                      style="width: {Math.min(100, 100 - item.latency / 5)}%"></div>
                  {/if}
                </div>
                <button class="nx-btn text-xs h-7 px-2" onclick={() => testCdnUrl(item)} disabled={item.testing}>
                  测速
                </button>
              </div>
            </div>
          {/each}
        </div>
      </div>

    <!-- ════ 代理设置 ════ -->
    {:else if activeTab === "proxy"}
      <div class="max-w-4xl">
        <div class="mb-6">
          <h2 class="text-lg font-semibold text-nx-text">代理设置</h2>
          <p class="mt-1 text-xs text-nx-text-muted">配置系统 HTTP/HTTPS/SOCKS5 代理</p>
        </div>

        <!-- Current status -->
        <div class="nx-section mb-5">
          <div class="nx-section-header">
            <span class="text-sm font-medium text-nx-text">当前状态</span>
            <span class="nx-pill text-[10px] {proxyConfig.enabled ? 'text-nx-success bg-nx-success-bg' : ''}">
              {proxyConfig.enabled ? "已启用" : "未启用"}
            </span>
          </div>
          <div class="nx-section-body">
            {#if proxyConfig.enabled}
              <div class="space-y-2 text-xs">
                {#if proxyConfig.http_proxy}
                  <div class="flex items-center gap-2">
                    <span class="text-nx-text-muted w-20">HTTP:</span>
                    <span class="font-mono text-nx-text">{proxyConfig.http_proxy}</span>
                  </div>
                {/if}
                {#if proxyConfig.https_proxy}
                  <div class="flex items-center gap-2">
                    <span class="text-nx-text-muted w-20">HTTPS:</span>
                    <span class="font-mono text-nx-text">{proxyConfig.https_proxy}</span>
                  </div>
                {/if}
                {#if proxyConfig.all_proxy}
                  <div class="flex items-center gap-2">
                    <span class="text-nx-text-muted w-20">SOCKS5:</span>
                    <span class="font-mono text-nx-text">{proxyConfig.all_proxy}</span>
                  </div>
                {/if}
              </div>
            {:else}
              <p class="text-xs text-nx-text-muted">未检测到代理设置</p>
            {/if}
          </div>
        </div>

        <!-- Proxy form -->
        <div class="nx-section">
          <div class="nx-section-header">
            <span class="text-sm font-medium text-nx-text">配置代理</span>
          </div>
          <div class="nx-section-body space-y-4">
            <div>
              <label class="mb-1.5 block text-xs text-nx-text-muted">HTTP 代理</label>
              <input class="nx-input w-full h-9 font-mono text-sm"
                type="text"
                placeholder="http://127.0.0.1:7890"
                bind:value={proxyForm.http} />
            </div>
            <div>
              <label class="mb-1.5 block text-xs text-nx-text-muted">HTTPS 代理</label>
              <input class="nx-input w-full h-9 font-mono text-sm"
                type="text"
                placeholder="http://127.0.0.1:7890"
                bind:value={proxyForm.https} />
            </div>
            <div>
              <label class="mb-1.5 block text-xs text-nx-text-muted">SOCKS5 代理</label>
              <input class="nx-input w-full h-9 font-mono text-sm"
                type="text"
                placeholder="socks5://127.0.0.1:7891"
                bind:value={proxyForm.socks5} />
            </div>
            <div>
              <label class="mb-1.5 block text-xs text-nx-text-muted">不使用代理的地址</label>
              <input class="nx-input w-full h-9 font-mono text-sm"
                type="text"
                placeholder="localhost,127.0.0.1,::1"
                bind:value={proxyForm.no} />
            </div>
            <div class="flex items-center gap-3 pt-2">
              <button class="nx-btn nx-btn-primary h-9" onclick={applyProxy} disabled={proxyLoading}>
                <span class="material-symbols-outlined text-sm">check</span>
                应用代理
              </button>
              <button class="nx-btn h-9" onclick={removeProxy} disabled={proxyLoading}>
                <span class="material-symbols-outlined text-sm">delete</span>
                清除代理
              </button>
            </div>
            <p class="text-[11px] text-nx-text-muted">
              ⚠ 代理设置将写入 ~/.bashrc，需要重启终端或执行 <code class="bg-nx-bg px-1 py-0.5 rounded">source ~/.bashrc</code> 生效
            </p>
          </div>
        </div>
      </div>
    {/if}
  </div>
</div>
