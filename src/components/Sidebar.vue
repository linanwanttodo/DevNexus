<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { useRoute, useRouter } from "vue-router";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import AppIcon from "./AppIcon.vue";
import { t } from "../lib/i18n.js";
import { APP_VERSION } from "../lib/version.js";
import { navItems, navForPath } from "../lib/nav-config.js";

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

const active = computed(() =>
  navForPath(route.path === "/ports" ? "/processes" : route.path)
);

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

function handleNavClick(item) {
  router.push(item.route);
}

function handleSubClick(sub) {
  router.push(sub.route);
}
</script>

<template>
  <aside class="sidebar" aria-label="Main navigation">
    <!-- Logo area -->
    <div class="logo">
      <svg
        class="logo-icon"
        width="18"
        height="18"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <polyline points="4 17 10 11 4 5" />
        <line x1="12" y1="19" x2="20" y2="19" />
      </svg>
      <span class="logo-text">DevNexus</span>
    </div>

    <!-- Navigation -->
    <div class="sidebar-body">
      <!-- 左：图标轨 -->
      <nav class="icon-rail" aria-label="Main navigation">
        <button
          v-for="item in navItems"
          :key="item.id"
          type="button"
          class="rail-item"
          :class="{ active: active.nav && active.nav.id === item.id }"
          :title="t(item.labelKey)"
          @click="handleNavClick(item)"
        >
          <AppIcon :name="item.icon" class="rail-icon" />
        </button>
      </nav>

      <!-- 右：上下文面板 -->
      <nav v-if="active.nav && active.nav.context" class="context-panel">
        <div class="context-title">{{ t(active.nav.context.titleKey) }}</div>
        <button
          v-for="sub in active.nav.context.items"
          :key="sub.route"
          type="button"
          class="context-item"
          :class="{ active: active.sub && active.sub.route === sub.route }"
          @click="handleSubClick(sub)"
        >
          <AppIcon :name="sub.icon" class="context-icon" />
          <span>{{ t(sub.labelKey) }}</span>
        </button>
      </nav>
    </div>

    <!-- Status Bar -->
    <div v-if="resourceUsage" class="status-bar">
      <div class="status-row">
        <span class="status-name">CPU</span>
        <div class="status-track">
          <div
            class="status-fill"
            :style="{ width: cpuBar + '%' }"
            :class="cpuBar > 80 ? 'fill-high' : 'fill-normal'"
          ></div>
        </div>
        <span class="status-value">{{ cpuPercent }}%</span>
      </div>
      <div class="status-row">
        <span class="status-name">MEM</span>
        <div class="status-track">
          <div
            class="status-fill"
            :style="{ width: memBar + '%' }"
            :class="memBar > 80 ? 'fill-high' : 'fill-green'"
          ></div>
        </div>
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
  width: auto;
  min-width: 52px;
  flex-shrink: 0;
  height: 100%;
  border-right: 1px solid var(--color-border);
  background-color: var(--color-sidebar);
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
  color: var(--color-sidebar-primary);
}

.logo-text {
  font-size: 14px;
  font-weight: 600;
  letter-spacing: -0.01em;
  color: var(--color-sidebar-foreground);
}

.sidebar-body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.icon-rail {
  width: 52px;
  flex-shrink: 0;
  padding: 8px 6px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  border-right: 1px solid var(--color-border);
  overflow-y: auto;
}

.rail-item {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 34px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--color-sidebar-foreground);
  opacity: 0.72;
  cursor: pointer;
  transition:
    background-color 0.12s ease,
    opacity 0.12s ease;
}

.rail-item:hover {
  background-color: var(--color-sidebar-accent);
  opacity: 1;
}

.rail-item.active {
  background-color: var(--color-sidebar-accent);
  opacity: 1;
}

.rail-icon {
  width: 18px;
  height: 18px;
}

.context-panel {
  flex: 1;
  min-width: 0;
  padding: 10px 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  overflow-y: auto;
}

.context-title {
  padding: 4px 10px 8px;
  font-size: 12px;
  font-weight: 600;
  color: var(--color-muted-foreground);
  letter-spacing: 0.02em;
}

.context-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 6px 10px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--color-sidebar-foreground);
  opacity: 0.72;
  font-size: 13px;
  text-align: left;
  cursor: pointer;
  transition:
    background-color 0.12s ease,
    opacity 0.12s ease;
}

.context-item:hover {
  background-color: var(--color-sidebar-accent);
  opacity: 1;
}

.context-item.active {
  background-color: var(--color-sidebar-accent);
  opacity: 1;
  font-weight: 500;
}

.context-icon {
  width: 15px;
  height: 15px;
  flex-shrink: 0;
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
  color: var(--color-muted-foreground);
  font-family: "JetBrains Mono", monospace;
}

.status-track {
  flex: 1;
  height: 4px;
  border-radius: 9999px;
  background-color: var(--color-muted);
  overflow: hidden;
}

.status-fill {
  height: 100%;
  border-radius: 9999px;
  transition: width 0.4s ease;
}

.fill-normal {
  background-color: var(--color-primary);
}

.fill-green {
  background-color: var(--color-primary);
  opacity: 0.85;
}

.fill-high {
  background-color: var(--color-destructive);
}

.status-value {
  width: 32px;
  text-align: right;
  font-size: 10px;
  color: var(--color-muted-foreground);
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
  color: var(--color-muted-foreground);
  transition: color 0.15s ease;
}

.github-link:hover {
  color: var(--color-foreground);
}

.version {
  font-size: 11px;
  color: var(--color-muted-foreground);
}
</style>