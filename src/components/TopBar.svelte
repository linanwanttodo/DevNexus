<script>
  import { getRoute, getSearchQuery, setSearchQuery } from "../lib/stores.svelte.js";
  import { t } from "../lib/i18n.svelte.js";

  let routeTitles = $derived.by(() => {
    return {
      "/dashboard": t("nav.dashboard"),
      "/environments": t("nav.environments"),
      "/software": t("nav.software"),
      "/mirrors": t("nav.mirrors"),
      "/ports": t("nav.ports"),
      "/scheduler": t("nav.scheduler"),
      "/passwords": t("nav.passwords"),
      "/cookies": t("nav.cookies"),
      "/settings": t("nav.settings"),
    };
  });

  let currentRoute = $derived(getRoute());
  let searchQuery = $derived(getSearchQuery());

  function handleSearchInput(e) {
    setSearchQuery(e.target.value);
  }
</script>

<header class="flex h-10 flex-shrink-0 items-center justify-between border-b border-nx-border bg-nx-surface px-4">
  <div class="flex items-center gap-3">
    <span class="material-symbols-outlined text-nx-accent text-lg">terminal</span>
    <h2 class="text-sm font-medium text-nx-text">
      {routeTitles[currentRoute] || "DevNexus"}
    </h2>
  </div>

  <div class="flex items-center gap-3">
    <div class="relative">
      <span class="material-symbols-outlined absolute left-2 top-1/2 -translate-y-1/2 text-nx-text-muted text-sm">search</span>
      <input
        type="text"
        placeholder={t("search_ports")}
        value={searchQuery}
        oninput={handleSearchInput}
        class="h-7 w-56 border border-nx-border bg-nx-bg pl-7 pr-2 text-xs text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-accent"
      />
    </div>
  </div>
</header>