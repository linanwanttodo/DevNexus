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
      "/processes": t("nav.processes"),
      "/uninstall": t("nav.uninstall"),
    };
  });

  let currentRoute = $derived(getRoute());
  let searchQuery = $derived(getSearchQuery());

  function handleSearchInput(e) {
    setSearchQuery(e.target.value);
  }
</script>

<header class="flex h-11 flex-shrink-0 items-center justify-end border-b border-nx-border bg-nx-deep px-4">
  <div class="flex items-center gap-3">
    <div class="relative">
      <span class="material-symbols-outlined absolute left-2 top-1/2 -translate-y-1/2 text-nx-text-muted text-sm pointer-events-none">search</span>
      <input
        type="text"
        placeholder={t("search_ports")}
        value={searchQuery}
        oninput={handleSearchInput}
        class="nx-input h-7 w-56 pl-7 pr-2 text-xs"
      />
    </div>
  </div>
</header>
