mod achievements;
mod ally;
mod art;
mod autofix;
mod backup;
mod bios_check;
mod commands;
mod config;
mod crash;
mod device;
mod filters;
mod formats;
mod gog;
mod homebrew;
mod importer;
mod input;
mod installer;
mod itch;
mod launch_check;
mod launcher;
mod library;
mod manifest;
mod metadata;
mod overlay;
mod pc;
mod power;
mod presets;
mod retroachievements;
mod sleep;
mod state;
mod states;
mod stats;
mod store_library;
mod uninstall;
mod vita;
mod windows;
mod xbox;

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
            crash::install(&state.dirs.data);
            ally::init(&state.dirs.data);
            launcher::thaw_leftover(&state.dirs.data);
            // Manage state before starting anything that may call app.state().
            app.manage(state);
            let running = app.state::<state::AppState>().running.clone();
            input::spawn(app.handle().clone(), running);
            sleep::watch(app.handle().clone());
            overlay::create(app.handle());
            ally::force_gamepad_mode();
            Ok(())
        })
        // Armoury Crate may switch the Ally to desktop (keyboard/mouse) mode
        // while Lumen is in the background; take gamepad mode back on return.
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Focused(true)) {
                ally::force_gamepad_mode();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_overview,
            commands::get_library,
            commands::set_rom_root,
            commands::default_rom_root,
            commands::scan_library,
            commands::install_systems,
            commands::launch_game,
            commands::stop_game,
            commands::toggle_favorite,
            commands::set_sounds,
            commands::set_graphics,
            commands::set_filter,
            commands::create_collection,
            commands::delete_collection,
            commands::toggle_collection,
            commands::play_stats,
            commands::set_steam_key,
            commands::overlay_pressed,
            commands::set_quick_combo,
            commands::set_overlay_button,
            commands::set_theme,
            commands::check_updates,
            commands::update_emulator,
            commands::resume_game,
            commands::is_resumable,
            commands::set_power,
            commands::game_info,
            commands::take_crash_report,
            commands::uninstall_plan,
            commands::uninstall_game,
            commands::uninstall_emulator,
            commands::backup_now,
            commands::ra_login,
            commands::ra_logout,
            commands::ra_refresh,
            commands::set_ra_hardcore,
            commands::restore_backup,
            commands::set_backup,
            commands::homebrew_installed,
            commands::install_homebrew,
            commands::import_scan,
            commands::import_games,
            commands::auto_fix,
            commands::state_slots,
            commands::game_achievements,
            commands::save_state,
            commands::load_state,
            commands::check_app_update,
            commands::install_app_update,
            commands::set_autostart,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lumen");
}
