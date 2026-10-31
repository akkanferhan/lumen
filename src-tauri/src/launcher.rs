//! Starts a game in its emulator and brings Lumen back when it ends.

use crate::{
    config, installer,
    library::Game,
    manifest,
    state::{AppState, RunningGame},
};
use anyhow::{anyhow, bail};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;

pub const EXITED_EVENT: &str = "game-exited";
/// The game's window is up (or Lumen stopped waiting for it): the loading
/// screen can go.
pub const SHOWN_EVENT: &str = "game-shown";
/// Kids mode: today's play time ran out; the game is being saved and closed.
pub const KIDS_TIME_UP_EVENT: &str = "kids-time-up";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exited {
    pub game_id: String,
    pub session_seconds: u64,
    /// Why the emulator closed right after starting, if it did.
    pub error: Option<String>,
}

/// `fresh` starts from the beginning instead of the last resume state.
/// `time_left`: kids mode's play time left today (seconds); the game is
/// saved and closed when it runs out.
pub async fn launch(
    app: &AppHandle,
    game: &Game,
    fresh: bool,
    time_left: Option<u64>,
) -> anyhow::Result<()> {
    let state = app.state::<AppState>();
    if state.running.lock().unwrap().is_some() {
        bail!(crate::tr!(
            "Zaten bir oyun çalışıyor",
            "A game is already running"
        ));
    }
    // Games need the Ally's controller as a gamepad, not keyboard and mouse.
    crate::ally::force_gamepad_mode();

    // Store games start through their launcher; Lumen can't follow their process.
    if crate::pc::is_pc(&game.system_id) {
        crate::pc::launch(game)?;
        // Opening a store's install page isn't playing.
        if game.not_installed {
            return Ok(());
        }
        let mut user = state.user.lock().unwrap();
        user.entry(game.id.clone()).or_default().last_played = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs());
        drop(user);
        let _ = state.save_user();
        return Ok(());
    }

    let system = manifest::system(&game.system_id)
        .ok_or_else(|| anyhow!(crate::tr!("Bilinmeyen sistem", "Unknown system")))?;
    let bios = state.bios_dir();
    let runner = crate::library::effective(system, bios.as_deref());
    if !runner.fallback {
        if let Some(missing) = crate::library::bios_status(system, bios.as_deref())
            .into_iter()
            .find(|b| !b.found && !b.optional)
        {
            bail!(crate::tr!(
                "Eksik dosya: {}",
                "Missing file: {}",
                missing.label
            ));
        }
    }
    let emu = manifest::emulator(runner.emulator)
        .ok_or_else(|| anyhow!(crate::tr!("Bilinmeyen emülatör", "Unknown emulator")))?;
    let exe = installer::find_exe(emu, &state.dirs.emulators.join(&emu.id)).ok_or_else(|| {
        anyhow!(crate::tr!(
            "{} kurulu değil",
            "{} isn't installed",
            emu.name
        ))
    })?;
    let exe_dir = exe.parent().ok_or_else(|| {
        anyhow!(crate::tr!(
            "Geçersiz emülatör yolu",
            "Invalid emulator path"
        ))
    })?;

    config::configure(emu, exe_dir, bios.as_deref(), state.tier())?;
    pull_synced_saves(&state, &emu.id).await;
    {
        let account = state.ra.lock().unwrap().clone();
        let hardcore = state.settings.lock().unwrap().ra_hardcore;
        crate::retroachievements::apply(
            &emu.id,
            exe_dir,
            crate::retroachievements::Cheevos {
                account: account.as_ref(),
                hardcore,
            },
        )?;
    }
    match emu.id.as_str() {
        "rpcs3" => ensure_ps3_firmware(app, &exe, bios.as_deref(), &state.dirs.downloads).await?,
        "xemu" => installer::ensure_xbox_hdd(app, exe_dir, &state.dirs.downloads).await?,
        "vita3k" => crate::vita::ensure_firmware(app, &exe, &state.dirs.downloads).await?,
        _ => {}
    }

    let core = match runner.core {
        Some(core) => {
            let path = exe_dir.join("cores").join(format!("{core}_libretro.dll"));
            if !path.exists() {
                bail!(crate::tr!(
                    "{} çekirdeği kurulu değil",
                    "The {} core isn't installed",
                    system.short
                ));
            }
            path.to_string_lossy().into_owned()
        }
        None => String::new(),
    };
    let rom_path = match &system.launch_file {
        Some(file) if game.path.is_dir() => game.path.join(file),
        _ => game.path.clone(),
    };
    let rom = rom_path.to_string_lossy();
    let resume = resume_store(&emu.id, exe_dir);
    let resume_args = match (&resume, fresh) {
        (Some(_), false) => resume_file(&state, &game.id)
            .map(|path| {
                vec![
                    "-statefile".to_string(),
                    path.to_string_lossy().into_owned(),
                ]
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    // Vita3K takes an installed title ID instead of a path once installed.
    let rom_args = if emu.id == "vita3k" {
        crate::vita::rom_args(exe_dir, &rom_path)
    } else {
        vec![rom.to_string()]
    };
    // The display filter: a config layer plus the shader preset.
    let filter_args = if emu.id == "retroarch" {
        let setting = state.settings.lock().unwrap().filter;
        let filter = setting.resolve(&system.id);
        // No shaders (offline): play unfiltered rather than not at all.
        let preset = crate::filters::ensure_preset(app, filter, exe_dir, &state.dirs.downloads)
            .await
            .unwrap_or_default();
        let filter = if preset.is_none() && filter.needs_shader() {
            crate::filters::Filter::Original
        } else {
            filter
        };
        crate::filters::apply(exe_dir, filter, preset.as_deref())?
    } else {
        Vec::new()
    };
    // RetroArch only logs its errors with -v; the log explains early exits.
    let log = (emu.id == "retroarch").then(|| crate::launch_check::retroarch_log(exe_dir));
    let log_args = match &log {
        Some(path) => {
            let _ = std::fs::create_dir_all(path.parent().unwrap_or(exe_dir));
            let _ = std::fs::remove_file(path);
            vec![
                "-v".to_string(),
                "--log-file".to_string(),
                path.to_string_lossy().into_owned(),
            ]
        }
        None => Vec::new(),
    };
    let args: Vec<String> =
        crate::presets::launch_args(emu, exe_dir, state.tier(), fresh, &system.id)
            .into_iter()
            .chain(log_args)
            .chain(filter_args)
            .chain(resume_args)
            .chain(emu.args.iter().flat_map(|a| {
                if a == "{rom}" {
                    rom_args.clone()
                } else {
                    vec![a.replace("{rom}", &rom).replace("{core}", &core)]
                }
            }))
            .collect();

    let mut child = tokio::process::Command::new(&exe)
        .args(&args)
        .envs(crate::presets::launch_env(emu).iter().copied())
        .current_dir(exe_dir)
        .spawn()
        .map_err(|e| {
            let launch = crate::launch_check::Launch {
                emulator_id: &emu.id,
                emulator: &emu.name,
                core: runner.core,
            };
            let diagnosis = crate::launch_check::spawn_error(&launch, &e);
            *state.launch_fix.lock().unwrap() = diagnosis.fix.map(|f| (game.id.clone(), f));
            anyhow!(diagnosis.message)
        })?;
    if let Some(pid) = child.id() {
        bring_up_when_shown(app.clone(), pid);
    }
    let (stop_tx, stop_rx) = oneshot::channel();
    *state.running.lock().unwrap() = Some(RunningGame {
        game_id: game.id.clone(),
        stop: Some(stop_tx),
        pid: child.id(),
        emulator: emu.id.clone(),
        suspended: false,
        controls: crate::controls::for_game(&emu.id, &system.id, &rom_path),
        exe_dir: exe_dir.to_path_buf(),
    });
    let session_start = SystemTime::now();

    // Match the ASUS power mode to the game (its own choice, else its
    // system's); Performance (the default) is restored when the game ends.
    let game_power = state
        .user
        .lock()
        .unwrap()
        .get(&game.id)
        .and_then(|u| u.power);
    let power = state
        .settings
        .lock()
        .unwrap()
        .power
        .resolve_for_game(game_power, system.power);
    let power_changed = power.is_some_and(crate::power::set);

    let app = app.clone();
    let game_id = game.id.clone();
    let system_id = game.system_id.clone();
    let emu_id = emu.id.clone();
    let emu_dir = exe_dir.to_path_buf();
    let emu_name = emu.name.clone();
    let core = runner.core.map(String::from);
    tauri::async_runtime::spawn(async move {
        let started = Instant::now();
        // None when Lumen closed it (quick menu, home combo, kids' time up).
        let time_up = async {
            match time_left {
                Some(secs) => tokio::time::sleep(Duration::from_secs(secs)).await,
                None => std::future::pending().await,
            }
        };
        let exit = tokio::select! {
            status = child.wait() => Some(status.ok().and_then(|s| s.code())),
            _ = stop_rx => {
                close_gracefully(&mut child).await;
                None
            }
            _ = time_up => {
                let _ = app.emit(KIDS_TIME_UP_EVENT, ());
                close_gracefully(&mut child).await;
                None
            }
        };
        let ran = started.elapsed();
        let launch = crate::launch_check::Launch {
            emulator_id: &emu_id,
            emulator: &emu_name,
            core: core.as_deref(),
        };
        let diagnosis = exit
            .and_then(|code| crate::launch_check::early_exit(&launch, code, ran, log.as_deref()));
        let session_seconds = ran.as_secs();

        let state = app.state::<AppState>();
        match &diagnosis {
            Some(d) => {
                *state.launch_fix.lock().unwrap() = d.fix.clone().map(|f| (game_id.clone(), f));
            }
            // A good session: problems later may be fixed again.
            None => state
                .fixes_tried
                .lock()
                .unwrap()
                .retain(|(g, _)| g != &game_id),
        }
        let error = diagnosis.map(|d| d.message);
        *state.running.lock().unwrap() = None;
        // A launch that failed isn't a session: no play time, no "last played".
        if error.is_none() {
            let mut user = state.user.lock().unwrap();
            let entry = user.entry(game_id.clone()).or_default();
            entry.play_seconds += session_seconds;
            entry.last_played = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs());
            if let Some(store) = &resume {
                if let Some(path) = store.written_since(session_start) {
                    entry.resume_state = Some(path);
                }
            }
        }
        let _ = state.save_user();
        if error.is_none() {
            let start = session_start
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or_default();
            let _ = crate::stats::record(
                &state.dirs.data,
                &crate::stats::Session {
                    game_id: game_id.clone(),
                    system_id: system_id.clone(),
                    start,
                    seconds: session_seconds,
                },
            );
        }
        if power_changed {
            crate::power::set(crate::power::PowerMode::Performance);
        }

        // Mirror this emulator's saves right after the session.
        let backup = state.settings.lock().unwrap().backup.clone();
        if backup.enabled {
            let (emu_id, dir, data, target) = (
                emu_id.clone(),
                emu_dir.clone(),
                state.dirs.data.clone(),
                backup.target(),
            );
            let done = tauri::async_runtime::spawn_blocking(move || {
                crate::backup::backup_lumen(&data, &target)?;
                crate::backup::backup_emulator(&emu_id, &dir, &target)
            })
            .await;
            if matches!(done, Ok(Ok(_))) {
                crate::commands::mark_backed_up(&state);
            }
        }

        let _ = std::fs::remove_file(frozen_marker(&state.dirs.data));
        crate::overlay::hide(&app);
        focus_lumen(&app, false);
        let _ = app.emit(
            EXITED_EVENT,
            Exited {
                game_id,
                session_seconds,
                error,
            },
        );
    });

    Ok(())
}

const PS3_FIRMWARE_ID: &str = "firmware:ps3";

/// RPCS3 needs the PS3 system software installed once. Its `--installfw` only
/// works with the GUI and doesn't exit afterwards, so we watch `dev_flash` and
/// close RPCS3 once the extracted files stop changing.
async fn ensure_ps3_firmware(
    app: &AppHandle,
    exe: &Path,
    bios: Option<&Path>,
    downloads: &Path,
) -> anyhow::Result<()> {
    let exe_dir = exe.parent().ok_or_else(|| {
        anyhow!(crate::tr!(
            "Geçersiz emülatör yolu",
            "Invalid emulator path"
        ))
    })?;
    let dev_flash = exe_dir.join("dev_flash");
    let marker = dev_flash.join("vsh").join("module").join("vsh.self");
    if marker.exists() {
        return Ok(());
    }
    // The user's copy wins; otherwise fetch it from Sony, who give it away free.
    let user_copy = bios.and_then(|b| crate::library::find_file(b, "ps3/PS3UPDAT.PUP"));
    let downloaded = user_copy.is_none();
    let pup = match user_copy {
        Some(pup) => pup,
        None => installer::download_ps3_firmware(app, PS3_FIRMWARE_ID, downloads).await?,
    };

    installer::emit(app, PS3_FIRMWARE_ID, "configuring", 1.0, None);
    let mut child = tokio::process::Command::new(exe)
        .arg("--installfw")
        .arg(&pup)
        .current_dir(exe_dir)
        .spawn()?;

    let started = Instant::now();
    let (mut last_count, mut stable_polls) = (0usize, 0u32);
    loop {
        tokio::select! {
            _ = child.wait() => break,
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
        }
        if marker.exists() {
            let count = walkdir::WalkDir::new(&dev_flash)
                .into_iter()
                .flatten()
                .count();
            stable_polls = if count == last_count {
                stable_polls + 1
            } else {
                0
            };
            last_count = count;
            if stable_polls >= 5 {
                let _ = child.kill().await;
                break;
            }
        }
        if started.elapsed() > Duration::from_secs(600) {
            let _ = child.kill().await;
            break;
        }
    }

    if downloaded {
        let _ = std::fs::remove_file(&pup);
    }
    if marker.exists() {
        installer::emit(app, PS3_FIRMWARE_ID, "done", 1.0, None);
        Ok(())
    } else {
        let message = crate::tr!(
            "PS3 firmware kurulamadı",
            "Couldn't install the PS3 firmware"
        );
        installer::emit(app, PS3_FIRMWARE_ID, "error", 0.0, Some(message.clone()));
        bail!(message)
    }
}

/// Where an emulator writes the state it saves on exit. The file is named by
/// the game's serial, which Lumen doesn't know up front, so the file written
/// during a session is remembered for that game.
struct ResumeStore {
    dir: PathBuf,
    suffix: &'static str,
}

impl ResumeStore {
    fn written_since(&self, since: SystemTime) -> Option<PathBuf> {
        std::fs::read_dir(&self.dir)
            .ok()?
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(self.suffix))
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
            .filter(|(modified, _)| *modified >= since)
            .max_by_key(|(modified, _)| *modified)
            .map(|(_, path)| path)
    }
}

fn resume_store(emulator: &str, exe_dir: &Path) -> Option<ResumeStore> {
    match emulator {
        "duckstation" => Some(ResumeStore {
            dir: exe_dir.join("savestates"),
            suffix: "_resume.sav",
        }),
        "pcsx2" => Some(ResumeStore {
            dir: exe_dir.join("sstates"),
            suffix: ".resume.p2s",
        }),
        _ => None,
    }
}

fn resume_file(state: &AppState, game_id: &str) -> Option<PathBuf> {
    let user = state.user.lock().unwrap();
    user.get(game_id)?
        .resume_state
        .clone()
        .filter(|p| p.exists())
}

/// Whether launching `game` would continue where the player left off.
pub fn is_resumable(state: &AppState, game: &Game) -> bool {
    if resume_file(state, &game.id).is_some() {
        return true;
    }
    // RetroArch auto-saves `<rom>.state.auto` and loads it on start.
    let system = manifest::system(&game.system_id);
    let runner = system.map(|s| crate::library::effective(s, state.bios_dir().as_deref()));
    if runner.as_ref().map(|r| r.emulator) != Some("retroarch") {
        return false;
    }
    let stem = game
        .path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let states = state.dirs.emulators.join("retroarch");
    crate::library::find_file(
        &states,
        &format!("**/states/**/{}.state.auto", glob::Pattern::escape(&stem)),
    )
    .is_some()
}

/// Cross-device sync: newer saves for this emulator from the backup folder
/// (another PC or handheld wrote them there), before the game starts. A
/// cloud folder that is slow to answer can't hold the launch up for long.
async fn pull_synced_saves(state: &AppState, emulator: &str) {
    let backup = state.settings.lock().unwrap().backup.clone();
    if !backup.enabled || !backup.sync {
        return;
    }
    let target = backup.target();
    if !target.is_dir() {
        return;
    }
    let install = state.dirs.emulators.join(emulator);
    let emulator = emulator.to_string();
    let pull = tauri::async_runtime::spawn_blocking(move || {
        crate::backup::restore_emulator(&emulator, &install, &target)
    });
    let _ = tokio::time::timeout(Duration::from_secs(20), pull).await;
}

/// Brings the emulator's window to the front once it appears and minimizes
/// Lumen, so the game never opens hidden behind Lumen's fullscreen window
/// (where button presses went to Lumen instead of the game). Slow starters
/// (RPCS3 compiling, Xenia) get half a minute. Lumen shows a loading screen
/// until then.
fn bring_up_when_shown(app: AppHandle, pid: u32) {
    tauri::async_runtime::spawn(async move {
        for _ in 0..120 {
            tokio::time::sleep(Duration::from_millis(250)).await;
            let running = app
                .state::<AppState>()
                .running
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|g| g.pid == Some(pid));
            if !running {
                return;
            }
            if crate::windows::focus_process(pid) {
                hide_lumen(&app);
                crate::overlay::show(&app);
                let _ = app.emit(SHOWN_EVENT, ());
                keep_game_in_front(&app, pid).await;
                return;
            }
        }
        let _ = app.emit(SHOWN_EVENT, ());
    });
}

/// Emulators often swap windows while starting (DuckStation shows its main
/// window, then the fullscreen game window), and Windows may hand focus back
/// to Lumen in between, leaving the game behind. For a few seconds, put the
/// game back in front whenever it loses focus, unless the player opened the
/// quick menu (Lumen restored).
async fn keep_game_in_front(app: &AppHandle, pid: u32) {
    for _ in 0..16 {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let state = app.state::<AppState>();
        let still_running = state
            .running
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|g| g.pid == Some(pid));
        let menu_open = app
            .get_webview_window("main")
            .is_some_and(|w| !w.is_minimized().unwrap_or(true));
        if !still_running || menu_open {
            return;
        }
        if !crate::windows::is_foreground(pid) {
            crate::windows::focus_process(pid);
        }
    }
}

/// Steps Lumen out of the way while the game plays; `focus_lumen` brings it
/// back for the quick menu and when the game ends.
fn hide_lumen(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_always_on_top(false);
        let _ = window.minimize();
    }
}

/// Shows Lumen above the emulator. `on_top` keeps it there for the quick menu.
pub fn focus_lumen(app: &AppHandle, on_top: bool) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.unminimize();
    let _ = window.set_always_on_top(on_top);
    #[cfg(windows)]
    if let Ok(hwnd) = window.hwnd() {
        crate::windows::bring_to_front(hwnd.0 as isize);
    }
    let _ = window.set_focus();
}

/// Emulators without a pause-on-focus-loss option; the quick menu freezes
/// their process instead (see presets.rs for the ones that pause themselves).
const FREEZE_FOR_MENU: &[&str] = &["vita3k", "xemu", "xenia", "shadps4", "sharpemu", "cemu"];

/// Marker for a frozen emulator, so a crashed Lumen can thaw it on next start.
fn frozen_marker(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("frozen-pid")
}

/// Opens the in-game quick menu: Lumen on top, the game paused (by focus loss,
/// or by freezing emulators that can't pause themselves).
pub fn open_quick_menu(app: &AppHandle) {
    crate::overlay::hide(app);
    let _ = app.emit(crate::input::QUICK_MENU_EVENT, ());
    let frozen = {
        let state = app.state::<AppState>();
        let mut running = state.running.lock().unwrap();
        match running.as_mut() {
            Some(game) if !game.suspended && FREEZE_FOR_MENU.contains(&game.emulator.as_str()) => {
                match game.pid {
                    Some(pid) if crate::windows::set_process_suspended(pid, true) => {
                        game.suspended = true;
                        Some(pid)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    };
    if let Some(pid) = frozen {
        let data = app.state::<AppState>().dirs.data.clone();
        let _ = std::fs::write(frozen_marker(&data), pid.to_string());
    }
    focus_lumen(app, true);
}

/// Thaws the emulator if the quick menu froze it.
fn thaw(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut running = state.running.lock().unwrap();
    if let Some(game) = running.as_mut().filter(|g| g.suspended) {
        if let Some(pid) = game.pid {
            crate::windows::set_process_suspended(pid, false);
        }
        game.suspended = false;
    }
    drop(running);
    let _ = std::fs::remove_file(frozen_marker(&state.dirs.data));
}

/// On start: a process Lumen froze before crashing would stay frozen forever.
/// Takes the data dir (not the AppHandle) because it runs during setup,
/// before AppState is managed: `app.state()` there panics.
pub fn thaw_leftover(data_dir: &Path) {
    let marker = frozen_marker(data_dir);
    if let Some(pid) = std::fs::read_to_string(&marker)
        .ok()
        .and_then(|s| s.trim().parse().ok())
    {
        crate::windows::set_process_suspended(pid, false);
    }
    let _ = std::fs::remove_file(marker);
}

/// Leaves the quick menu: thaws a frozen emulator, drops Lumen's
/// always-on-top and refocuses the game.
pub fn resume_game(app: &AppHandle) {
    thaw(app);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_always_on_top(false);
    }
    let state = app.state::<AppState>();
    let pid = state.running.lock().unwrap().as_ref().and_then(|g| g.pid);
    if let Some(pid) = pid {
        if crate::windows::focus_process(pid) {
            hide_lumen(app);
            crate::overlay::show(app);
        }
    }
}

/// How long an emulator gets to save state and exit after being asked.
const CLOSE_GRACE: Duration = Duration::from_secs(10);

/// Closes the emulator like its window's X button so it runs its own
/// shutdown (save state on exit, flushing memory cards), and only kills it
/// if it hasn't exited after `CLOSE_GRACE`.
async fn close_gracefully(child: &mut tokio::process::Child) {
    // A frozen process can't handle WM_CLOSE; thaw it first (no-op otherwise).
    if let Some(pid) = child.id() {
        crate::windows::set_process_suspended(pid, false);
    }
    let asked = child.id().map(crate::windows::request_close).unwrap_or(0);
    if asked > 0
        && tokio::time::timeout(CLOSE_GRACE, child.wait())
            .await
            .is_ok()
    {
        return;
    }
    let _ = child.kill().await;
}

/// Ends the running game; used by the home combo and the in-app "close game" button.
pub fn stop(state: &AppState) {
    if let Some(game) = state.running.lock().unwrap().as_mut() {
        if let Some(stop) = game.stop.take() {
            let _ = stop.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn freeze_list_names_real_emulators() {
        for id in super::FREEZE_FOR_MENU {
            assert!(
                crate::manifest::emulator(id).is_some(),
                "unknown emulator {id}"
            );
        }
    }
}
