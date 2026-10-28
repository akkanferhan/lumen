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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exited {
    pub game_id: String,
    pub session_seconds: u64,
}

/// `fresh` starts from the beginning instead of the last resume state.
pub async fn launch(app: &AppHandle, game: &Game, fresh: bool) -> anyhow::Result<()> {
    let state = app.state::<AppState>();
    if state.running.lock().unwrap().is_some() {
        bail!("Zaten bir oyun çalışıyor");
    }

    // Store games start through their launcher; Lumen can't follow their process.
    if crate::pc::is_pc(&game.system_id) {
        crate::pc::launch(game)?;
        let mut user = state.user.lock().unwrap();
        user.entry(game.id.clone()).or_default().last_played = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs());
        drop(user);
        let _ = state.save_user();
        return Ok(());
    }

    let system = manifest::system(&game.system_id).ok_or_else(|| anyhow!("Bilinmeyen sistem"))?;
    let bios = state.bios_dir();
    let runner = crate::library::effective(system, bios.as_deref());
    if !runner.fallback {
        if let Some(missing) = crate::library::bios_status(system, bios.as_deref())
            .into_iter()
            .find(|b| !b.found && !b.optional)
        {
            bail!("Eksik dosya: {}", missing.label);
        }
    }
    let emu = manifest::emulator(runner.emulator).ok_or_else(|| anyhow!("Bilinmeyen emülatör"))?;
    let exe = installer::find_exe(emu, &state.dirs.emulators.join(&emu.id))
        .ok_or_else(|| anyhow!("{} kurulu değil", emu.name))?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| anyhow!("Geçersiz emülatör yolu"))?;

    config::configure(emu, exe_dir, bios.as_deref(), state.tier())?;
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
                bail!("{} çekirdeği kurulu değil", system.short);
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
    let args: Vec<String> = crate::presets::launch_args(emu, exe_dir, state.tier(), fresh)
        .into_iter()
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
        .current_dir(exe_dir)
        .spawn()?;
    let (stop_tx, stop_rx) = oneshot::channel();
    *state.running.lock().unwrap() = Some(RunningGame {
        game_id: game.id.clone(),
        stop: Some(stop_tx),
        pid: child.id(),
    });
    let session_start = SystemTime::now();

    // Match the ASUS power mode to the system; Performance (the default) is
    // restored when the game ends.
    let power = state.settings.lock().unwrap().power.resolve(system.power);
    let power_changed = power.is_some_and(crate::power::set);

    let app = app.clone();
    let game_id = game.id.clone();
    let emu_id = emu.id.clone();
    let emu_dir = exe_dir.to_path_buf();
    tauri::async_runtime::spawn(async move {
        let started = Instant::now();
        tokio::select! {
            _ = child.wait() => {}
            _ = stop_rx => close_gracefully(&mut child).await,
        }
        let session_seconds = started.elapsed().as_secs();

        let state = app.state::<AppState>();
        *state.running.lock().unwrap() = None;
        {
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

        focus_lumen(&app, false);
        let _ = app.emit(
            EXITED_EVENT,
            Exited {
                game_id,
                session_seconds,
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
    let exe_dir = exe
        .parent()
        .ok_or_else(|| anyhow!("Geçersiz emülatör yolu"))?;
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
        let message = "PS3 firmware kurulamadı".to_string();
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

/// Leaves the quick menu: drops Lumen's always-on-top and refocuses the game.
pub fn resume_game(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_always_on_top(false);
    }
    let state = app.state::<AppState>();
    let pid = state.running.lock().unwrap().as_ref().and_then(|g| g.pid);
    if let Some(pid) = pid {
        crate::windows::focus_process(pid);
    }
}

/// How long an emulator gets to save state and exit after being asked.
const CLOSE_GRACE: Duration = Duration::from_secs(10);

/// Closes the emulator like its window's X button so it runs its own
/// shutdown (save state on exit, flushing memory cards), and only kills it
/// if it hasn't exited after `CLOSE_GRACE`.
async fn close_gracefully(child: &mut tokio::process::Child) {
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
