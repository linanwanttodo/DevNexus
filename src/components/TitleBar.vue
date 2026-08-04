<script setup>
import { getCurrentWindow } from "@tauri-apps/api/window";
import { t } from "../lib/i18n.js";

// 非 Tauri 环境（纯浏览器调试）时降级为 no-op，避免整个应用崩溃
let appWindow = null;
try {
  appWindow = getCurrentWindow();
} catch {
  appWindow = null;
}

function minimize() {
  if (appWindow) appWindow.minimize();
}
function toggleMaximize() {
  if (appWindow) appWindow.toggleMaximize();
}
function close() {
  if (appWindow) appWindow.close();
}
</script>

<template>
  <header
    data-tauri-drag-region
    class="titlebar"
  >
    <!-- Left: drag region spacer -->
    <div class="titlebar-spacer" data-tauri-drag-region></div>

    <!-- Right: window controls -->
    <div class="titlebar-controls">
      <button class="win-btn" :aria-label="t('titleBar.minimize')" @click="minimize">
        <svg width="11" height="1" viewBox="0 0 11 1" fill="currentColor">
          <rect width="11" height="1" />
        </svg>
      </button>
      <button class="win-btn" :aria-label="t('titleBar.maximize')" @click="toggleMaximize">
        <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1">
          <rect x="0.5" y="0.5" width="9" height="9" />
        </svg>
      </button>
      <button class="win-btn win-btn-close" :aria-label="t('titleBar.close')" @click="close">
        <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor">
          <path d="M1.007 0L0 1.007 3.993 5 0 8.993 1.007 10 5 6.007 8.993 10 10 8.993 6.007 5 10 1.007 8.993 0 5 3.993z" />
        </svg>
      </button>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 36px;
  flex-shrink: 0;
  border-bottom: 1px solid var(--color-border);
  background-color: var(--color-bg-2, #161616);
  user-select: none;
}

.titlebar-spacer {
  flex: 1;
}

.titlebar-controls {
  display: flex;
  height: 100%;
}

.win-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 100%;
  border: none;
  background: none;
  color: var(--color-text-3);
  cursor: pointer;
  transition: background-color 0.12s ease, color 0.12s ease;
}

.win-btn:hover {
  background-color: var(--color-fill-2);
  color: var(--color-text-1);
}

.win-btn-close:hover {
  background-color: rgb(var(--red-6));
  color: var(--color-white);
}
</style>
