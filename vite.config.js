import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  build: {
    chunkSizeWarningLimit: 1000,
    rollupOptions: {
      output: {
        manualChunks: {
          "vue-vendor": ["vue", "vue-router"],
          "arco-vendor": ["@arco-design/web-vue"],
          "tauri-vendor": ["@tauri-apps/api/core", "@tauri-apps/plugin-dialog", "@tauri-apps/plugin-shell", "@tauri-apps/plugin-process"],
        },
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    // 当前系统 inotify 文件监视数上限过低（ENOSPC），无法用 sudo 提升时，
    // 改用轮询（usePolling）替代 inotify 监听，彻底避免 "file watchers reached" 错误。
    // 根因修复：sudo sysctl -w fs.inotify.max_user_watches=524288（并写入 /etc/sysctl.d）。
    watch: {
      usePolling: true,
      interval: 1000,
      ignored: ["**/src-tauri/**", "**/target/**", "**/node_modules/**"],
    },
  },
});
