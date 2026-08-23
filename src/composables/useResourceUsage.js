// src/composables/useResourceUsage.js — 系统资源用量单例轮询
//
// 职责：
//   1. 维护一个全局的 reactive resourceUsage 对象
//   2. 每 30s 调用一次 rust get_resource_usage，页面隐藏时暂停
//   3. 被多个组件（Dashboard / Sidebar）共享，避免重复锁竞争
//   4. 首次调用 start() 启动轮询，多次调用安全
//
// 用法：
//   import { useResourceUsage } from "../composables/useResourceUsage.js";
//   const { resourceUsage, start, stop } = useResourceUsage();
//   onMounted(start);
//   onBeforeUnmount(stop);

import { ref, shallowRef } from "vue";
import { invoke } from "@tauri-apps/api/core";

const POLL_INTERVAL_MS = 30000;

// 模块级单例
const resourceUsage = shallowRef(null);
let timer = null;
let visibleListener = null;
let refCount = 0;

function onVisibility() {
  if (!document.hidden) {
    refresh();
  }
}

async function refresh() {
  try {
    resourceUsage.value = await invoke("get_resource_usage");
  } catch {
    // silently fail (no Tauri context)
  }
}

function start() {
  refCount++;
  if (timer) return; // already running
  refresh();
  timer = setInterval(() => {
    if (document.hidden) return;
    refresh();
  }, POLL_INTERVAL_MS);
  visibleListener = onVisibility;
  document.addEventListener("visibilitychange", onVisibility);
}

function stop() {
  refCount--;
  if (refCount > 0) return; // another component still needs it
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
  if (visibleListener) {
    document.removeEventListener("visibilitychange", onVisibility);
    visibleListener = null;
  }
}

export function useResourceUsage() {
  return { resourceUsage, start, stop };
}