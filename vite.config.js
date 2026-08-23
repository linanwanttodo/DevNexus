import { fileURLToPath, URL } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  clearScreen: false,
  build: {
    chunkSizeWarningLimit: 1000,
    // 启用轻量压缩与现代目标，降低运行时内存与首次加载开销
    target: "esnext",
    cssMinify: true,
    reportCompressedSize: false,
    rollupOptions: {
      // 多入口：主应用 + 灵动岛悬浮窗（透明窗口加载 island.html）
      input: {
        main: fileURLToPath(new URL("./index.html", import.meta.url)),
        island: fileURLToPath(new URL("./island.html", import.meta.url)),
      },
      output: {
        // Vite 8 使用 rolldown，不支持 object 式 manualChunks（会报
        // "manualChunks is not a function"），须用函数式按模块 id 分组。
        manualChunks(id) {
          const m = id.replace(/\\/g, "/");
          if (m.includes("/node_modules/vue/") || m.includes("/node_modules/vue-router/")) {
            return "vue-vendor";
          }
          if (m.includes("/node_modules/@tauri-apps/")) {
            return "tauri-vendor";
          }
          if (m.includes("/node_modules/xterm") || m.includes("/node_modules/@xterm/")) {
            return "xterm-vendor";
          }
          if (m.includes("/node_modules/codemirror") || m.includes("/node_modules/@codemirror")) {
            return "editor-vendor";
          }
          // shadcn/ui 组件池：被多个路由视图共享，单独立块避免重复打包
          if (m.includes("/components/ui/")) {
            return "shadcn-vendor";
          }
          // vue-sonner 被 toast.js 和 Sonner.vue 引用，拆分后可独立缓存
          if (m.includes("/node_modules/vue-sonner")) {
            return "sonner-vendor";
          }
        },
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    headers: {
      "Cache-Control": "no-store, no-cache, must-revalidate, proxy-revalidate",
      Pragma: "no-cache",
      Expires: "0",
    },
    // 监听策略：默认使用 inotify（零 CPU 空转），仅当检测到 ENOSPC 或
    // 显式设置 VITE_USE_POLLING=1 时回退到轮询，避免全量轮询常驻占 CPU。
    // 根因修复：sudo sysctl -w fs.inotify.max_user_watches=524288
    watch: {
      usePolling: !!process.env.VITE_USE_POLLING,
      interval: process.env.VITE_USE_POLLING ? 2000 : undefined,
      ignored: ["**/src-tauri/**", "**/target/**", "**/node_modules/**", "**/dist/**", "**/.git/**"],
    },
    hmr: {
      overlay: false,
    },
  },
});
