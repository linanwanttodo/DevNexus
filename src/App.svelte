<script>
  import { getRoute } from "./lib/stores.svelte.js";
  import TitleBar from "./components/TitleBar.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import Toast from "./components/Toast.svelte";
  import ConfirmDialog from "./components/ConfirmDialog.svelte";
  import ErrorBoundary from "./components/ErrorBoundary.svelte";
  import Dashboard from "./routes/Dashboard.svelte";
  import EnvironmentManager from "./routes/EnvironmentManager.svelte";
  import SoftwareCenter from "./routes/SoftwareCenter.svelte";
  import MirrorSettings from "./routes/MirrorSettings.svelte";
  import ProcessManager from "./routes/ProcessManager.svelte";
  import PasswordManager from "./routes/PasswordManager.svelte";
  import CookieExtractor from "./routes/CookieExtractor.svelte";
  import AppUninstaller from "./routes/AppUninstaller.svelte";
  import Settings from "./routes/Settings.svelte";
  import ContainerManager from "./routes/ContainerManager.svelte";
  import ApiHub from "./routes/ApiHub.svelte";
  import Migration from "./routes/Migration.svelte";
  import DownloadManager from "./routes/DownloadManager.svelte";

  let page = $derived(getRoute());
  let prevPage = $state(getRoute());
  let transitioning = $state(false);

  // Trigger a subtle page transition on route change
  $effect(() => {
    if (page !== prevPage) {
      transitioning = true;
      // Use setTimeout to allow CSS animation to trigger on re-render
      const id = setTimeout(() => {
        transitioning = false;
        prevPage = page;
      }, 20);
      return () => clearTimeout(id);
    }
  });
</script>

<div class="flex h-screen w-screen flex-col bg-nx-bg overflow-hidden">
  <!-- Custom title bar -->
  <TitleBar />

  <div class="flex flex-1 overflow-hidden">
    <Sidebar />
    <div class="flex flex-1 flex-col overflow-hidden min-w-0">
      <main class="flex-1 overflow-y-auto" class:nx-page={!transitioning}>
        {#if page === "/" || page === "/dashboard"}
          <Dashboard />
        {:else if page === "/environments"}
          <EnvironmentManager />
        {:else if page === "/software"}
          <SoftwareCenter />
        {:else if page === "/mirrors"}
          <MirrorSettings />
        {:else if page === "/processes" || page === "/ports"}
          <ProcessManager />
        {:else if page === "/passwords"}
          <PasswordManager />
        {:else if page === "/cookies"}
          <CookieExtractor />
        {:else if page === "/uninstall"}
          <AppUninstaller />
        {:else if page === "/containers"}
          <ContainerManager />
        {:else if page === "/settings"}
          <Settings />
        {:else if page === "/api-hub"}
          <ApiHub />
        {:else if page === "/migration"}
          <Migration />
        {:else if page === "/downloads"}
          <DownloadManager />
        {/if}
      </main>
    </div>
  </div>
  <Toast />
  <ConfirmDialog />
  <ErrorBoundary />
</div>
