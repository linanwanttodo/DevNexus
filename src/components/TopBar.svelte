<script>
  import { getRoute, onRouteChange, getSearchQuery, setSearchQuery } from "../lib/stores.js";

  const routeTitles = {
    "/dashboard": "Dashboard",
    "/environments": "Environment Manager",
    "/software": "Software Center",
    "/mirrors": "Package Mirrors",
    "/ports": "Port Manager",
    "/scheduler": "Task Scheduler",
    "/passwords": "Password Manager",
    "/cookies": "Cookie Extractor",
    "/settings": "Settings",
  };

  let currentRoute = $state(getRoute());
  let searchQuery = $state(getSearchQuery());

  function handleSearchInput(e) {
    searchQuery = e.target.value;
    setSearchQuery(e.target.value);
  }

  $effect(() => {
    return onRouteChange((r) => {
      currentRoute = r;
    });
  });
</script>

<header class="flex h-10 flex-shrink-0 items-center justify-between border-b border-nx-border bg-nx-surface px-4">
  <div class="flex items-center gap-3">
    <span class="material-symbols-outlined text-nx-accent text-lg">terminal</span>
    <span class="text-xs font-medium text-nx-text-secondary">DevNexus</span>
    <span class="text-nx-border">|</span>
    <h2 class="text-sm font-medium text-nx-text">
      {routeTitles[currentRoute] || "DevNexus"}
    </h2>
  </div>

  <div class="flex items-center gap-3">
    <div class="relative">
      <span class="material-symbols-outlined absolute left-2 top-1/2 -translate-y-1/2 text-nx-text-muted text-sm">search</span>
      <input
        type="text"
        placeholder="Search ports, processes..."
        value={searchQuery}
        oninput={handleSearchInput}
        class="h-7 w-56 border border-nx-border bg-nx-bg pl-7 pr-2 text-xs text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-accent"
      />
    </div>
  </div>
</header>