pub mod api_hub;
mod commands;
mod residue_scanner;
mod utils;

use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let password_manager = commands::password_manager::PasswordManager::new();
    let version_cache = commands::version_manager::VersionCache::new();

    // 初始化 API Hub
    let api_hub_state = api_hub::init(&crate::utils::data_dir());

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
    .manage(password_manager)
    .manage(version_cache)
    .manage(api_hub_state)
        .setup(move |app| {
            // 启动 API Hub 后台服务
            let state = app.state::<api_hub::types::AppState>();
            let hub = Arc::new(state.inner().clone());
            tauri::async_runtime::spawn(async move {
                api_hub::start(hub).await;
            });
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
            commands::system::get_app_version,
            commands::environment::list_environments,
            commands::environment::add_to_path,
            commands::environment::remove_from_path,
            commands::software::list_software,
            commands::software::list_package_managers,
            commands::software::install_software,
            commands::software::uninstall_software,
            commands::software::uninstall_software_deep,
            commands::software::scan_app_residues,
            commands::software::clean_specific_residues,
            commands::software::force_uninstall_software,
            commands::software::fetch_software_versions,
            commands::software::install_software_from_url,
            commands::software::list_installed_apps,
            commands::mirror::list_mirrors,
            commands::mirror::test_mirror_latency,
            commands::mirror::switch_mirror,
            commands::migration::export_migration,
            commands::migration::save_export_file,
            commands::migration::parse_migration_manifest,
            commands::migration::load_migration_file,
            commands::migration::import_migration,
            commands::password_manager::add_password,
            commands::password_manager::list_passwords,
            commands::password_manager::get_password,
            commands::password_manager::delete_password,
            commands::password_manager::is_locked,
            commands::password_manager::set_master_password,
            commands::password_manager::unlock,
            commands::password_manager::lock,
            commands::password_manager::has_master_password,
            commands::password_manager::update_password,
            commands::password_manager::export_chrome_csv,
            commands::password_manager::import_chrome_csv,
            commands::password_manager::save_to_file,
            commands::password_manager::load_from_file,
            commands::cookie_extractor::get_supported_browsers,
            commands::cookie_extractor::extract_cookies,
            commands::cookie_extractor::export_as_netscape,
            commands::cookie_extractor::export_as_json,
            commands::process_ports::list_processes,
            commands::process_ports::kill_process,
            commands::process_ports::kill_process_force,
            commands::process_ports::list_ports,
            commands::process_ports::kill_port,
            commands::container::check_docker,
            commands::container::list_containers,
            commands::container::container_action,
            commands::container::get_container_logs,
            commands::container::exec_in_container,
            commands::container::list_images,
            commands::container::pull_image,
            commands::container::remove_image,
            commands::container::build_image,
            commands::container::tag_image,
            commands::container::push_image,
            commands::container::list_volumes,
            commands::container::volume_action,
            commands::container::list_networks,
            commands::container::network_action,
            commands::container::compose_up,
            commands::container::compose_down,
            commands::container::compose_ps,
            commands::container::compose_logs,
            commands::updater::check_for_updates_github,
            commands::updater::get_download_url,
            commands::version_manager::list_versions,
            commands::version_manager::switch_version,
            api_hub::commands::api_hub_list_providers,
            api_hub::commands::api_hub_add_provider,
            api_hub::commands::api_hub_delete_provider,
            api_hub::commands::api_hub_update_provider,
            api_hub::commands::api_hub_get_logs,
            api_hub::commands::api_hub_get_usage_stats,
            api_hub::commands::api_hub_status,
            api_hub::commands::api_hub_fetch_models,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DevNexus");
}
