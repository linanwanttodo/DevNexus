<script>
  import { getRoute, onRouteChange } from "./lib/stores.js";
  import Sidebar from "./components/Sidebar.svelte";
  import TopBar from "./components/TopBar.svelte";
  import Dashboard from "./routes/Dashboard.svelte";
  import EnvironmentManager from "./routes/EnvironmentManager.svelte";
  import SoftwareCenter from "./routes/SoftwareCenter.svelte";
  import MirrorSettings from "./routes/MirrorSettings.svelte";
  import PortManager from "./routes/PortManager.svelte";
  import TaskScheduler from "./routes/TaskScheduler.svelte";
  import PasswordManager from "./routes/PasswordManager.svelte";
  import CookieExtractor from "./routes/CookieExtractor.svelte";
  import Settings from "./routes/Settings.svelte";

  let page = $state(getRoute());

  $effect(() => {
    return onRouteChange((r) => {
      page = r;
    });
  });
</script>

<div class="flex h-screen w-screen flex-col bg-nx-bg overflow-hidden">
  <div class="flex flex-1 overflow-hidden">
    <Sidebar />
    <div class="flex flex-1 flex-col overflow-hidden">
      <TopBar />
      <main class="flex-1 overflow-y-auto p-6">
      {#if page === "/" || page === "/dashboard"}
        <Dashboard />
      {:else if page === "/environments"}
        <EnvironmentManager />
      {:else if page === "/software"}
        <SoftwareCenter />
      {:else if page === "/mirrors"}
        <MirrorSettings />
      {:else if page === "/ports"}
        <PortManager />
      {:else if page === "/scheduler"}
        <TaskScheduler />
      {:else if page === "/passwords"}
        <PasswordManager />
      {:else if page === "/cookies"}
        <CookieExtractor />
      {:else if page === "/settings"}
        <Settings />
      {/if}
      </main>
    </div>
  </div>
</div>