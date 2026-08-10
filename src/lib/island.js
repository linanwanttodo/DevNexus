// src/lib/island.js — 灵动岛窗口控制桥（主应用侧）
// 负责按持久化状态显示/隐藏 island 窗口；dpi/位置逻辑在岛窗自身维护。
// 多显示器：每个显示器一个岛窗口实例（label: island-0/island-1/...），
// 所有实例统一显示/隐藏，位置各自按显示器隔离持久化。
import { getIslandEnabled } from "./stores.js";

/**
 * 获取/创建所有显示器上的岛窗口实例。
 * 主显示器沿用配置的 "island" label；其他显示器按索引创建 island-<n>。
 * 每个实例创建时直接定位到对应显示器顶部居中（逻辑像素），
 * 避免窗口先落在主屏、再由 JS 跨屏搬移造成闪烁。
 */
async function getIslandWindows() {
  try {
    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const { availableMonitors, primaryMonitor } = await import("@tauri-apps/api/window");
    const [monitors, primary] = await Promise.all([availableMonitors(), primaryMonitor()]);
    if (!monitors.length) return [];
    const dpr = window.devicePixelRatio || 1;
    const wins = [];
    for (let i = 0; i < monitors.length; i++) {
      const m = monitors[i];
      const isPrimary = !!primary && m.name === primary.name;
      const label = isPrimary ? "island" : `island-${i}`;
      // getByLabel 是 async：必须 await，否则拿到 Promise（永远 truthy），
      // 导致既有窗口被误判、新实例永不创建、show() 静默失败。
      let win = await WebviewWindow.getByLabel(label);
      if (!win) {
        // 顶部居中：逻辑坐标 = 物理坐标 / dpr
        // 窗口初始逻辑尺寸按收起态 256×48 计算，岛窗自身会再精确调整
        const x = Math.round((m.position.x + (m.size.width - 256 * dpr) / 2) / dpr);
        const y = Math.round(m.position.y / dpr + 12);
        win = new WebviewWindow(label, {
          title: "DevNexus Island",
          url: "island.html",
          x,
          y,
          width: 256,
          height: 48,
          decorations: false,
          transparent: true,
          alwaysOnTop: true,
          shadow: false,
          resizable: false,
          skipTaskbar: true,
          visible: false,
          backgroundColor: [0, 0, 0, 0],
        });
      }
      wins.push(win);
    }
    return wins;
  } catch {
    return []; // 非 Tauri 环境
  }
}

/** 置顶 + 所有工作区可见（灵动岛是全局悬浮窗，切换虚拟桌面/工作区不消失） */
async function promoteWindow(win) {
  await win.setAlwaysOnTop(true);
  try {
    await win.setVisibleOnAllWorkspaces(true);
  } catch {
    // 某些平台不支持，忽略
  }
}

/**
 * 按当前启用状态同步所有显示器悬浮窗：
 *   - 启用 → 全部 show + 置顶；各岛窗自身会恢复本显示器上的上次位置
 *   - 禁用 → 全部 hide
 * 在应用启动恢复与设置页切换时调用，幂等。
 */
export async function applyIslandState() {
  const wins = await getIslandWindows();
  for (const win of wins) {
    try {
      if (getIslandEnabled().value) {
        await win.show();
        await promoteWindow(win);
      } else {
        await win.hide();
      }
    } catch {
      // 窗口创建失败/已销毁等异常静默，不影响主应用
    }
  }
}

/** 立即显示所有悬浮窗（设置页"预览"等功能用） */
export async function showIslandWindow() {
  const wins = await getIslandWindows();
  for (const win of wins) {
    try {
      await win.show();
      await promoteWindow(win);
    } catch {
      // ignore
    }
  }
}

/** 立即隐藏所有悬浮窗（不改变启用状态） */
export async function hideIslandWindow() {
  const wins = await getIslandWindows();
  for (const win of wins) {
    try {
      await win.hide();
    } catch {
      // ignore
    }
  }
}

/** 查询悬浮窗当前是否可见（设置页状态徽章用；任一显示器可见即视为可见） */
export async function isIslandVisible() {
  const wins = await getIslandWindows();
  for (const win of wins) {
    try {
      if (await win.isVisible()) return true;
    } catch {
      // ignore
    }
  }
  return false;
}
