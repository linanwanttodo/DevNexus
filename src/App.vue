<script setup>
import { defineAsyncComponent, onMounted } from "vue";
import TitleBar from "./components/TitleBar.vue";
import Sidebar from "./components/Sidebar.vue";
import ErrorBoundary from "./components/ErrorBoundary.vue";
import { getTheme, setIslandEnabled } from "./lib/stores.js";
import { applyIslandState } from "./lib/island.js";
import { router } from "./router.js";

// 首屏只渲染主布局；ConfirmDialog / Sonner / SudoDialog 为运行时交互组件，按需懒加载
const ConfirmDialog = defineAsyncComponent(() => import("./components/ConfirmDialog.vue"));
const SudoDialog = defineAsyncComponent(() => import("./components/SudoDialog.vue"));
const Sonner = defineAsyncComponent(() => import("./components/ui/sonner/Sonner.vue"));

const theme = getTheme();

// 启动时按持久化状态恢复灵动岛悬浮窗（独立透明置顶窗口）
onMounted(() => {
  applyIslandState();
  // 托盘菜单导航：点击"灵动岛设置/检查更新"时跳转对应页面
  try {
    const { listen } = window.__TAURI_INTERNALS__
      ? require_tauri_listen()
      : { listen: async () => () => {} };
    listen("tray-nav", (ev) => {
      const path = ev?.payload;
      if (typeof path === "string" && path.startsWith("/")) {
        router.push(path);
      }
    });
    // 托盘开关灵动岛：Rust 侧直接隐藏/显示窗口后广播新状态，
    // 这里只回写本地状态（syncOnly=true），不反向调用 Rust 命令，
    // 否则会再次触发 island-state 事件 → 前端/后端无限循环开关节目岛。
    listen("island-state", (ev) => {
      if (typeof ev?.payload === "boolean") {
        setIslandEnabled(ev.payload, true);
      }
    });
  } catch {
    // 非 Tauri 环境忽略
  }
});

function require_tauri_listen() {
  // 动态导入以保持 SSR/浏览器降级友好
  return { listen: (evt, cb) => import("@tauri-apps/api/event").then((m) => m.listen(evt, cb)) };
}
</script>

<template>
  <div class="app-shell">
    <!-- Custom title bar -->
    <TitleBar />

    <div class="app-body">
      <Sidebar />
      <main class="app-main">
        <router-view v-slot="{ Component }">
          <ErrorBoundary>
            <component :is="Component" />
          </ErrorBoundary>
        </router-view>
      </main>
    </div>

    <ConfirmDialog />
    <SudoDialog />
    <Sonner :theme="theme" position="top-center" rich-colors expand :toast-options="{ duration: 3500 }" />
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background-color: var(--color-background);
}

.app-body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.app-main {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  background-color: transparent;
}

.app-main :deep(.page) {
  padding: var(--nx-page-py, 12px) 24px;
  max-width: var(--nx-page-max-width);
  margin: 0 auto;
}
</style>
