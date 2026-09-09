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

/// 各语言的托盘菜单文案：show / check_update / quit
pub fn tray_texts(lang: &str) -> (String, String, String) {
    match lang {
        "zh" => ("显示 DevNexus".into(), "检查更新".into(), "退出".into()),
        "ru" => (
            "Показать DevNexus".into(),
            "Проверить обновления".into(),
            "Выход".into(),
        ),
        _ => (
            "Show DevNexus".into(),
            "Check for Updates".into(),
            "Quit".into(),
        ),
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
    let (show_text, check_update_text, quit_text) = tray_texts(&lang);
    for (id, text) in [
        ("show", show_text),
        ("check-update", check_update_text),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_texts_zh() {
        let (show, check, quit) = tray_texts("zh");
        assert_eq!(show, "显示 DevNexus");
        assert_eq!(check, "检查更新");
        assert_eq!(quit, "退出");
    }

    #[test]
    fn test_tray_texts_ru() {
        let (show, check, quit) = tray_texts("ru");
        assert_eq!(show, "Показать DevNexus");
        assert_eq!(check, "Проверить обновления");
        assert_eq!(quit, "Выход");
    }

    #[test]
    fn test_tray_texts_default_en() {
        let (show, check, quit) = tray_texts("xx");
        assert_eq!(show, "Show DevNexus");
        assert_eq!(check, "Check for Updates");
        assert_eq!(quit, "Quit");
    }
}
