//! Commands invoked from the UI. Errors are returned as user-facing strings.

use crate::{
    device::{self, Device, Tier, TierSetting},
    installer, launcher,
    library::{self, BiosStatus, Game},
    manifest::{self, manifest, BIOS_SUBDIRS},
    state::{AppState, Settings},
};
use serde::Serialize;
use std::path::{Path, PathBuf};
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
    /// Resolved save backup folder (the default when none is chosen).
    pub backup_dir: PathBuf,
    /// Signed-in RetroAchievements user (token never leaves the backend).
    pub ra_user: Option<RaUser>,
    /// Emulator downloads are Windows builds; on other OSes the UI runs in preview mode.
    pub platform_supported: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RaUser {
    pub username: String,
    pub score: Option<u64>,
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

// Startup commands run off the main thread (async), so a bug that blocks
// them can't freeze the window and hide the error screen.
#[tauri::command]
pub async fn get_overview(app: AppHandle) -> Overview {
    let state = app.state::<AppState>();
    overview(&state, autostart_enabled(&app))
}

/// Never hold a lock across a call that takes the same lock again: std
/// mutexes aren't re-entrant, and this runs on the main thread, so a
/// self-deadlock freezes the whole app on a black screen.
fn overview(state: &AppState, autostart: bool) -> Overview {
    let settings = state.settings.lock().unwrap().clone();
    let backup_dir = settings.backup.target();
    let ra_user = state.ra.lock().unwrap().as_ref().map(|a| RaUser {
        username: a.username.clone(),
        score: a.score,
    });
    let tier = state.tier();
    let running_game_id = state
        .running
        .lock()
        .unwrap()
        .as_ref()
        .map(|g| g.game_id.clone());
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
        settings,
        systems,
        device: device::device().clone(),
        tier,
        power_available: crate::power::available(),
        autostart,
        backup_dir,
        ra_user,
        running_game_id,
        platform_supported: cfg!(target_os = "windows"),
    }
}

#[tauri::command]
pub async fn get_library(app: AppHandle) -> Vec<GameView> {
    library_view(&app.state::<AppState>())
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
    let root = state.rom_root();
    // Emulated games from the ROM folder plus installed Steam / Epic games.
    let games = tauri::async_runtime::spawn_blocking(move || {
        let mut games = root.map(|r| library::scan(&r)).unwrap_or_default();
        let mut pc = crate::pc::scan();
        pc.sort_by_key(|g| g.title.to_lowercase());
        games.extend(pc);
        games
    })
    .await
    .map_err(err)?;
    // Xbox games carry their Store id; swap it for the Store's poster URL.
    let mut games = games;
    let store_ids: Vec<String> = games
        .iter()
        .filter(|g| g.system_id == crate::xbox::XBOX_APP && !g.thumb_name.is_empty())
        .map(|g| g.thumb_name.clone())
        .collect();
    if !store_ids.is_empty() {
        let posters =
            crate::xbox::posters(&store_ids, &state.dirs.data.join("xbox-posters.json")).await;
        for game in games
            .iter_mut()
            .filter(|g| g.system_id == crate::xbox::XBOX_APP)
        {
            game.thumb_name = posters.get(&game.thumb_name).cloned().unwrap_or_default();
        }
    }
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

/// Year, developer, genre, players and description for a game, if known.
#[tauri::command]
pub async fn game_info(app: AppHandle, game_id: String) -> Option<crate::metadata::GameInfo> {
    let state = app.state::<AppState>();
    let game = state
        .library
        .lock()
        .unwrap()
        .iter()
        .find(|g| g.id == game_id)
        .cloned()?;
    let system = manifest::system(&game.system_id)?;
    let cached = state.metadata.lock().unwrap().get(&system.id).cloned();
    let info = match cached {
        Some(info) => info,
        None => {
            let info = std::sync::Arc::new(
                crate::metadata::system_info(system, &state.dirs.data.join("metadata")).await,
            );
            state
                .metadata
                .lock()
                .unwrap()
                .insert(system.id.clone(), info.clone());
            info
        }
    };
    crate::metadata::lookup(&info, &game).cloned()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdate {
    pub version: String,
    pub notes: Option<String>,
}

/// A newer signed Lumen release, if any (GitHub releases' latest.json).
#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> CmdResult<Option<AppUpdate>> {
    use tauri_plugin_updater::UpdaterExt;
    let update = app.updater().map_err(err)?.check().await.map_err(err)?;
    Ok(update.map(|u| AppUpdate {
        version: u.version,
        notes: u.body,
    }))
}

pub const APP_UPDATE_EVENT: &str = "app-update-progress";

/// Downloads, verifies the signature, installs (passive installer) and restarts.
#[tauri::command]
pub async fn install_app_update(app: AppHandle) -> CmdResult<()> {
    use tauri::Emitter;
    use tauri_plugin_updater::UpdaterExt;
    if app.state::<AppState>().running.lock().unwrap().is_some() {
        return Err("Oyun açıkken güncelleme yapılamaz".into());
    }
    let Some(update) = app.updater().map_err(err)?.check().await.map_err(err)? else {
        return Ok(());
    };
    let mut received = 0usize;
    let progress_app = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                received += chunk;
                let progress = total.map_or(0.0, |t| received as f32 / t as f32);
                let _ = progress_app.emit(APP_UPDATE_EVENT, progress);
            },
            || {},
        )
        .await
        .map_err(err)?;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    fn temp_state(name: &str) -> AppState {
        let dir = std::env::temp_dir().join(format!("lumen-state-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        AppState::at(dir).unwrap()
    }

    /// Regression: get_overview re-locked `settings` (via `tier()`) while
    /// holding it, deadlocking the main thread on start (black screen).
    #[test]
    fn overview_and_library_return_without_deadlock() {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let state = temp_state("overview");
            let overview = overview(&state, false);
            let library = library_view(&state);
            tx.send((overview.systems.len(), library.len())).unwrap();
        });
        let (systems, games) = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("overview deadlocked");
        assert_eq!(systems, manifest().systems.len());
        assert_eq!(games, 0);
    }
}

/// Slugs of catalog homebrew already in the ROM folders.
#[tauri::command]
pub fn homebrew_installed(state: State<'_, AppState>) -> Vec<String> {
    let Some(root) = state.rom_root() else {
        return Vec::new();
    };
    crate::homebrew::catalog()
        .iter()
        .filter(|g| crate::homebrew::target(g, &root).is_some_and(|p| p.exists()))
        .map(|g| g.slug.clone())
        .collect()
}

/// Downloads a catalog game into its system folder and rescans the library.
#[tauri::command]
pub async fn install_homebrew(app: AppHandle, slug: String) -> CmdResult<Vec<GameView>> {
    let state = app.state::<AppState>();
    let game = crate::homebrew::find(&slug).ok_or("Katalogda böyle bir oyun yok")?;
    let root = state.rom_root().ok_or("Önce bir oyun klasörü seç")?;
    let target = crate::homebrew::target(game, &root).ok_or("Bilinmeyen sistem")?;
    let id = format!("homebrew:{slug}");
    if let Err(e) =
        installer::download_to(&app, &id, &game.url, &target, &state.dirs.downloads).await
    {
        installer::emit(&app, &id, "error", 0.0, Some(e.to_string()));
        return Err(err(e));
    }
    scan_library(app.clone()).await
}

/// Backs up every installed emulator's saves and Lumen's play data now.
#[tauri::command]
pub async fn backup_now(app: AppHandle) -> CmdResult<crate::backup::Report> {
    let state = app.state::<AppState>();
    let target = state.settings.lock().unwrap().backup.target();
    let emulators = state.dirs.emulators.clone();
    let data = state.dirs.data.clone();
    let report =
        tauri::async_runtime::spawn_blocking(move || backup_all(&emulators, &data, &target))
            .await
            .map_err(err)?
            .map_err(err)?;
    mark_backed_up(&state);
    Ok(report)
}

/// Copies saves back from the backup wherever the backup copy is newer.
#[tauri::command]
pub async fn restore_backup(app: AppHandle) -> CmdResult<crate::backup::Report> {
    let state = app.state::<AppState>();
    if state.running.lock().unwrap().is_some() {
        return Err("Oyun açıkken geri yükleme yapılamaz".into());
    }
    let target = state.settings.lock().unwrap().backup.target();
    if !target.is_dir() {
        return Err("Yedek klasörü bulunamadı".into());
    }
    let emulators = state.dirs.emulators.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut report = crate::backup::Report::default();
        for emu in &manifest().emulators {
            report += crate::backup::restore_emulator(&emu.id, &emulators.join(&emu.id), &target)?;
        }
        std::io::Result::Ok(report)
    })
    .await
    .map_err(err)?
    .map_err(err)
}

#[tauri::command]
pub fn set_backup(
    state: State<'_, AppState>,
    enabled: bool,
    dir: Option<PathBuf>,
) -> CmdResult<()> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.backup.enabled = enabled;
        if dir.is_some() {
            settings.backup.dir = dir;
        }
    }
    state.save_settings().map_err(err)
}

fn backup_all(
    emulators: &Path,
    data: &Path,
    target: &Path,
) -> std::io::Result<crate::backup::Report> {
    let mut report = crate::backup::backup_lumen(data, target)?;
    for emu in &manifest().emulators {
        report += crate::backup::backup_emulator(&emu.id, &emulators.join(&emu.id), target)?;
    }
    Ok(report)
}

pub(crate) fn mark_backed_up(state: &AppState) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs());
    state.settings.lock().unwrap().backup.last_backup = now;
    let _ = state.save_settings();
}

/// Logs in to RetroAchievements; only the returned token is kept.
#[tauri::command]
pub async fn ra_login(app: AppHandle, username: String, password: String) -> CmdResult<RaUser> {
    use crate::retroachievements::{login, Secret};
    let account = login(username.trim(), Secret::Password(&password))
        .await
        .map_err(err)?;
    let user = RaUser {
        username: account.username.clone(),
        score: account.score,
    };
    let state = app.state::<AppState>();
    *state.ra.lock().unwrap() = Some(account);
    state.save_ra().map_err(err)?;
    Ok(user)
}

#[tauri::command]
pub fn ra_logout(state: State<'_, AppState>) -> CmdResult<()> {
    *state.ra.lock().unwrap() = None;
    state.save_ra().map_err(err)
}

#[tauri::command]
pub fn set_ra_hardcore(state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    state.settings.lock().unwrap().ra_hardcore = enabled;
    state.save_settings().map_err(err)
}

/// Re-checks the stored token (updates the score). An expired token signs
/// the user out so emulators don't keep failing silently.
#[tauri::command]
pub async fn ra_refresh(app: AppHandle) -> CmdResult<Option<RaUser>> {
    use crate::retroachievements::{login, Secret};
    let state = app.state::<AppState>();
    let Some(current) = state.ra.lock().unwrap().clone() else {
        return Ok(None);
    };
    match login(&current.username, Secret::Token(&current.token)).await {
        Ok(fresh) => {
            let user = RaUser {
                username: fresh.username.clone(),
                score: fresh.score,
            };
            *state.ra.lock().unwrap() = Some(fresh);
            state.save_ra().map_err(err)?;
            Ok(Some(user))
        }
        // Network trouble: keep the login, report it unchanged.
        Err(e) if e.to_string().contains("ulaşılamadı") => Ok(Some(RaUser {
            username: current.username,
            score: current.score,
        })),
        Err(_) => {
            *state.ra.lock().unwrap() = None;
            state.save_ra().map_err(err)?;
            Err("RetroAchievements oturumunun süresi doldu, tekrar giriş yap".into())
        }
    }
}

#[tauri::command]
pub fn set_theme(state: State<'_, AppState>, theme: String) -> CmdResult<()> {
    state.settings.lock().unwrap().theme = theme;
    state.save_settings().map_err(err)
}

fn find_game(state: &AppState, game_id: &str) -> CmdResult<Game> {
    state
        .library
        .lock()
        .unwrap()
        .iter()
        .find(|g| g.id == game_id)
        .cloned()
        .ok_or_else(|| "Oyun bulunamadı".into())
}

/// What removing a game will do (and how much space it frees).
#[tauri::command]
pub async fn uninstall_plan(app: AppHandle, game_id: String) -> CmdResult<crate::uninstall::Plan> {
    let game = find_game(&app.state::<AppState>(), &game_id)?;
    tauri::async_runtime::spawn_blocking(move || crate::uninstall::plan(&game))
        .await
        .map_err(err)
}

/// Removes a game. Emulated games go to the Recycle Bin (saves stay);
/// store games are handed to their store. Returns the updated library.
#[tauri::command]
pub async fn uninstall_game(app: AppHandle, game_id: String) -> CmdResult<Vec<GameView>> {
    let state = app.state::<AppState>();
    if state
        .running
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|g| g.game_id == game_id)
    {
        return Err("Oyun açıkken kaldırılamaz".into());
    }
    let game = find_game(&state, &game_id)?;
    // Vita games also live inside Vita3K once installed; remove them there too.
    if game.system_id == "psvita" {
        if let Some(emu) = manifest::emulator("vita3k") {
            let exe = installer::find_exe(emu, &state.dirs.emulators.join(&emu.id));
            if let (Some(exe), Some(id)) = (exe, crate::vita::title_id(&game.path)) {
                let _ = crate::vita::delete_title(&exe, &id);
            }
        }
    }
    let gone = tauri::async_runtime::spawn_blocking(move || crate::uninstall::uninstall(&game))
        .await
        .map_err(err)?
        .map_err(err)?;
    if gone {
        state.library.lock().unwrap().retain(|g| g.id != game_id);
        state.save_library().map_err(err)?;
    }
    Ok(library_view(&state))
}

/// Removes an installed emulator. Its folder holds portable saves, so they are
/// backed up first (when backups are on) and the folder goes to the Recycle Bin.
#[tauri::command]
pub async fn uninstall_emulator(app: AppHandle, id: String) -> CmdResult<()> {
    let state = app.state::<AppState>();
    if state.running.lock().unwrap().is_some() {
        return Err("Oyun açıkken emülatör kaldırılamaz".into());
    }
    let emu = manifest::emulator(&id).ok_or("Bilinmeyen emülatör")?;
    let dir = state.dirs.emulators.join(&emu.id);
    if !dir.exists() {
        return Ok(());
    }
    let backup = state.settings.lock().unwrap().backup.clone();
    let emu_id = emu.id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if backup.enabled {
            crate::backup::backup_emulator(&emu_id, &dir, &backup.target())?;
        }
        trash::delete(&dir).map_err(|e| anyhow::anyhow!("Geri Dönüşüm Kutusu'na taşınamadı: {e}"))
    })
    .await
    .map_err(err)?
    .map_err(err)
}
