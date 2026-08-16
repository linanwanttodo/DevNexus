pub mod api_hub;
pub mod commands;
mod residue_scanner;
mod utils;

use commands::window_factory::create_main_window;

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
        // 仅在用户未显式指定后端时强制 x11：尊重显式设置，
        // 避免覆盖用户为兼容性/调试目的配置的 GDK_BACKEND。
        if std::env::var_os("GDK_BACKEND").is_none() {
            std::env::set_var("GDK_BACKEND", "x11");
        }
        eprintln!(
            "[DevNexus] GDK_BACKEND = {}",
            std::env::var("GDK_BACKEND").unwrap_or_default()
        );
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
        .manage(commands::ssh::connections::SshStore::new())
        .manage(commands::ssh::session::SshSessionManager::new())
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

            // 静默启动：开启时主窗口不显示，后台常驻托盘 + 灵动岛。
            // 直接从 tauri.conf.json 自动创建的主窗口销毁（而非 hide），
            // 省掉 ~260MB 主窗口渲染进程；用户从托盘「显示 DevNexus」时再重建。
            if commands::autostart::get_silent_start() {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.set_skip_taskbar(true);
                    let _ = w.destroy();
                }
            }

            let lang = commands::tray::saved_lang();
            let (show_label, _island_label, check_update_label, quit_label) =
                commands::tray::tray_texts(&lang);
            let balance_label = commands::tray::balance_placeholder(&lang);
            let show = MenuItemBuilder::with_id("show", show_label).build(app)?;
            // 灵动岛：check 开关项，文字显示当前状态（"灵动岛：开"/"灵动岛：关"），
            // 点击直接切换并同步更新文字
            let island_checked = commands::island_bridge::island_get_enabled();
            let island_state_label = commands::tray::island_label_by_state(&lang, island_checked);
            let island = CheckMenuItemBuilder::with_id("island", island_state_label)
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
                        // 关键：菜单事件在主线程且持有 GTK 菜单指针 grab 的上下文中触发，
                        // 此处同步执行窗口 show/set_focus 等 X11 操作会令主循环死锁、
                        // grab 永不释放 → 整个桌面卡死（参见下方 "island" 分支说明）。
                        // 因此全部窗口操作抛到异步线程，先让菜单回调返回并释放 grab。
                        let app_clone = app.clone();
                        tauri::async_runtime::spawn(async move {
                            // 主窗口可能因「关闭转后台」被 destroy()；不存在时先重建。
                            if app_clone.get_webview_window("main").is_none() {
                                create_main_window(&app_clone);
                            }
                            if let Some(w) = app_clone.get_webview_window("main") {
                                let _ = w.set_skip_taskbar(false);
                                let _ = w.unminimize();
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        });
                    }
                    "island" => {
                        // 灵动岛开关：以 Rust 侧持久化状态文件为准计算 next，
                        // 不再依赖 dbusmenu 的 is_checked()——在某些环境下
                        // is_checked() 不可靠、恒返回 false，会导致 next 永远为
                        // true（永远"启用"），托盘点击毫无变化（"开关没用"）。
                        let cur = crate::commands::island_bridge::island_get_enabled();
                        let next = !cur;
                        // 同步更新菜单 check 项与文字，确保视觉一致
                        {
                            let menu = app.state::<tauri::menu::Menu<tauri::Wry>>();
                            if let Some(item) = menu.get("island") {
                                if let Some(ci) = item.as_check_menuitem() {
                                    let _ = ci.set_checked(next);
                                }
                            }
                        }

                        // 关键修复：菜单事件回调运行在 GTK 主线程，且当时正持有
                        // 托盘菜单的 pointer grab（指针捕获）。若在此处同步执行
                        // island_set_enabled() 内的窗口 X11 操作（show/hide/
                        // set_always_on_top/set_visible_on_all_workspaces），这些
                        // 操作需要主事件循环继续推进才能完成 X11 往返，但当前回调
                        // 本就阻塞着主循环 → 死锁。grab 永不释放，X 服务器把所有
                        // 指针输入只路由给本进程 → 整个桌面冻结、仅光标可动。
                        // 因此：先把 next 状态算好，让菜单回调立即返回释放 grab，
                        // 再把真正的窗口变更抛到异步线程执行。
                        let app_clone = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = crate::commands::island_bridge::island_set_enabled(
                                next,
                                app_clone.clone(),
                            );
                            // 同步更新菜单文字：开 → "灵动岛：开" / 关 → "灵动岛：关"
                            let lang = crate::commands::tray::saved_lang();
                            crate::commands::tray::update_island_menu_text(&app_clone, &lang, next);
                        });
                    }
                    "check-update" => {
                        // 同样避免在菜单 grab 上下文中同步操作窗口（同 "island"/"show" 死锁风险）。
                        let app_clone = app.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Some(w) = app_clone.get_webview_window("main") {
                                let _ = w.set_skip_taskbar(false);
                                let _ = w.unminimize();
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                            use tauri::Emitter;
                            let _ = app_clone.emit("tray-nav", "/settings");
                        });
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
                    // 主窗口：关闭即销毁 WebView 渲染进程（而非 hide）。
                    // 原因：hide() 只把窗口 unmap，背后的 WebKit 渲染进程（~260MB）
                    // 不退出、JS 上下文与 DOM 全保留 → 内存一分不少。
                    // destroy() 才真正回收渲染进程；下次由托盘「显示 DevNexus」
                    // 或点击灵动岛时按需重建（create_main_window）。
                    // 这样软件后台常驻时，主窗口的 260MB 渲染进程被释放，
                    // 常驻内存从 ~760MB 降到 ~300MB（仅 Rust 宿主 + 网络进程 + 岛窗口）。
                    // 恢复入口：托盘「显示 DevNexus」(rebuild + show)，或点击灵动岛。
                    let _ = window.set_skip_taskbar(true);
                    let _ = window.destroy();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::autostart::get_autostart,
            commands::autostart::set_autostart,
            commands::autostart::get_silent_start,
            commands::autostart::set_silent_start,
            commands::tray::update_tray_menu,
            commands::window_factory::show_main_window,
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
            commands::ssh::connections::ssh_list_connections,
            commands::ssh::connections::ssh_save_connection,
            commands::ssh::connections::ssh_delete_connection,
            commands::ssh::session::ssh_hostkey_accept,
            commands::ssh::session::ssh_hostkey_reject,
            commands::ssh::session::ssh_close,
            commands::ssh::session::ssh_test_connection,
            commands::ssh::terminal::ssh_terminal_open,
            commands::ssh::terminal::ssh_terminal_input,
            commands::ssh::terminal::ssh_terminal_resize,
            commands::ssh::terminal::ssh_terminal_close,
            commands::ssh::sftp::ssh_sftp_open,
            commands::ssh::sftp::ssh_sftp_list_dir,
            commands::ssh::sftp::ssh_sftp_read_file,
            commands::ssh::sftp::ssh_sftp_write_file,
            commands::ssh::sftp::ssh_sftp_mkdir,
            commands::ssh::sftp::ssh_sftp_rename,
            commands::ssh::sftp::ssh_sftp_delete,
            commands::ssh::sftp::ssh_sftp_stat,
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
            commands::island_bridge::island_get_hud,
            commands::island_bridge::island_get_enabled,
            commands::island_bridge::island_set_enabled,
            commands::island_bridge::deepseek_get_balance,
            commands::island_bridge::deepseek_set_key,
            commands::island_bridge::deepseek_get_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DevNexus");
}
