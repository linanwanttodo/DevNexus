mod commands;

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let task_scheduler = commands::scheduler::TaskScheduler::new();
    let password_manager = commands::password_manager::PasswordManager::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .manage(task_scheduler)
        .manage(password_manager)
        .setup(|app| {
            // 启动定时调度器后台循环
            let scheduler = app.state::<commands::scheduler::TaskScheduler>();
            scheduler.start_background();
            let show = MenuItemBuilder::with_id("show", "Show DevNexus").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

            let app_handle = app.handle().clone();
            let tray_icon = app
                .default_window_icon()
                .cloned()
                .or_else(|| Image::from_bytes(include_bytes!("../icons/32x32.png")).ok());

            let Some(tray_icon) = tray_icon else {
                eprintln!("[DevNexus] Warning: no tray icon available, skipping tray setup");
                return Ok(());
            };

            TrayIconBuilder::with_id("devnexus-tray")
                .tooltip("DevNexus")
                .icon(tray_icon)
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(move |tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(&app_handle)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::get_system_info,
            commands::system::get_resource_usage,
            commands::environment::list_environments,
            commands::environment::add_to_path,
            commands::environment::remove_from_path,
            commands::software::list_software,
            commands::software::list_package_managers,
            commands::software::install_software,
            commands::software::uninstall_software,
            commands::software::uninstall_software_deep,
            commands::software::fetch_software_versions,
            commands::software::install_software_from_url,
            commands::mirror::list_mirrors,
            commands::mirror::test_mirror_latency,
            commands::mirror::switch_mirror,
            commands::scheduler::add_task,
            commands::scheduler::list_tasks,
            commands::scheduler::delete_task,
            commands::scheduler::toggle_task,
            commands::scheduler::execute_task,
            commands::password_manager::add_password,
            commands::password_manager::list_passwords,
            commands::password_manager::get_password,
            commands::password_manager::delete_password,
            commands::password_manager::update_password,
            commands::password_manager::export_chrome_csv,
            commands::password_manager::import_chrome_csv,
            commands::password_manager::save_to_file,
            commands::password_manager::load_from_file,
            commands::cookie_extractor::get_supported_browsers,
            commands::cookie_extractor::extract_cookies,
            commands::cookie_extractor::export_as_netscape,
            commands::cookie_extractor::export_as_json,
            commands::port_manager::list_ports,
            commands::port_manager::kill_port,
            commands::updater::check_for_updates_github,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DevNexus");
}
