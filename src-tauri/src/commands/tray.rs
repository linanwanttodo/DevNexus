// src-tauri/src/commands/tray.rs — 系统托盘菜单
// 托盘菜单文案跟随应用语言（前端切换语言时通过 update_tray_menu 更新），
// 语言持久化到 data_dir/app_lang，启动时据此构建菜单。

use tauri::Manager;

fn lang_file_path() -> std::path::PathBuf {
    crate::utils::data_dir().join("app_lang")
}

/// 读取持久化的语言（默认 en）
pub fn saved_lang() -> String {
    std::fs::read_to_string(lang_file_path()).unwrap_or_else(|_| "en".into())
}

/// 根据启用状态生成灵动岛菜单文字（例如"灵动岛：开"/"灵动岛：关"）
/// 让托盘菜单直接反映当前状态，用户一目了然，点击切换时文字同步变化。
pub fn island_label_by_state(lang: &str, enabled: bool) -> String {
    match lang {
        "zh" => {
            if enabled {
                "灵动岛：开".into()
            } else {
                "灵动岛：关".into()
            }
        }
        "ru" => {
            if enabled {
                "Остров: вкл".into()
            } else {
                "Остров: выкл".into()
            }
        }
        _ => {
            if enabled {
                "Dynamic Island: On".into()
            } else {
                "Dynamic Island: Off".into()
            }
        }
    }
}

/// 各语言的托盘菜单文案：show / island / check_update / quit
/// island 为不带状态的基名（状态文字由 island_label_by_state 按启用状态生成）
pub fn tray_texts(lang: &str) -> (String, String, String, String) {
    match lang {
        "zh" => (
            "显示 DevNexus".into(),
            "灵动岛".into(),
            "检查更新".into(),
            "退出".into(),
        ),
        "ru" => (
            "Показать DevNexus".into(),
            "Остров".into(),
            "Проверить обновления".into(),
            "Выход".into(),
        ),
        _ => (
            "Show DevNexus".into(),
            "Dynamic Island".into(),
            "Check for Updates".into(),
            "Quit".into(),
        ),
    }
}

/// 更新灵动岛菜单项文字为当前启用状态的显示（"开"/"关"）
pub fn update_island_menu_text(app: &tauri::AppHandle, lang: &str, checked: bool) {
    let menu = app.state::<tauri::menu::Menu<tauri::Wry>>().inner();
    if let Some(item) = menu.get("island") {
        if let Some(ci) = item.as_check_menuitem() {
            let label = island_label_by_state(lang, checked);
            let _ = ci.set_text(label);
            let _ = ci.set_checked(checked);
        }
    }
}

/// 余额菜单项初始文字（按语言）
pub fn balance_placeholder(lang: &str) -> String {
    match lang {
        "zh" => "DeepSeek 余额: —".into(),
        "ru" => "Баланс DeepSeek: —".into(),
        _ => "DeepSeek Balance: —".into(),
    }
}

/// 格式化余额为菜单文字（优先 CNY，其次 USD）
pub fn format_balance(b: &crate::commands::island_bridge::DeepSeekBalance) -> String {
    let info = b
        .balance_infos
        .iter()
        .find(|i| i.currency == "CNY")
        .or_else(|| b.balance_infos.first());
    match info {
        Some(i) => format!("DeepSeek 余额: {} {}", i.total_balance, i.currency),
        None => "DeepSeek 余额: —".into(),
    }
}

/// 更新指定菜单项文字
pub fn set_menu_item_text(app: &tauri::AppHandle, id: &str, text: String) {
    let menu = app.state::<tauri::menu::Menu<tauri::Wry>>().inner();
    if let Some(item) = menu.get(id) {
        if let Some(mi) = item.as_menuitem() {
            let _ = mi.set_text(text);
        }
    }
}

/// 启动 DeepSeek 余额自动刷新：启动后立即查询一次，之后每 5 分钟刷新，
/// 结果直接更新托盘菜单的 balance 项文字（无需用户点击）。
pub fn start_balance_refresh(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let text = match crate::commands::island_bridge::deepseek_get_balance().await {
                Ok(b) => format_balance(&b),
                Err(_) => balance_placeholder(&saved_lang()),
            };
            set_menu_item_text(&app, "balance", text);
            tokio::time::sleep(std::time::Duration::from_secs(300)).await; // 5 分钟
        }
    });
}

/// 更新托盘菜单文案（前端切换语言时调用）
#[tauri::command]
pub fn update_tray_menu(app: tauri::AppHandle, lang: String) -> Result<(), String> {
    let _ = std::fs::write(lang_file_path(), &lang);
    let (show_text, _island_text, check_update_text, quit_text) = tray_texts(&lang);
    let balance_text = balance_placeholder(&lang);
    for (id, text) in [
        ("show", show_text),
        ("island", String::new()), // island 文字单独处理，见下方状态逻辑
        ("check-update", check_update_text),
        ("balance", balance_text),
        ("quit", quit_text),
    ] {
        if let Some(item) = app.state::<tauri::menu::Menu<tauri::Wry>>().inner().get(id) {
            // island 是 CheckMenuItem（开关），文字显示"灵动岛：开"/"灵动岛：关"状态
            if id == "island" {
                let enabled = crate::commands::island_bridge::island_get_enabled();
                let state_label = island_label_by_state(&lang, enabled);
                if let Some(ci) = item.as_check_menuitem() {
                    let _ = ci.set_text(state_label);
                    let _ = ci.set_checked(enabled);
                }
                continue;
            }
            if let Some(mi) = item.as_menuitem() {
                let _ = mi.set_text(text);
            }
        }
    }
    Ok(())
}
