// src-tauri/src/commands/window_factory.rs — 主窗口创建工厂
// 职责：按 tauri.conf.json 中 "main" 窗口的参数创建主 WebView 窗口。
// 集中此逻辑后，主窗口在「关闭转后台」时被 destroy()，
// 托盘「显示 DevNexus」/ 点击灵动岛时可按需重建——从而回收 WebKit 渲染进程（~260MB）。
//
// 注意：窗口参数必须与 tauri.conf.json 的 main 窗口保持一致，
// 否则重建出的窗口尺寸/装饰/背景色会与首次启动不一致。

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

/// 主窗口配置（与 tauri.conf.json `windows[0]` 对齐）
const MAIN_WIDTH: f64 = 1280.0;
const MAIN_HEIGHT: f64 = 800.0;
const MAIN_MIN_WIDTH: f64 = 960.0;
const MAIN_MIN_HEIGHT: f64 = 600.0;

/// 创建主窗口（若已存在则直接返回既有窗口，避免重复创建）
pub fn create_main_window(app: &tauri::AppHandle) -> tauri::WebviewWindow {
    if let Some(existing) = app.get_webview_window("main") {
        return existing;
    }
    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("DevNexus")
        .inner_size(MAIN_WIDTH, MAIN_HEIGHT)
        .min_inner_size(MAIN_MIN_WIDTH, MAIN_MIN_HEIGHT)
        .center()
        .decorations(false)
        .resizable(true)
        .shadow(false)
        .build()
        .unwrap_or_else(|e| {
            tracing::error!(
                error = %e,
                width = MAIN_WIDTH,
                height = MAIN_HEIGHT,
                "Failed to create main window. This may indicate a Tauri runtime issue."
            );
            panic!("Failed to create main window: {}", e);
        })
}

/// 显示主窗口：若不存在则先重建（主窗口可能因「关闭转后台」被 destroy）。
/// 供前端（托盘/灵动岛点击打开主窗口）调用，确保窗口被销毁后仍能恢复。
#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) {
    create_main_window(&app);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_skip_taskbar(false);
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}
