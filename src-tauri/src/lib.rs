pub mod commands;
mod residue_scanner;
mod utils;

use commands::window_factory::create_main_window;

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager,
};

fn init_tracing() {
    let filter = std::env::var("RUST_LOG")
        .map(|s| {
            tracing_subscriber::EnvFilter::try_new(s)
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"))
        })
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    let version_cache = commands::version_manager::VersionCache::new();

    // SSH AI 独立配置存档（自带 Provider，不依赖 API Hub）
    let ssh_ai_store = commands::ssh::ai::SshAiStore::load(&crate::utils::data_dir());

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_fs::init())
        .manage(version_cache)
        .manage(ssh_ai_store)
        .manage(commands::ssh::connections::SshStore::new())
        .manage(commands::ssh::session::SshSessionManager::new())
        .setup(move |app| {
            // 开发模式下硬刷新一次主窗口，确保显示最新前端代码。
            // 注意：不能调用 clear_all_browsing_data()——它会清空 localStorage，
            // 导致用户偏好（主题等）每次 dev 启动都丢失。
            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                    let _ = w.eval("location.reload(true)");
                });
            }

            // 静默启动：开启时主窗口不显示，后台常驻托盘。
            // 直接从 tauri.conf.json 自动创建的主窗口销毁（而非 hide），
            // 省掉 ~260MB 主窗口渲染进程；用户从托盘「显示 DevNexus」时再重建。
            if commands::autostart::get_silent_start() {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.set_skip_taskbar(true);
                    let _ = w.destroy();
                }
            }

            let lang = commands::tray::saved_lang();
            let (show_label, check_update_label, quit_label) =
                commands::tray::tray_texts(&lang);
            let show = MenuItemBuilder::with_id("show", show_label).build(app)?;
            let check_update =
                MenuItemBuilder::with_id("check-update", check_update_label).build(app)?;
            let quit = MenuItemBuilder::with_id("quit", quit_label).build(app)?;
            let menu = MenuBuilder::new(app)
                .items(&[&show, &check_update, &quit])
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
                tracing::warn!("[DevNexus] No tray icon available, skipping tray setup");
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
                        // 此处同步执行窗口 show/set_focus 等操作会令主循环死锁、
                        // grab 永不释放 → 整个桌面卡死。
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
                    "check-update" => {
                        // 避免在菜单 grab 上下文中同步操作窗口（同 "show" 死锁风险）。
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
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(&app_handle)?;

            // ── 启动期防"壁纸化"看门狗（Linux/XWayland）──
            // 已确诊根因：XWayland 的 _NET_CURRENT_DESKTOP 会在无人操作时抖动
            // （GNOME 工作区事件 / fcitx 输入法弹窗等外部发起），mutter 重算窗口
            // workspace 归属时，会把 frameless 主窗口误判为"在其他工作区"而置为
            // Iconic——窗口仍显示最后一帧（compositor 缓存），但不再接收输入与
            // 激活请求：用户看到的是"点不动、拖不动、贴死在桌面上"。
            // 防御：启动后 45s 内轮询 is_minimized()，一旦发现主窗口被偷偠
            // Iconic 化（此阶段用户不可能主动最小化），立即 unminimize+focus 自愈。
            // 45s 后自动退出：之后的正常最小化/恢复完全交给用户与托盘逻辑。
            #[cfg(target_os = "linux")]
            {
                let guard = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let start = std::time::Instant::now();
                    while start.elapsed() < std::time::Duration::from_secs(45) {
                        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
                        use tauri::Manager as _;
                        if let Some(w) = guard.get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) && w.is_minimized().unwrap_or(false)
                            {
                                tracing::warn!(
                                    "[DevNexus] main window wrongly iconified during \
                                     boot grace — restoring (anti-wallpaper watchdog)"
                                );
                                let _ = w.unminimize();
                                let _ = w.set_focus();
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                // 主窗口：关闭即销毁 WebView 渲染进程（而非 hide）。
                // 原因：hide() 只把窗口 unmap，背后的 WebKit 渲染进程（~260MB）
                // 不退出、JS 上下文与 DOM 全保留 → 内存一分不少。
                // destroy() 才真正回收渲染进程；下次由托盘「显示 DevNexus」按需重建。
                let _ = window.set_skip_taskbar(true);
                let _ = window.destroy();
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
            commands::tuning::scan_caches,
            commands::tuning::clean_paths,
            commands::tuning::get_disk_usage,
            commands::tuning::list_exclusions,
            commands::tuning::add_exclusion,
            commands::tuning::remove_exclusion,
            commands::tuning::optimize_disk,
            commands::tuning::clean_requires_sudo,
            commands::tuning::get_tuning_overview,
            commands::tuning::verify_sudo_password,
            commands::tuning::get_swap_info,
            commands::tuning::set_swap,
            commands::tuning::disable_swap,
            commands::tuning::get_dns_config,
            commands::tuning::set_dns,
            commands::tuning::get_timezone_info,
            commands::tuning::set_timezone,
            commands::tuning::get_firewall_status,
            commands::tuning::set_firewall,
            commands::tuning::get_system_limits,
            commands::tuning::scan_cleanup_targets,
            commands::tuning::clean_targets,
            commands::tuning::win_scan_cleanup,
            commands::tuning::win_clean_paths,
            commands::tuning::win_winsxs_cleanup,
            commands::tuning::win_get_hibernation,
            commands::tuning::win_set_hibernation,
            commands::tuning::win_list_startup,
            commands::tuning::win_set_startup,
            commands::tuning::win_storage_usage,
            commands::environment::list_environments,
            commands::environment::add_to_path,
            commands::environment::remove_from_path,
            commands::software::list_software,
            commands::software::list_package_managers,
            commands::software::install_software,
            commands::software::uninstall_software,
            commands::software::uninstall_installed_app,
            commands::software::uninstall_software_deep,
            commands::software::uninstall_software_deep_with_source,
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
            commands::local_files::local_read_text,
            commands::local_files::local_write_text,
            commands::local_files::local_mkdir_all,
            commands::local_files::local_list_dir,
            commands::local_files::local_read_file_chunk,
            commands::ssh::connections::ssh_list_connections,
            commands::ssh::connections::ssh_save_connection,
            commands::ssh::connections::ssh_delete_connection,
            commands::ssh::connections::ssh_touch_connection,
            commands::ssh::connections::ssh_import_open_ssh_config,
            commands::ssh::connections::ssh_export_openssh_config,
            commands::ssh::session::ssh_hostkey_accept,
            commands::ssh::session::ssh_hostkey_reject,
            commands::ssh::session::ssh_close,
            commands::ssh::session::ssh_test_connection,
            commands::ssh::session::ssh_forward_local,
            commands::ssh::session::ssh_close_forward,
            commands::ssh::session::ssh_list_forwards,
            commands::ssh::session::ssh_forward_agent,
            commands::ssh::session::ssh_socks_proxy,
            commands::ssh::session::ssh_close_socks,
            commands::ssh::session::ssh_list_socks,
            commands::ssh::terminal::ssh_terminal_open,
            commands::ssh::terminal::ssh_terminal_input,
            commands::ssh::terminal::ssh_terminal_resize,
            commands::ssh::terminal::ssh_terminal_close,
            commands::ssh::sftp::ssh_sftp_open,
            commands::ssh::sftp::ssh_sftp_close,
            commands::ssh::sftp::ssh_sftp_list_dir,
            commands::ssh::sftp::ssh_sftp_read_file,
            commands::ssh::sftp::ssh_sftp_write_file,
            commands::ssh::sftp::ssh_sftp_mkdir,
            commands::ssh::sftp::ssh_sftp_rename,
            commands::ssh::sftp::ssh_sftp_delete,
            commands::ssh::sftp::ssh_sftp_stat,
            commands::ssh::sftp::ssh_sftp_chmod,
            commands::ssh::sftp::ssh_sftp_copy_recursive,
            commands::ssh::sftp::ssh_sftp_rm_recursive,
            commands::ssh::sftp::ssh_sftp_search,
            commands::ssh::sftp::sftp_write_local_chunk,
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
            // SSH AI 助手（独立 Provider 配置，见 SSH 助手页）
            commands::ssh::ai::ssh_ai_list_providers,
            commands::ssh::ai::ssh_ai_add_provider,
            commands::ssh::ai::ssh_ai_update_provider,
            commands::ssh::ai::ssh_ai_delete_provider,
            commands::ssh::ai::ssh_ai_list_models,
            commands::ssh::ai::ssh_ai_list_terminals,
            commands::ssh::ai::ssh_ai_chat,
            commands::ssh::ai::ssh_ai_execute,
            commands::ssh::ai::ssh_ai_get_buffer,
        ])
        .run(tauri::generate_context!())
        .map_err(|e| {
            tracing::error!(
                error = %e,
                "DevNexus failed to start. This is a critical error that prevents the application from running."
            );
            e
        })
        .expect("DevNexus runtime failed to start - check logs for details");
}
