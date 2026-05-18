mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let terminal_state = commands::terminal::TerminalState::new();
    let task_scheduler = commands::scheduler::TaskScheduler::new();
    let password_manager = commands::password_manager::PasswordManager::new();
    
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(terminal_state)
        .manage(task_scheduler)
        .manage(password_manager)
        .invoke_handler(tauri::generate_handler![
            commands::system::get_system_info,
            commands::system::get_resource_usage,
            commands::environment::list_environments,
            commands::environment::add_to_path,
            commands::environment::remove_from_path,
            commands::software::list_software,
            commands::software::install_software,
            commands::software::uninstall_software,
            commands::mirror::list_mirrors,
            commands::mirror::test_mirror_latency,
            commands::mirror::switch_mirror,
            commands::terminal::spawn_terminal,
            commands::terminal::write_to_terminal,
            commands::terminal::close_terminal,
            commands::terminal::resize_terminal,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running DevNexus");
}
