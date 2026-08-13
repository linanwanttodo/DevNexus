// src-tauri/src/commands/island_bridge.rs — 灵动岛数据桥（Linux）
// 职责：
//   1. MPRIS 媒体控制：查询当前播放器（曲目/艺术家/播放状态），提供 播放/暂停/上一首/下一首
//   2. 系统通知监听：BecomeMonitor 旁听 org.freedesktop.Notifications 的 Notify 调用，
//      把微信/QQ 等应用通知转发为 island-notify 事件，灵动岛窗口显示横幅（类似 iPhone/Mac）。
// 仅 Linux 实现（D-Bus/MPRIS 均为 Linux 生态），其他平台返回 None/Err，前端自动隐藏对应 UI。

use serde::Serialize;

/// 当前媒体播放状态（MPRIS）
#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaStatus {
    /// 播放器总线名（如 org.mpris.MediaPlayer2.spotify）
    pub player: String,
    /// 曲目名
    pub title: Option<String>,
    /// 艺术家
    pub artist: Option<String>,
    /// Playing / Paused / Stopped
    pub status: String,
    /// 曲目总长（毫秒）
    pub length_ms: Option<i64>,
}

#[cfg(target_os = "linux")]
mod imp {
    use super::MediaStatus;
    use dbus::arg::RefArg;
    use dbus::blocking::{BlockingSender, Connection};
    use dbus::Message;
    use std::time::Duration;

    const TIMEOUT: Duration = Duration::from_secs(2);

    fn session() -> Option<Connection> {
        Connection::new_session().ok()
    }

    /// 列出所有 MPRIS 播放器总线名
    fn list_players(conn: &Connection) -> Option<Vec<String>> {
        let reply = conn
            .send_with_reply_and_block(
                Message::new_method_call(
                    "org.freedesktop.DBus",
                    "/org/freedesktop/DBus",
                    "org.freedesktop.DBus",
                    "ListNames",
                )
                .ok()?,
                TIMEOUT,
            )
            .ok()?;
        // ListNames 返回签名是 `as`（字符串数组），不是 `(as)` 元组——
        // 之前用 `(Vec<String>,)` 解析永远 TypeMismatch，导致播放器识别不到。
        let names: Vec<String> = reply.read1().ok()?;
        Some(
            names
                .into_iter()
                .filter(|n| n.starts_with("org.mpris.MediaPlayer2."))
                .collect(),
        )
    }

    /// 读取播放器某个属性（Properties.Get 返回 Variant<T>）
    fn get_prop<T: dbus::arg::Arg + for<'z> dbus::arg::Get<'z>>(
        conn: &Connection,
        player: &str,
        prop: &str,
    ) -> Option<T> {
        let msg = Message::new_method_call(
            player,
            "/org/mpris/MediaPlayer2",
            "org.freedesktop.DBus.Properties",
            "Get",
        )
        .ok()?
        .append2("org.mpris.MediaPlayer2.Player", prop);
        let reply = conn.send_with_reply_and_block(msg, TIMEOUT).ok()?;
        let v: dbus::arg::Variant<T> = reply.read1().ok()?;
        Some(v.0)
    }

    /// 读取曲目元数据（Metadata: a{sv}）
    fn read_metadata(
        conn: &Connection,
        player: &str,
    ) -> (Option<String>, Option<String>, Option<i64>) {
        let Some(map) = get_prop::<dbus::arg::PropMap>(conn, player, "Metadata") else {
            return (None, None, None);
        };
        let title = map
            .get("xesam:title")
            .and_then(|v| v.as_str().map(String::from));
        // 艺术家是字符串数组，取第一个
        let artist = map.get("xesam:artist").and_then(|v| {
            v.as_iter()
                .and_then(|mut it| it.next())
                .and_then(|a| a.as_str().map(String::from))
        });
        let length = map.get("mpris:length").and_then(|v| v.as_i64());
        (title, artist, length)
    }

    /// 优先返回正在播放的播放器，否则第一个
    fn pick_player(conn: &Connection) -> Option<String> {
        let players = list_players(conn)?;
        for p in &players {
            if let Some(status) = get_prop::<String>(conn, p, "PlaybackStatus") {
                if status == "Playing" {
                    return Some(p.clone());
                }
            }
        }
        players.into_iter().next()
    }

    pub fn media_status() -> Option<MediaStatus> {
        let conn = session()?;
        let player = pick_player(&conn)?;
        let status = get_prop::<String>(&conn, &player, "PlaybackStatus").unwrap_or_default();
        let (title, artist, length) = read_metadata(&conn, &player);
        Some(MediaStatus {
            player,
            title,
            artist,
            status,
            length_ms: length,
        })
    }

    pub fn media_control(action: &str) -> Result<(), String> {
        let conn = session().ok_or("D-Bus session unavailable")?;
        let player = pick_player(&conn).ok_or("no MPRIS media player found")?;
        let member = match action {
            "play_pause" => "PlayPause",
            "next" => "Next",
            "previous" => "Previous",
            "stop" => "Stop",
            _ => return Err(format!("unknown action: {action}")),
        };
        let msg = Message::new_method_call(
            &player,
            "/org/mpris/MediaPlayer2",
            "org.mpris.MediaPlayer2.Player",
            member,
        )
        .map_err(|e| e.to_string())?;
        conn.send_with_reply_and_block(msg, TIMEOUT)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 后台线程：BecomeMonitor 旁听系统通知，转发为 island-notify 事件
    pub fn start_notification_listener(app: tauri::AppHandle) {
        std::thread::spawn(move || {
            let Ok(conn) = Connection::new_session() else {
                return;
            };
            // 请求成为总线监控者（监听 Notify 方法调用；权限不足则静默降级）
            let rule =
                "interface='org.freedesktop.Notifications',member='Notify',type='method_call'"
                    .to_string();
            let monitor = Message::new_method_call(
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus.Monitoring",
                "BecomeMonitor",
            )
            .ok()
            .map(|m| m.append2(vec![rule], 0u32));
            let Some(monitor) = monitor else { return };
            if conn.send_with_reply_and_block(monitor, TIMEOUT).is_err() {
                eprintln!(
                    "[DevNexus] notification monitor not permitted; island notifications disabled"
                );
                return;
            }
            // BecomeMonitor 后连接转为监控模式，只能接收消息，不能正常收发方法调用
            loop {
                let msg = match conn
                    .channel()
                    .blocking_pop_message(Duration::from_millis(500))
                {
                    Ok(Some(m)) => m,
                    _ => continue,
                };
                if msg.interface().as_deref() != Some("org.freedesktop.Notifications") {
                    continue;
                }
                if msg.member().as_deref() != Some("Notify") {
                    continue;
                }
                // Notify 参数: app_name, replaces_id, app_icon, summary, body, actions, hints, expire_timeout
                let mut it = msg.iter_init();
                let app_name: String = it.read().unwrap_or_default();
                let _replaces: u32 = it.read().unwrap_or_default();
                let _icon: String = it.read().unwrap_or_default();
                let summary: String = it.read().unwrap_or_default();
                let body: String = it.read().unwrap_or_default();
                // 跳过自家通知，避免循环
                if app_name.to_lowercase().contains("devnexus") {
                    continue;
                }
                if summary.is_empty() && body.is_empty() {
                    continue;
                }
                let payload = serde_json::json!({
                    "app": app_name,
                    "title": summary,
                    "body": body,
                });
                use tauri::Emitter;
                let _ = app.emit_to("island", "island-notify", payload);
            }
        });
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    use super::MediaStatus;

    pub fn media_status() -> Option<MediaStatus> {
        None
    }

    pub fn media_control(_action: &str) -> Result<(), String> {
        Err("media control not supported on this platform".into())
    }

    pub fn start_notification_listener(_app: tauri::AppHandle) {
        // 非 Linux：不启动通知监听
    }
}

/// 查询当前媒体播放状态（前端轮询调用）
/// 必须 async：D-Bus 调用是阻塞式（2s 超时），同步命令会跑在主线程，
/// 每 3 秒一次的轮询会把整个 UI 主线程卡死（窗口拖不动、点击无响应）。
/// spawn_blocking 把阻塞调用挪到线程池，主线程始终空闲。
#[tauri::command]
pub async fn island_media_status() -> Option<MediaStatus> {
    tauri::async_runtime::spawn_blocking(imp::media_status)
        .await
        .ok()
        .flatten()
}

/// 媒体控制：play_pause / next / previous / stop
/// 同样 async + spawn_blocking，避免阻塞主线程。
#[tauri::command]
pub async fn island_media_control(action: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || imp::media_control(&action))
        .await
        .map_err(|e| e.to_string())?
}

/// 启动系统通知监听（在 setup 中调用一次）
pub fn init(app: tauri::AppHandle) {
    imp::start_notification_listener(app.clone());
    // 工作区跟随：mutter(Wayland) 下 STICKY 不可靠，改为监听工作区切换、移动岛窗口
    start_workspace_follower(app);
}

// ═══════════════ 跨工作区可见（GNOME/mutter 兼容）═══════════════
// 用户环境是 Wayland 会话（GNOME + XWayland，应用强制 GDK_BACKEND=x11）：
// mutter 对 XWayland 窗口的 _NET_WM_STATE_STICKY（0xFFFFFFFF）支持不可靠——
// 实测设置了 STICKY 属性后切工作区窗口仍不显示。
// 可靠方案：监听 _NET_CURRENT_DESKTOP 变化，把岛窗口 move_to_desktop(当前工作区)，
// 窗口始终跟随当前工作区显示（等效于"每个工作区都常驻"）。

/// 读取当前工作区号（_NET_CURRENT_DESKTOP，root window 属性）
#[cfg(target_os = "linux")]
fn current_desktop() -> Option<u32> {
    use x11_dl::xlib::{Xlib, XA_CARDINAL};
    let xlib = Xlib::open().ok()?;
    unsafe {
        let display = (xlib.XOpenDisplay)(std::ptr::null());
        if display.is_null() {
            return None;
        }
        let root = (xlib.XDefaultRootWindow)(display);
        let atom = (xlib.XInternAtom)(display, c"_NET_CURRENT_DESKTOP".as_ptr(), 0);
        let mut actual_type: std::os::raw::c_ulong = 0;
        let mut actual_format: std::os::raw::c_int = 0;
        let mut nitems: std::os::raw::c_ulong = 0;
        let mut bytes_after: std::os::raw::c_ulong = 0;
        let mut data: *mut u8 = std::ptr::null_mut();
        let status = (xlib.XGetWindowProperty)(
            display,
            root,
            atom,
            0,
            1,
            0,
            XA_CARDINAL,
            &mut actual_type,
            &mut actual_format,
            &mut nitems,
            &mut bytes_after,
            &mut data,
        );
        let result = if status == 0 && !data.is_null() && nitems >= 1 {
            Some(*(data as *const std::os::raw::c_ulong) as u32)
        } else {
            None
        };
        if !data.is_null() {
            (xlib.XFree)(data as *mut std::ffi::c_void);
        }
        (xlib.XCloseDisplay)(display);
        result
    }
}

/// 把所有岛窗口移动到指定工作区（走 GDK 自身连接，安全）
#[cfg(target_os = "linux")]
fn move_island_to_desktop(app: &tauri::AppHandle, desktop: u32) {
    use tauri::Manager; // webview_windows()
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        for (label, win) in app.webview_windows() {
            if label.starts_with("island") {
                use gdkx11::X11Window;
                use gtk::prelude::*;
                if let Ok(gtk_win) = win.gtk_window() {
                    if let Some(gdk_win) = gtk_win.window() {
                        if let Some(x11_win) = gdk_win.downcast_ref::<X11Window>() {
                            x11_win.move_to_desktop(desktop);
                        }
                    }
                }
            }
        }
    });
}

/// 后台线程：监听工作区切换，岛窗口跟随到当前工作区
#[cfg(target_os = "linux")]
pub fn start_workspace_follower(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut last: Option<u32> = None;
        loop {
            let cur = current_desktop();
            if cur != last {
                last = cur;
                if let Some(d) = cur {
                    move_island_to_desktop(&app, d);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
}

#[cfg(not(target_os = "linux"))]
pub fn start_workspace_follower(_app: tauri::AppHandle) {
    // 非 Linux：无需跟随
}

#[cfg(target_os = "linux")]
fn set_sticky_x11(window: &tauri::Window) -> Result<(), String> {
    use gdkx11::X11Window;
    use gtk::prelude::*; // GtkWindowExt::window() + Cast::downcast_ref

    // 1) 通过 GTK 拿到窗口 XID
    let gtk_win = window.gtk_window().map_err(|e| e.to_string())?;
    let gdk_win = gtk_win.window().ok_or("no gdk window")?;
    let x11_win = gdk_win
        .downcast_ref::<X11Window>()
        .ok_or("not an X11 window")?;

    // 2) Wayland 会话下 STICKY(0xFFFFFFFF) 无效：改为钉到当前工作区，
    //    窗口跟随当前工作区显示；后续由 start_workspace_follower 持续跟随。
    if let Some(desktop) = current_desktop() {
        x11_win.move_to_desktop(desktop);
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn set_sticky_x11(_window: &tauri::Window) -> Result<(), String> {
    Ok(())
}

/// 让指定岛窗口在所有工作区可见（X11 直接写 STICKY，不依赖 GTK stick）
#[tauri::command]
pub fn island_set_sticky(window: tauri::Window) -> Result<(), String> {
    set_sticky_x11(&window)
}

// ═══════════════ 灵动岛开关状态（托盘 check 项 / 前端共享）═══════════════
// 持久化到 data_dir/island_enabled（"1"/"0"），前端 stores.js 与托盘 check 项
// 都读写此状态，保证两边一致；默认开启（与前端行为一致）。

fn island_enabled_path() -> std::path::PathBuf {
    crate::utils::data_dir().join("island_enabled")
}

/// 读取灵动岛开关状态（默认开启）
#[tauri::command]
pub fn island_get_enabled() -> bool {
    std::fs::read_to_string(island_enabled_path())
        .map(|s| s.trim() == "1")
        .unwrap_or(true)
}

/// 设置灵动岛开关状态并同步所有岛窗口显示/隐藏
#[tauri::command]
pub fn island_set_enabled(enabled: bool, app: tauri::AppHandle) -> Result<(), String> {
    if let Err(e) = std::fs::create_dir_all(crate::utils::data_dir()) {
        eprintln!("[DevNexus] cannot create data dir for island_enabled: {e}");
    }
    let _ = std::fs::write(island_enabled_path(), if enabled { "1" } else { "0" });
    // 同步所有岛窗口显示状态（托盘点击时主窗口可能未打开，这里直接控制窗口）
    use tauri::Manager;
    for (label, win) in app.webview_windows() {
        if label.starts_with("island") {
            if enabled {
                let _ = win.show();
                let _ = win.set_always_on_top(true);
                let _ = win.set_visible_on_all_workspaces(true);
            } else {
                // 关键：Wayland 下被 set_visible_on_all_workspaces(true) 的窗口，
                // 直接 hide() 在某些 compositor 上不生效（窗口仍残留可见）。
                // 必须先取消 all-workspaces 置顶/跨桌面属性，再 hide 才能可靠隐藏。
                // 否则托盘点了「灵动岛」也关不掉——正是用户反馈的现象。
                let _ = win.set_visible_on_all_workspaces(false);
                let _ = win.set_always_on_top(false);
                let _ = win.hide();
            }
        }
    }
    // 同步托盘菜单文字：无论从设置页、侧边栏还是托盘切换，菜单都显示当前状态
    let lang = crate::commands::tray::saved_lang();
    crate::commands::tray::update_island_menu_text(&app, &lang, enabled);
    // 广播状态给主窗口：主应用侧边栏开关与 applyIslandState 以 localStorage 为准，
    // 若不回写，托盘关掉窗口后下次 applyIslandState 会按默认 true 把岛重新 show 出来。
    use tauri::Emitter;
    let _ = app.emit_to("main", "island-state", enabled);
    Ok(())
}

// ═══════════════ DeepSeek 余额查询 ═══════════════
// 官方接口：GET https://api.deepseek.com/user/balance（Bearer API Key）
// 返回 is_available + balance_infos[]（currency / total_balance / granted_balance / topped_up_balance）
//
// API Key 存储：主窗口（设置页）写入 → data_dir 文件（0600）+ Rust 内存缓存，
// 岛窗口从内存/文件读取。不在两个窗口间共享 localStorage：Tauri 多窗口各自持有
// 独立 WebView，localStorage 按 origin 隔离，island 窗口读不到主窗口写入的 key，
// 表现为"填了 key 岛还是显示未配置"。用 Rust 侧命令读写绕开该隔离；
// 且必须落盘——仅存内存的话应用重启后 key 丢失，重启后余额又变成"未配置"。

use std::sync::{Mutex, OnceLock};

const KEY_FILE: &str = "deepseek_api_key";

/// 进程内 DeepSeek API Key 缓存（写盘时同步更新；读时若为空则从磁盘加载）
fn deepseek_key_store() -> &'static Mutex<Option<String>> {
    static KEY: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    KEY.get_or_init(|| Mutex::new(None))
}

fn key_file_path() -> std::path::PathBuf {
    crate::utils::data_dir().join(KEY_FILE)
}

fn load_key_from_disk() -> Option<String> {
    std::fs::read_to_string(key_file_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn save_key_to_disk(key: &str) {
    if let Err(e) = std::fs::create_dir_all(crate::utils::data_dir()) {
        eprintln!("[DevNexus] cannot create data dir for deepseek key: {e}");
        return;
    }
    let path = key_file_path();
    if std::fs::write(&path, key).is_ok() {
        // 仅本用户可读（Unix）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
    } else {
        eprintln!("[DevNexus] cannot persist deepseek key");
    }
}

/// 保存 DeepSeek API Key（设置页调用；主窗口与岛窗口共享此值，重启后仍在）
#[tauri::command]
pub fn deepseek_set_key(key: String) {
    let trimmed = key.trim().to_string();
    let mut store = deepseek_key_store().lock().unwrap();
    *store = Some(trimmed.clone());
    drop(store);
    save_key_to_disk(&trimmed);
}

/// 读取 DeepSeek API Key（岛窗口查询余额前调用；内存为空时从磁盘恢复）
#[tauri::command]
pub fn deepseek_get_key() -> String {
    let mut store = deepseek_key_store().lock().unwrap();
    if let Some(k) = store.as_ref() {
        return k.clone();
    }
    // 首次读取：从磁盘加载（重启后恢复）
    if let Some(k) = load_key_from_disk() {
        *store = Some(k.clone());
        return k;
    }
    String::new()
}

/// DeepSeek 余额信息（单个币种）
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeepSeekBalanceInfo {
    /// 币种：CNY / USD
    pub currency: String,
    /// 总余额（含赠送与充值）
    pub total_balance: String,
    /// 未过期赠送余额
    pub granted_balance: String,
    /// 充值余额
    pub topped_up_balance: String,
}

/// DeepSeek 余额查询结果
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeepSeekBalance {
    /// 余额是否足够调用 API
    pub is_available: bool,
    /// 各币种余额列表
    pub balance_infos: Vec<DeepSeekBalanceInfo>,
}

#[derive(serde::Deserialize, Debug)]
struct BalanceResp {
    is_available: bool,
    balance_infos: Vec<BalanceInfoResp>,
}

#[derive(serde::Deserialize, Debug)]
struct BalanceInfoResp {
    currency: String,
    total_balance: String,
    granted_balance: String,
    topped_up_balance: String,
}

/// 查询 DeepSeek 账户余额（key 从进程内 store 读取，由设置页写入；不从前端传参，
/// 避免 key 在 IPC 参数中反复传递）
#[tauri::command]
pub async fn deepseek_get_balance() -> Result<DeepSeekBalance, String> {
    let api_key = deepseek_get_key();
    if api_key.trim().is_empty() {
        return Err("DeepSeek API Key is empty".into());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get("https://api.deepseek.com/user/balance")
        .header("Accept", "application/json")
        .bearer_auth(api_key.trim())
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {status}: {body}"));
    }
    let data: BalanceResp = resp.json().await.map_err(|e| e.to_string())?;
    Ok(DeepSeekBalance {
        is_available: data.is_available,
        balance_infos: data
            .balance_infos
            .into_iter()
            .map(|b| DeepSeekBalanceInfo {
                currency: b.currency,
                total_balance: b.total_balance,
                granted_balance: b.granted_balance,
                topped_up_balance: b.topped_up_balance,
            })
            .collect(),
    })
}

// ═══════════════ HUD：音量 / 亮度（只读快照）═══════════════
// 参考调研文档功能优先级「系统 HUD 替换（音量/亮度）」。
// 只做静态读取，不执行任何系统修改，失败时返回 None（前端显示 "--"）。

/// 系统 HUD 快照
#[derive(serde::Serialize, Clone, Debug)]
pub struct IslandHud {
    /// 音量百分比 0-100；读取失败为 None
    pub volume_percent: Option<f32>,
    /// 亮度百分比 0-100；读取失败为 None
    pub brightness_percent: Option<f32>,
}

/// 读取当前音量百分比（Linux PulseAudio/WirePlumber，通过 pactl 查询）
#[cfg(target_os = "linux")]
fn read_volume_percent() -> Option<f32> {
    let out = std::process::Command::new("pactl")
        .args(["get-sink-volume", "@DEFAULT_SINK@"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // 输出形如: Volume: front-left: 65536 / 100% / 0.00 dB, front-right: ...
    let pct = text
        .split('%')
        .next()
        .and_then(|s| s.split_whitespace().last())
        .and_then(|v| v.parse::<f32>().ok())?;
    Some(pct.clamp(0.0, 100.0))
}

/// 读取当前亮度百分比（Linux 通过 brightnessctl 或 sysfs）
#[cfg(target_os = "linux")]
fn read_brightness_percent() -> Option<f32> {
    // 优先 brightnessctl（常见于笔记本）
    if let Ok(out) = std::process::Command::new("brightnessctl")
        .args(["info"])
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            // 输出形如: Current brightness: 400 (30%)
            if let Some(pct) = text
                .lines()
                .find(|l| l.contains('(') && l.contains('%'))
                .and_then(|l| l.split('(').nth(1))
                .and_then(|s| s.split('%').next())
                .and_then(|s| s.trim().parse::<f32>().ok())
            {
                return Some(pct.clamp(0.0, 100.0));
            }
        }
    }
    // 兜底 sysfs：/sys/class/backlight/*/brightness 与 max_brightness
    let dirs = std::fs::read_dir("/sys/class/backlight").ok()?;
    for entry in dirs.flatten() {
        let base = entry.path();
        let cur = std::fs::read_to_string(base.join("brightness"))
            .ok()?
            .trim()
            .parse::<f32>()
            .ok()?;
        let max = std::fs::read_to_string(base.join("max_brightness"))
            .ok()?
            .trim()
            .parse::<f32>()
            .ok()?;
        if max > 0.0 {
            return Some((cur / max * 100.0).clamp(0.0, 100.0));
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn read_volume_percent() -> Option<f32> {
    None
}

#[cfg(not(target_os = "linux"))]
fn read_brightness_percent() -> Option<f32> {
    None
}

/// 查询系统 HUD（音量/亮度）快照
#[tauri::command]
pub async fn island_get_hud() -> IslandHud {
    tauri::async_runtime::spawn_blocking(|| IslandHud {
        volume_percent: read_volume_percent(),
        brightness_percent: read_brightness_percent(),
    })
    .await
    .unwrap_or(IslandHud {
        volume_percent: None,
        brightness_percent: None,
    })
}
