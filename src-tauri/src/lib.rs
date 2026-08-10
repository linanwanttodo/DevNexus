pub mod api_hub;
pub mod commands;
mod residue_scanner;
mod utils;

use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // GNOME Wayland 不支持透明窗口与 always-on-top（Tauri/WebKitGTK 已知限制，
    // 灵动岛窗口依赖两者）。在 GTK 初始化前强制走 XWayland（X11 后端）：
    // X11 协议原生支持 ARGB 透明 + _NET_WM_STATE_ABOVE 置顶，mutter 会正常合成。
    // Wayland 原生协议不提供窗口级 alpha 与置顶层，因此无条件覆盖
    // GDK_BACKEND=wayland（含用户显式设置）；纯 Wayland 环境若有 XWayland 同样适用。
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("GDK_BACKEND", "x11");
        eprintln!("[DevNexus] GDK_BACKEND forced to x11 (XWayland) for transparent + always-on-top support");
    }

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
            // 开发模式下硬刷新一次主窗口，确保显示最新前端代码。
            // 注意：不能调用 clear_all_browsing_data()——它会清空 localStorage，
            // 导致用户偏好（主题/灵动岛开关/DeepSeek Key 等）每次 dev 启动都丢失。
            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                    let _ = w.eval("location.reload(true)");
                });
            }

            // 启动 API Hub 后台服务
            let state = app.state::<api_hub::types::AppState>();
            let hub = Arc::new(state.inner().clone());
            tauri::async_runtime::spawn(async move {
                api_hub::start(hub).await;
            });

            // 启动灵动岛数据桥：系统通知监听（微信/QQ 等 → island-notify 事件）
            commands::island_bridge::init(app.handle().clone());

            // 静默启动：开启时主窗口不显示，后台常驻托盘 + 灵动岛
            if commands::autostart::get_silent_start() {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }

            let lang = commands::tray::saved_lang();
            let (show_label, island_label, check_update_label, quit_label) =
                commands::tray::tray_texts(&lang);
            let balance_label = commands::tray::balance_placeholder(&lang);
            let show = MenuItemBuilder::with_id("show", show_label).build(app)?;
            // 灵动岛：check 开关项（勾选=开，取消勾选=关），点击直接切换
            let island_checked = commands::island_bridge::island_get_enabled();
            let island = CheckMenuItemBuilder::with_id("island", island_label)
                .checked(island_checked)
                .build(app)?;
            let check_update =
                MenuItemBuilder::with_id("check-update", check_update_label).build(app)?;
            let balance = MenuItemBuilder::with_id("balance", balance_label).build(app)?;
            let quit = MenuItemBuilder::with_id("quit", quit_label).build(app)?;
            let menu = MenuBuilder::new(app)
                .items(&[&show, &island, &check_update, &balance, &quit])
                .build()?;
            // Linux(libappindicator/dbusmenu) 下菜单对象必须在 setup 返回后保持存活：
            // 否则 Rust 侧 Menu drop 会释放 D-Bus 菜单 registrar，导致托盘菜单项
            // 只剩空白框、文字不渲染（tauri#7648 / tray-icon#89）。
            // 通过 manage 存进 state 保持菜单引用存活。
            app.manage(menu.clone());

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
                    "island" => {
                        // 灵动岛开关：翻转 check 状态并同步显示/隐藏
                        let menu = app.state::<tauri::menu::Menu<tauri::Wry>>();
                        let next = if let Some(item) = menu.get("island") {
                            if let Some(ci) = item.as_check_menuitem() {
                                let cur = ci.is_checked().unwrap_or(false);
                                let _ = ci.set_checked(!cur);
                                !cur
                            } else {
                                !crate::commands::island_bridge::island_get_enabled()
                            }
                        } else {
                            !crate::commands::island_bridge::island_get_enabled()
                        };
                        let _ =
                            crate::commands::island_bridge::island_set_enabled(next, app.clone());
                    }
                    "check-update" => {
                        // 打开主窗口并导航到设置页触发检查更新
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                        use tauri::Emitter;
                        let _ = app.emit("tray-nav", "/settings");
                    }
                    "balance" => {
                        // 点击余额菜单项：查询 DeepSeek 余额并更新菜单文字
                        let app_handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let text = match crate::commands::island_bridge::deepseek_get_balance()
                                .await
                            {
                                Ok(b) => crate::commands::tray::format_balance(&b),
                                Err(e) => {
                                    let lang = crate::commands::tray::saved_lang();
                                    match lang.as_str() {
                                        "zh" => format!("DeepSeek 余额: 查询失败 ({e})"),
                                        "ru" => format!("Баланс DeepSeek: ошибка ({e})"),
                                        _ => format!("DeepSeek Balance: error ({e})"),
                                    }
                                }
                            };
                            crate::commands::tray::set_menu_item_text(&app_handle, "balance", text);
                        });
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(&app_handle)?;

            // 托盘 DeepSeek 余额自动刷新：启动后立即查询并周期性更新菜单文字
            commands::tray::start_balance_refresh(app_handle.clone());

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.label() == "island" {
                    // 灵动岛：关闭即隐藏（设置页可重新显示）
                    let _ = window.hide();
                } else {
                    // 主窗口：最小化而非隐藏。GNOME 桌面默认无托盘扩展，
                    // 隐藏后没有任何入口能找回窗口（用户会以为应用卡死）。
                    // 最小化保留任务栏入口，随时可恢复。
                    let _ = window.minimize();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::autostart::get_autostart,
            commands::autostart::set_autostart,
            commands::autostart::get_silent_start,
            commands::autostart::set_silent_start,
            commands::tray::update_tray_menu,
            commands::system::get_system_info,
            commands::system::get_resource_usage,
            commands::system::get_hardware_status,
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
            commands::password_manager::update_password,
            commands::password_manager::export_chrome_csv,
            commands::password_manager::import_chrome_csv,
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
            commands::island_bridge::island_media_status,
            commands::island_bridge::island_media_control,
            commands::island_bridge::island_set_sticky,
            commands::island_bridge::island_get_enabled,
            commands::island_bridge::island_set_enabled,
            commands::island_bridge::deepseek_get_balance,
            commands::island_bridge::deepseek_set_key,
            commands::island_bridge::deepseek_get_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DevNexus");
}
