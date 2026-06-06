<script>
  import BrandIcons from "../icons/BrandIcons.svelte";
  import { getRoute, navigate } from "../lib/stores.svelte.js";
  import { t } from "../lib/i18n.svelte.js";

  let navItems = $derived.by(() => {
    return [
      { route: "/dashboard", label: t("nav.dashboard"), icon: "dashboard" },
      { route: "/environments", label: t("nav.environments"), icon: "code" },
      { route: "/software", label: t("nav.software"), icon: "apps" },
      { route: "/mirrors", label: t("nav.mirrors"), icon: "sync" },
      { route: "/ports", label: t("nav.ports"), icon: "lan" },
      { route: "/scheduler", label: t("nav.scheduler"), icon: "schedule" },
      { route: "/passwords", label: t("nav.passwords"), icon: "key" },
      { route: "/cookies", label: t("nav.cookies"), icon: "cookie" },
      { route: "/uninstall", label: t("nav.uninstall"), icon: "delete" },
      { route: "/settings", label: t("nav.settings"), icon: "settings" },
    ];
  });

  let currentRoute = $derived(getRoute());

  function handleClick(route) {
    navigate(route);
  }
</script>

<aside class="flex h-full w-64 flex-shrink-0 flex-col border-r border-nx-border bg-nx-bg" aria-label="Main navigation">
  <nav class="flex-1 overflow-y-auto px-3 py-4">
    <ul class="space-y-px">
      {#each navItems as item}
        <li>
          <button
            type="button"
            role="tab"
            aria-current={currentRoute === item.route ? "page" : undefined}
            class="flex w-full items-center gap-3 px-3 py-2 text-sm cursor-pointer {currentRoute === item.route
              ? 'bg-nx-accent/15 text-nx-accent font-medium'
              : 'text-nx-text-secondary'}"
            onclick={() => handleClick(item.route)}
          >
            {#if item.route === '/cookies'}
              <BrandIcons name="cookie" size={20} class="text-xl" />
            {:else}
              <span class="material-symbols-outlined text-xl">{item.icon}</span>
            {/if}
            {item.label}
          </button>
        </li>
      {/each}
    </ul>
  </nav>

  <div class="border-t border-nx-border px-5 py-3">
    <span class="text-xs text-nx-text-muted">{t("version")} 1.0.3</span>
  </div>
</aside>