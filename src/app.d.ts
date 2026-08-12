/// <reference types="vite/client" />

declare module "*.css";

declare global {
  interface Window {
    /** Tauri 运行时注入的内部对象（webview 环境存在，浏览器降级时不存在） */
    __TAURI_INTERNALS__?: unknown;
  }
}

export {};
