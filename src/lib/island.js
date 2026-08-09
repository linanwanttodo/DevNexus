// src/lib/island.js — 灵动岛窗口控制桥（主应用侧）
// 负责按持久化状态显示/隐藏 island 窗口；dpi/位置逻辑在岛窗自身维护。
import { getIslandEnabled } from "./stores.js";

async function getIslandWindow() {
  try {
    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    let win = WebviewWindow.getByLabel("island");
    if (win) return win;
    // 兜底：某些平台/启动时序下配置的可见窗口不会自动创建（getByLabel 为 null，
    // 所有控制函数会静默失效，表现为"开关点了没用"）。
    // 主动创建，参数与 tauri.conf.json 的 island 窗口保持一致。
    win = new WebviewWindow("island", {
      title: "DevNexus Island",
      url: "island.html",
      width: 248,
      height: 60,
      decorations: false,
      transparent: true,
      alwaysOnTop: true,
      shadow: false,
      resizable: false,
      skipTaskbar: true,
      visible: false,
      backgroundColor: [0, 0, 0, 0],
    });
    return win;
  } catch {
    return null; // 非 Tauri 环境
  }
}

/**
 * 按当前启用状态同步悬浮窗：
 *   - 启用 → show + 置顶；岛窗自身会恢复上次位置
 *   - 禁用 → hide
 * 在应用启动恢复与设置页切换时调用，幂等。
 */
export async function applyIslandState() {
  const win = await getIslandWindow();
  if (!win) return;
  try {
    if (getIslandEnabled().value) {
      await win.show();
      await win.setAlwaysOnTop(true);
    } else {
      await win.hide();
    }
  } catch {
    // 窗口创建失败/已销毁等异常静默，不影响主应用
  }
}

/** 立即显示悬浮窗（设置页"预览"等功能用） */
export async function showIslandWindow() {
  const win = await getIslandWindow();
  if (!win) return;
  try {
    await win.show();
    await win.setAlwaysOnTop(true);
  } catch {
    // ignore
  }
}

/** 立即隐藏悬浮窗（不改变启用状态） */
export async function hideIslandWindow() {
  const win = await getIslandWindow();
  if (!win) return;
  try {
    await win.hide();
  } catch {
    // ignore
  }
}

/** 查询悬浮窗当前是否可见（设置页状态徽章用） */
export async function isIslandVisible() {
  const win = await getIslandWindow();
  if (!win) return false;
  try {
    return await win.isVisible();
  } catch {
    return false;
  }
}