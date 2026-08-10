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

/// 各语言的托盘菜单文案：show / island / check_update / quit
pub fn tray_texts(lang: &str) -> (String, String, String, String) {
    match lang {
        "zh" => (
            "显示 DevNexus".into(),
            "灵动岛设置".into(),
            "检查更新".into(),
            "退出".into(),
        ),
        "ru" => (
            "Показать DevNexus".into(),
            "Настройки острова".into(),
            "Проверить обновления".into(),
            "Выход".into(),
        ),
        _ => (
            "Show DevNexus".into(),
            "Dynamic Island Settings".into(),
            "Check for Updates".into(),
            "Quit".into(),
        ),
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

/// 更新托盘菜单文案（前端切换语言时调用）
#[tauri::command]
pub fn update_tray_menu(app: tauri::AppHandle, lang: String) -> Result<(), String> {
    let _ = std::fs::write(lang_file_path(), &lang);
    let (show_text, island_text, check_update_text, quit_text) = tray_texts(&lang);
    let balance_text = balance_placeholder(&lang);
    for (id, text) in [
        ("show", show_text),
        ("island", island_text),
        ("check-update", check_update_text),
        ("balance", balance_text),
        ("quit", quit_text),
    ] {
        if let Some(item) = app.state::<tauri::menu::Menu<tauri::Wry>>().inner().get(id) {
            if let Some(mi) = item.as_menuitem() {
                let _ = mi.set_text(text);
            }
        }
    }
    Ok(())
}
