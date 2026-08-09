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
        let (names,): (Vec<String>,) = reply.read1().ok()?;
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
#[tauri::command]
pub fn island_media_status() -> Option<MediaStatus> {
    imp::media_status()
}

/// 媒体控制：play_pause / next / previous / stop
#[tauri::command]
pub fn island_media_control(action: String) -> Result<(), String> {
    imp::media_control(&action)
}

/// 启动系统通知监听（在 setup 中调用一次）
pub fn init(app: tauri::AppHandle) {
    imp::start_notification_listener(app);
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
