//! Commands invoked from the UI. Errors are returned as user-facing strings.

use crate::{
    device::{self, Device, Tier, TierSetting},
    installer, launcher,
    library::{self, BiosStatus, Game},
    manifest::{self, manifest, BIOS_SUBDIRS},
    state::{AppState, Settings},
};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub settings: Settings,
    pub systems: Vec<SystemStatus>,
    pub running_game_id: Option<String>,
    pub device: Device,
    /// Preset level in effect (detected or chosen in settings).
    pub tier: Tier,
    /// The ASUS power-mode driver is present (ROG Ally).
    pub power_available: bool,
    /// Lumen starts with Windows.
    pub autostart: bool,
    /// Emulator downloads are Windows builds; on other OSes the UI runs in preview mode.
    pub platform_supported: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    pub id: String,
    pub emulator_installed: bool,
    pub core_installed: bool,
    /// Running on the built-in (HLE) BIOS because the user's BIOS is missing.
    pub using_fallback: bool,
    pub bios: Vec<BiosStatus>,
    pub game_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameView {
    #[serde(flatten)]
    pub game: Game,
    pub favorite: bool,
    pub last_played: Option<u64>,
    pub play_seconds: u64,
}

#[tauri::command]
pub fn get_overview(app: AppHandle, state: State<'_, AppState>) -> Overview {
    let library = state.library.lock().unwrap();
    let bios = state.bios_dir();

    let systems = manifest()
        .systems
        .iter()
        .map(|system| {
            let runner = library::effective(system, bios.as_deref());
            let emu = manifest::emulator(runner.emulator);
            let exe = emu.and_then(|e| installer::find_exe(e, &state.dirs.emulators.join(&e.id)));
            let core_installed = match (runner.core, &exe) {
                (None, _) => true,
                (Some(core), Some(exe)) => exe.parent().is_some_and(|d| {
                    d.join("cores")
                        .join(format!("{core}_libretro.dll"))
                        .exists()
                }),
                (Some(_), None) => false,
            };
            SystemStatus {
                id: system.id.clone(),
                emulator_installed: exe.is_some(),
                core_installed,
                using_fallback: runner.fallback,
                bios: library::bios_status(system, bios.as_deref()),
                game_count: library.iter().filter(|g| g.system_id == system.id).count(),
            }
        })
        .collect();

    Overview {
        settings: state.settings.lock().unwrap().clone(),
        systems,
        device: device::device().clone(),
        tier: state.tier(),
        power_available: crate::power::available(),
        autostart: autostart_enabled(&app),
        running_game_id: state
            .running
            .lock()
            .unwrap()
            .as_ref()
            .map(|g| g.game_id.clone()),
        platform_supported: cfg!(target_os = "windows"),
    }
}

#[tauri::command]
pub fn get_library(state: State<'_, AppState>) -> Vec<GameView> {
    library_view(&state)
}

fn library_view(state: &AppState) -> Vec<GameView> {
    let user = state.user.lock().unwrap();
    state
        .library
        .lock()
        .unwrap()
        .iter()
        .map(|game| {
            let data = user.get(&game.id).cloned().unwrap_or_default();
            GameView {
                game: game.clone(),
                favorite: data.favorite,
                last_played: data.last_played,
                play_seconds: data.play_seconds,
            }
        })
        .collect()
}

/// Sets the ROM root, creates one folder per system plus the BIOS folders, then scans.
#[tauri::command]
pub async fn set_rom_root(app: AppHandle, path: PathBuf) -> CmdResult<Vec<GameView>> {
    for system in &manifest().systems {
        let existing = system.folders.iter().any(|f| path.join(f).is_dir());
        if !existing {
            std::fs::create_dir_all(path.join(&system.folders[0])).map_err(err)?;
        }
    }
    for sub in BIOS_SUBDIRS {
        std::fs::create_dir_all(path.join("bios").join(sub)).map_err(err)?;
    }

    let state = app.state::<AppState>();
    state.settings.lock().unwrap().rom_root = Some(path);
    state.save_settings().map_err(err)?;
    scan_library(app.clone()).await
}

#[tauri::command]
pub async fn scan_library(app: AppHandle) -> CmdResult<Vec<GameView>> {
    let state = app.state::<AppState>();
    let Some(root) = state.rom_root() else {
        return Ok(Vec::new());
    };
    let games = tauri::async_runtime::spawn_blocking(move || library::scan(&root))
        .await
        .map_err(err)?;
    *state.library.lock().unwrap() = games;
    state.save_library().map_err(err)?;
    Ok(library_view(&state))
}

/// Installs whatever the given systems are missing: their emulators and, for
/// RetroArch systems, the libretro cores. Progress is streamed as events.
#[tauri::command]
pub async fn install_systems(app: AppHandle, system_ids: Vec<String>) -> CmdResult<()> {
    if !cfg!(target_os = "windows") {
        return Err("Emülatör kurulumu şimdilik sadece Windows'ta destekleniyor".into());
    }
    let state = app.state::<AppState>();
    let bios = state.bios_dir();

    // What actually runs each system right now (the HLE fallback without BIOS).
    let runners: Vec<_> = system_ids
        .iter()
        .filter_map(|id| manifest::system(id))
        .map(|s| library::effective(s, bios.as_deref()))
        .collect();
    let mut emulator_ids: Vec<&str> = runners.iter().map(|r| r.emulator).collect();
    emulator_ids.sort();
    emulator_ids.dedup();

    let mut failures = Vec::new();
    for emu_id in emulator_ids {
        let Some(emu) = manifest::emulator(emu_id) else {
            continue;
        };
        if !state.installing.lock().unwrap().insert(emu.id.clone()) {
            continue;
        }

        let install_dir = state.dirs.emulators.join(&emu.id);
        let exe = match installer::find_exe(emu, &install_dir) {
            Some(exe) => Ok(exe),
            None => {
                installer::install_emulator(
                    &app,
                    emu,
                    &state.dirs.emulators,
                    &state.dirs.downloads,
                    bios.as_deref(),
                    state.tier(),
                )
                .await
            }
        };

        match exe {
            Ok(exe) => {
                let mut cores: Vec<&str> = runners
                    .iter()
                    .filter(|r| r.emulator == emu.id)
                    .filter_map(|r| r.core)
                    .collect();
                cores.sort();
                cores.dedup();
                let cores_dir = exe.parent().map(|d| d.join("cores")).unwrap_or_default();
                for core in cores {
                    if cores_dir.join(format!("{core}_libretro.dll")).exists() {
                        continue;
                    }
                    if let Err(e) =
                        installer::install_core(&app, core, &cores_dir, &state.dirs.downloads).await
                    {
                        installer::emit(
                            &app,
                            &format!("core:{core}"),
                            "error",
                            0.0,
                            Some(e.to_string()),
                        );
                        failures.push(format!("{core}: {e}"));
                    }
                }
            }
            Err(e) => {
                installer::emit(&app, &emu.id, "error", 0.0, Some(e.to_string()));
                failures.push(format!("{}: {e}", emu.name));
            }
        }

        state.installing.lock().unwrap().remove(&emu.id);
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

#[tauri::command]
pub async fn launch_game(app: AppHandle, game_id: String, fresh: Option<bool>) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let game = state
        .library
        .lock()
        .unwrap()
        .iter()
        .find(|g| g.id == game_id)
        .cloned()
        .ok_or("Oyun bulunamadı")?;
    launcher::launch(&app, &game, fresh.unwrap_or(false))
        .await
        .map_err(err)
}

#[tauri::command]
pub fn stop_game(state: State<'_, AppState>) {
    launcher::stop(&state);
}

#[tauri::command]
pub fn toggle_favorite(state: State<'_, AppState>, game_id: String) -> CmdResult<bool> {
    let favorite = {
        let mut user = state.user.lock().unwrap();
        let entry = user.entry(game_id).or_default();
        entry.favorite = !entry.favorite;
        entry.favorite
    };
    state.save_user().map_err(err)?;
    Ok(favorite)
}

#[tauri::command]
pub fn set_sounds(state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    state.settings.lock().unwrap().sounds = enabled;
    state.save_settings().map_err(err)
}

#[tauri::command]
pub fn set_graphics(state: State<'_, AppState>, graphics: TierSetting) -> CmdResult<()> {
    state.settings.lock().unwrap().graphics = graphics;
    state.save_settings().map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorUpdate {
    pub id: String,
    pub update_available: bool,
}

/// Compares every installed emulator with its latest build. Sources that
/// can't be reached are reported as up to date rather than failing the list.
#[tauri::command]
pub async fn check_updates(app: AppHandle) -> Vec<EmulatorUpdate> {
    let state = app.state::<AppState>();
    let installed: Vec<_> = manifest()
        .emulators
        .iter()
        .filter(|e| installer::find_exe(e, &state.dirs.emulators.join(&e.id)).is_some())
        .collect();

    let checks = installed.iter().map(|emu| async {
        let current = installer::installed_version(&state.dirs.emulators.join(&emu.id));
        let latest = installer::resolve_release(&emu.source).await.ok();
        EmulatorUpdate {
            id: emu.id.clone(),
            // Installs from before version tracking have no record: offer the update.
            update_available: latest
                .is_some_and(|l| current.as_deref() != Some(l.version.as_str())),
        }
    });
    futures_util::future::join_all(checks).await
}

/// Re-downloads an emulator over its install (saves and settings survive) and,
/// for RetroArch, refreshes every installed core.
#[tauri::command]
pub async fn update_emulator(app: AppHandle, id: String) -> CmdResult<()> {
    let state = app.state::<AppState>();
    if state.running.lock().unwrap().is_some() {
        return Err("Oyun açıkken emülatör güncellenemez".into());
    }
    let emu = manifest::emulator(&id).ok_or("Bilinmeyen emülatör")?;
    if !state.installing.lock().unwrap().insert(emu.id.clone()) {
        return Ok(());
    }
    let result = async {
        let exe = installer::install_emulator(
            &app,
            emu,
            &state.dirs.emulators,
            &state.dirs.downloads,
            state.bios_dir().as_deref(),
            state.tier(),
        )
        .await?;
        if emu.id == "retroarch" {
            let cores_dir = exe.parent().map(|d| d.join("cores")).unwrap_or_default();
            let cores: Vec<String> = std::fs::read_dir(&cores_dir)
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    name.strip_suffix("_libretro.dll").map(String::from)
                })
                .collect();
            for core in cores {
                installer::install_core(&app, &core, &cores_dir, &state.dirs.downloads).await?;
            }
        }
        anyhow::Ok(())
    }
    .await;
    state.installing.lock().unwrap().remove(&emu.id);
    if let Err(e) = &result {
        installer::emit(&app, &emu.id, "error", 0.0, Some(e.to_string()));
    }
    result.map_err(err)
}

/// Quick menu → "back to game".
#[tauri::command]
pub fn resume_game(app: AppHandle) {
    launcher::resume_game(&app);
}

#[tauri::command]
pub fn is_resumable(state: State<'_, AppState>, game_id: String) -> bool {
    let game = state
        .library
        .lock()
        .unwrap()
        .iter()
        .find(|g| g.id == game_id)
        .cloned();
    game.is_some_and(|g| launcher::is_resumable(&state, &g))
}

#[tauri::command]
pub fn set_power(state: State<'_, AppState>, power: crate::power::PowerSetting) -> CmdResult<()> {
    state.settings.lock().unwrap().power = power;
    state.save_settings().map_err(err)
}

fn autostart_enabled(app: &AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// Console mode: Lumen starts (fullscreen) with Windows.
#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> CmdResult<()> {
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();
    if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    }
    .map_err(err)
}
