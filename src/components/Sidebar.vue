<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { useRoute, useRouter } from "vue-router";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../lib/i18n.js";
import { APP_VERSION } from "../lib/version.js";

const route = useRoute();
const router = useRouter();

const appVersion = ref(APP_VERSION);
const resourceUsage = ref(null);
let timer = null;

onMounted(() => {
  (async () => {
    try {
      appVersion.value = await getVersion();
    } catch {
      // non-Tauri env fallback
    }
    loadResourceUsage();
  })();
  timer = setInterval(loadResourceUsage, 10000);
});

onBeforeUnmount(() => {
  if (timer) clearInterval(timer);
});

async function loadResourceUsage() {
  try {
    resourceUsage.value = await invoke("get_resource_usage");
  } catch {
    // silently fail
  }
}

const navItems = [
  { route: "/dashboard", label: () => t("nav.dashboard"), icon: "icon-dashboard" },
  { route: "/environments", label: () => t("nav.environments"), icon: "icon-code" },
  { route: "/migration", label: () => t("nav.migration"), icon: "icon-swap" },
  { route: "/software", label: () => t("nav.software"), icon: "icon-apps" },
  { route: "/containers", label: () => t("nav.containers"), icon: "icon-command" },
  { route: "/mirrors", label: () => t("nav.mirrors"), icon: "icon-sync" },
  { route: "/processes", label: () => t("nav.processes"), icon: "icon-thunderbolt" },
  { route: "/passwords", label: () => t("nav.passwords"), icon: "icon-lock" },
  { route: "/cookies", label: () => t("nav.cookies"), icon: "icon-idcard" },
  { route: "/uninstall", label: () => t("nav.uninstall"), icon: "icon-delete" },
  { route: "/api-hub", label: () => t("nav.api_hub"), icon: "icon-branch" },
  { route: "/settings", label: () => t("nav.settings"), icon: "icon-settings" },
];

const selectedKey = computed(() => {
  const p = route.path;
  if (p === "/ports") return "/processes";
  return p;
});

const cpuPercent = computed(() =>
  resourceUsage.value ? resourceUsage.value.cpu_usage.toFixed(0) : null
);
const memPercent = computed(() =>
  resourceUsage.value ? resourceUsage.value.memory_percent.toFixed(0) : null
);
const cpuBar = computed(() =>
  resourceUsage.value ? Math.min(resourceUsage.value.cpu_usage, 100) : 0
);
const memBar = computed(() =>
  resourceUsage.value ? Math.min(resourceUsage.value.memory_percent, 100) : 0
);

function handleClick(key) {
  router.push(key);
}
</script>

<template>
  <aside class="sidebar" aria-label="Main navigation">
    <!-- Logo area -->
    <div class="logo">
      <icon-terminal class="logo-icon" />
      <span class="logo-text">DevNexus</span>
    </div>

    <!-- Navigation -->
    <a-menu
      class="nav-menu"
      :selected-keys="[selectedKey]"
      @menu-item-click="handleClick"
      :auto-open-selected="true"
    >
      <a-menu-item v-for="item in navItems" :key="item.route">
        <template #icon>
          <component :is="item.icon" />
        </template>
        {{ item.label() }}
      </a-menu-item>
    </a-menu>

    <!-- Status Bar -->
    <div v-if="resourceUsage" class="status-bar">
      <div class="status-row">
        <span class="status-name">CPU</span>
        <a-progress
          :percent="cpuBar"
          :show-text="false"
          :color="{ from: 'rgb(var(--primary-5))', to: 'rgb(var(--primary-6))' }"
          size="small"
          class="status-bar-progress"
        />
        <span class="status-value">{{ cpuPercent }}%</span>
      </div>
      <div class="status-row">
        <span class="status-name">MEM</span>
        <a-progress
          :percent="memBar"
          :show-text="false"
          :color="{ from: 'rgb(var(--green-5))', to: 'rgb(var(--green-6))' }"
          size="small"
          class="status-bar-progress"
        />
        <span class="status-value">{{ memPercent }}%</span>
      </div>
    </div>

    <!-- Version + GitHub -->
    <div class="footer">
      <div class="footer-row">
        <a
          href="https://github.com/linanwanttodo/DevNexus"
          target="_blank"
          rel="noopener noreferrer"
          class="github-link"
          title="GitHub"
        >
          <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
            <path
              d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"
            />
          </svg>
        </a>
        <span class="version">v{{ appVersion }}</span>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  display: flex;
  flex-direction: column;
  width: 240px;
  flex-shrink: 0;
  height: 100%;
  border-right: 1px solid var(--color-border);
  background-color: var(--color-bg-2, #161616);
}

.logo {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 44px;
  padding: 0 16px;
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
}

.logo-icon {
  font-size: 18px;
  color: var(--color-primary-6);
}

.logo-text {
  font-size: 14px;
  font-weight: 600;
  letter-spacing: -0.01em;
  color: var(--color-text-1);
}

.nav-menu {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
  border: none;
  background: transparent;
  overflow-x: hidden;
}

.nav-menu :deep(.arco-menu-inner) {
  padding: 8px;
}

.nav-menu :deep(.arco-menu-item) {
  border-radius: 6px;
  margin-bottom: 2px;
}

.status-bar {
  border-top: 1px solid var(--color-border);
  padding: 10px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex-shrink: 0;
}

.status-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-name {
  width: 28px;
  font-size: 10px;
  color: var(--color-text-3);
  font-family: "JetBrains Mono", monospace;
}

.status-bar-progress {
  flex: 1;
}

.status-value {
  width: 32px;
  text-align: right;
  font-size: 10px;
  color: var(--color-text-3);
  font-family: "JetBrains Mono", monospace;
}

.footer {
  border-top: 1px solid var(--color-border);
  padding: 10px 16px;
  flex-shrink: 0;
}

.footer-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.github-link {
  display: flex;
  color: var(--color-text-3);
  transition: color 0.15s ease;
}

.github-link:hover {
  color: var(--color-text-1);
}

.version {
  font-size: 11px;
  color: var(--color-text-3);
}
</style>
