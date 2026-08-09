<script setup>
import { ref, onMounted } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getWindowTop, setWindowTop } from "../lib/stores.js";
import { t } from "../lib/i18n.js";

// 非 Tauri 环境（纯浏览器调试）时降级为 no-op，避免整个应用崩溃
let appWindow = null;
try {
  appWindow = getCurrentWindow();
} catch {
  appWindow = null;
}

// 窗口置顶状态（与 localStorage 持久化，重启后自动恢复）
const pinned = ref(getWindowTop().value);

onMounted(() => {
  // 重启后 OS 层不会保留置顶状态，需要按持久化偏好重新拉起
  if (pinned.value && appWindow) {
    appWindow.setAlwaysOnTop(true).catch(() => {});
  }
});

async function togglePin() {
  const next = !pinned.value;
  if (appWindow) {
    try {
      await appWindow.setAlwaysOnTop(next);
    } catch (e) {
      console.error("setAlwaysOnTop failed:", e);
      return;
    }
  }
  pinned.value = next;
  setWindowTop(next);
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
  <header data-tauri-drag-region class="titlebar">
    <!-- Left: drag region spacer -->
    <div class="titlebar-spacer" data-tauri-drag-region></div>

    <!-- Right: window controls -->
    <div class="titlebar-controls">
      <!-- 窗口置顶（微信同款，位于三键左侧），再次点击取消 -->
      <button
        class="win-btn"
        :class="{ 'win-btn-pinned': pinned }"
        :aria-label="pinned ? t('titleBar.unpin') : t('titleBar.pin')"
        :title="pinned ? t('titleBar.unpin') : t('titleBar.pin')"
        @click="togglePin"
      >
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <line x1="12" x2="12" y1="17" y2="22" />
          <path
            d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z"
          />
        </svg>
      </button>
      <button
        class="win-btn"
        :aria-label="t('titleBar.minimize')"
        @click="minimize"
      >
        <svg width="11" height="1" viewBox="0 0 11 1" fill="currentColor">
          <rect width="11" height="1" />
        </svg>
      </button>
      <button
        class="win-btn"
        :aria-label="t('titleBar.maximize')"
        @click="toggleMaximize"
      >
        <svg
          width="10"
          height="10"
          viewBox="0 0 10 10"
          fill="none"
          stroke="currentColor"
          stroke-width="1"
        >
          <rect x="0.5" y="0.5" width="9" height="9" />
        </svg>
      </button>
      <button
        class="win-btn win-btn-close"
        :aria-label="t('titleBar.close')"
        @click="close"
      >
        <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor">
          <path
            d="M1.007 0L0 1.007 3.993 5 0 8.993 1.007 10 5 6.007 8.993 10 10 8.993 6.007 5 10 1.007 8.993 0 5 3.993z"
          />
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
  background-color: var(--color-card);
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
  color: var(--color-muted-foreground);
  cursor: pointer;
  transition: background-color 0.12s ease, color 0.12s ease;
}

.win-btn:hover {
  background-color: var(--color-accent);
  color: var(--color-foreground);
}

/* 置顶激活态：常亮高亮，与微信置顶按钮一致 */
.win-btn-pinned,
.win-btn-pinned:hover {
  background-color: var(--color-accent);
  color: var(--color-primary);
}

.win-btn-close:hover {
  background-color: var(--color-destructive);
  color: var(--color-destructive-foreground);
}
</style>