# 灵动岛 — 模块设计文档

## 1. 功能概述

灵动岛（Dynamic Island）是一个全局悬浮的胶囊状小窗口，常驻所有窗口之上，提供：

- **系统状态 HUD**：音量、亮度实时百分比显示（`island_get_hud`）
- **通知融合**：监听系统通知（Linux D-Bus `org.freedesktop.Notifications`），把微信/QQ 等应用通知融入岛内横幅，点击横幅可展开（`island-notify` 事件）
- **媒体控制**：读取当前播放器状态并支持播放/暂停/上一首/下一首（Linux MPRIS，`island_media_status` / `island_media_control`）
- **跨工作区可见**：GNOME/mutter 下监听 `_NET_CURRENT_DESKTOP` 切换，把岛窗口移动到当前工作区，等效于"每个工作区都常驻"
- **两态显示**：默认显示时间；有内容时展开成横幅（通知、媒体、HUD）

**通信链路**:
```
系统通知 (D-Bus) ──→ island_bridge.rs (BecomeMonitor)
                 ──→ emit_to("island", "island-notify", {app, title, body})
                 ──→ IslandApp.vue（独立窗口 island.html）渲染横幅

IslandApp.vue ──→ invoke("island_get_hud")         → 音量/亮度 HUD
              ──→ invoke("island_media_status")    → 媒体状态
              ──→ invoke("island_media_control")   → 媒体控制
              ──→ invoke("island_set_sticky")      → 跨工作区置顶
```

---

## 2. 核心命令

| 命令 | 说明 |
|------|------|
| `island_get_enabled` / `island_set_enabled` | 读取/切换灵动岛开关（状态文件持久化） |
| `island_get_hud` | 返回音量与亮度百分比（`IslandHud`） |
| `island_media_status` | 返回当前媒体播放状态（Linux MPRIS；非 Linux 返回 None） |
| `island_media_control` | 执行播放/暂停/上一首/下一首（Linux；非 Linux 返回错误） |
| `island_set_sticky` | 设置岛窗口跨工作区/跨显示器可见 |

---

## 3. 平台实现

### 3.1 Linux（完整功能）

- **通知监听**：通过 D-Bus `BecomeMonitor` 成为总线监控者，过滤 `org.freedesktop.Notifications.Notify` 方法调用，解析 `app_name / summary / body` 后推送到岛窗口；跳过自家（含 "devnexus" 的 app_name）避免循环。权限不足时静默降级（灵动岛通知功能禁用，不影响其他功能）。
- **跨工作区**：mutter(Wayland) 下 `_NET_WM_STATE_STICKY` 不可靠，改用 `x11-dl` 读取 `_NET_CURRENT_DESKTOP`，通过 GDK 把岛窗口 `move_to_desktop(当前工作区)`；另用 `_NET_WM_STATE_STICKY` + `_NET_WM_STATE_ABOVE` 置顶（XWayland 会话）。
- **媒体控制**：通过 `org.mpris.MediaPlayer2` 会话总线枚举播放器（`list_players`），读取 `Metadata` / `PlaybackStatus`，控制方法走 `playerctl`-style 的 Play/Pause/Next/Previous 调用。

### 3.2 macOS / Windows

- 媒体状态与控制返回 `None` / 错误（暂未实现），通知监听不启动；其余 HUD / 开关 / 窗口行为仍可用。

---

## 4. 窗口与前端

- 独立窗口在 `tauri.conf.json` 中声明（label `island`），`url: "island.html"`，透明、置顶、无边框、跳过任务栏、默认不可见。
- 前端入口：`src/island/IslandApp.vue` + `src/island/main.js`（独立于主应用入口 `main.js`）。
- 主窗口侧边栏/托盘可通过 `island_set_enabled` 开关；托盘菜单项为勾选开关（勾选=开）。
- 数据桥初始化在 `lib.rs` 的 `setup` 中调用 `commands::island_bridge::init(app.handle().clone())`。

---

## 5. 关键文件

| 文件 | 说明 |
|------|------|
| `src-tauri/src/commands/island_bridge.rs` | 通知监听、媒体控制、HUD、跨工作区、开关与 DeepSeek 余额 |
| `src/island/IslandApp.vue` | 岛窗口 UI（时间/横幅/媒体/HUD） |
| `src/island/island.css` | 岛窗口样式 |
| `src/island/main.js` | 岛窗口 Vue 入口 |
| `src/views/IslandSettings.vue` | 主窗口中的灵动岛设置页（路由 `/island`） |
| `src-tauri/src/commands/tray.rs` | 托盘菜单中的灵动岛开关项 |
