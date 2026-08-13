<script setup>
import { ref, onMounted } from "vue";
import { Pin, AppWindow } from "@lucide/vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  getWindowTop,
  setWindowTop,
  getIslandEnabled,
  setIslandEnabled,
} from "../lib/stores.js";
import { applyIslandState } from "../lib/island.js";
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

// 灵动岛开关：反映本地持久化状态，点击直接开/关悬浮窗（推送到 Rust）
const islandOn = ref(getIslandEnabled().value);

onMounted(() => {
  // 重启后 OS 层不会保留置顶状态，需要按持久化偏好重新拉起
  if (pinned.value && appWindow) {
    appWindow.setAlwaysOnTop(true).catch(() => {});
  }
  // 托盘菜单切换灵动岛时，Rust 会广播 island-state 事件，此处只同步本地状态
  // （syncOnly=true），保证标题栏按钮与真实开关一致；不反向调用 Rust，避免循环。
  try {
    import("@tauri-apps/api/event").then(({ listen }) =>
      listen("island-state", (ev) => {
        if (typeof ev?.payload === "boolean") {
          setIslandEnabled(ev.payload, true);
          islandOn.value = ev.payload;
        }
      })
    );
  } catch {
    // 非 Tauri 环境忽略
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

async function toggleIsland() {
  const next = !islandOn.value;
  setIslandEnabled(next);
  islandOn.value = next;
  try {
    await applyIslandState();
  } catch {
    // 窗口操作失败静默
  }
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
  <!-- 整条标题栏可拖拽窗口：data-tauri-drag-region 放在 header 上。
       按钮区域通过 @pointerdown.stop 阻止事件冒泡到 Tauri 的拖拽监听，
       保证按钮可点、其余区域（含标题文字区）按住即可拖动窗口。 -->
  <header data-tauri-drag-region class="titlebar">
    <!-- Left: drag region spacer -->
    <div class="titlebar-spacer"></div>

    <!-- Right: window controls -->
    <div class="titlebar-controls">
      <!-- 窗口置顶按钮：置于窗口三键左侧，激活时高亮，再次点击取消。
           必须 @pointerdown.stop 阻止冒泡，否则会被 header 的拖拽监听吞掉点击。 -->
      <button
        class="win-btn"
        :class="{ 'win-btn-island-on': islandOn }"
        :aria-label="islandOn ? t('titleBar.island_off') : t('titleBar.island_on')"
        :title="islandOn ? t('titleBar.island_off') : t('titleBar.island_on')"
        @pointerdown.stop
        @click="toggleIsland"
      >
        <AppWindow :size="14" />
      </button>
      <button
        class="win-btn"
        :class="{ 'win-btn-pinned': pinned }"
        :aria-label="pinned ? t('titleBar.unpin') : t('titleBar.pin')"
        :title="pinned ? t('titleBar.unpin') : t('titleBar.pin')"
        @pointerdown.stop
        @click="togglePin"
      >
        <Pin :size="14" />
      </button>
      <button
        class="win-btn"
        :aria-label="t('titleBar.minimize')"
        @pointerdown.stop
        @click="minimize"
      >
        <svg width="11" height="1" viewBox="0 0 11 1" fill="currentColor">
          <rect width="11" height="1" />
        </svg>
      </button>
      <button
        class="win-btn"
        :aria-label="t('titleBar.maximize')"
        @pointerdown.stop
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
        @pointerdown.stop
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

/* 置顶激活态：常亮高亮 */
.win-btn-pinned,
.win-btn-pinned:hover {
  background-color: var(--color-accent);
  color: var(--color-primary);
}

/* 灵动岛开启态：与置顶激活态一致的高亮 */
.win-btn-island-on,
.win-btn-island-on:hover {
  background-color: var(--color-accent);
  color: var(--color-primary);
}

.win-btn-close:hover {
  background-color: var(--color-destructive);
  color: var(--color-destructive-foreground);
}
</style>