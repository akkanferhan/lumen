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
    path::Path,
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

pub async fn launch(app: &AppHandle, game: &Game) -> anyhow::Result<()> {
    let state = app.state::<AppState>();
    if state.running.lock().unwrap().is_some() {
        bail!("Zaten bir oyun çalışıyor");
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
    match emu.id.as_str() {
        "rpcs3" => ensure_ps3_firmware(app, &exe, bios.as_deref(), &state.dirs.downloads).await?,
        "xemu" => installer::ensure_xbox_hdd(app, exe_dir, &state.dirs.downloads).await?,
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
    let args: Vec<String> = crate::presets::launch_args(emu, exe_dir, state.tier())
        .into_iter()
        .chain(
            emu.args
                .iter()
                .map(|a| a.replace("{rom}", &rom).replace("{core}", &core)),
        )
        .collect();

    let mut child = tokio::process::Command::new(&exe)
        .args(&args)
        .current_dir(exe_dir)
        .spawn()?;
    let (kill_tx, kill_rx) = oneshot::channel();
    *state.running.lock().unwrap() = Some(RunningGame {
        game_id: game.id.clone(),
        kill: Some(kill_tx),
    });

    let app = app.clone();
    let game_id = game.id.clone();
    tauri::async_runtime::spawn(async move {
        let started = Instant::now();
        tokio::select! {
            _ = child.wait() => {}
            _ = kill_rx => { let _ = child.kill().await; }
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
        }
        let _ = state.save_user();

        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
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

/// Ends the running game; used by the home combo and the in-app "close game" button.
pub fn stop(state: &AppState) {
    if let Some(game) = state.running.lock().unwrap().as_mut() {
        if let Some(kill) = game.kill.take() {
            let _ = kill.send(());
        }
    }
}
