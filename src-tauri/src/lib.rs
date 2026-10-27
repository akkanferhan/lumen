mod commands;
mod config;
mod device;
mod formats;
mod input;
mod installer;
mod launcher;
mod library;
mod manifest;
mod metadata;
mod power;
mod presets;
mod state;
mod windows;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let state = state::AppState::load(app.handle())?;
            input::spawn(app.handle().clone(), state.running.clone());
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_overview,
            commands::get_library,
            commands::set_rom_root,
            commands::scan_library,
            commands::install_systems,
            commands::launch_game,
            commands::stop_game,
            commands::toggle_favorite,
            commands::set_sounds,
            commands::set_graphics,
            commands::check_updates,
            commands::update_emulator,
            commands::resume_game,
            commands::is_resumable,
            commands::set_power,
            commands::game_info,
            commands::check_app_update,
            commands::install_app_update,
            commands::set_autostart,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lumen");
}
