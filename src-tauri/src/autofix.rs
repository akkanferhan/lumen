//! Applies the fix `launch_check` found for a failed launch. Files Lumen
//! manages (emulators, cores, Microsoft's VC++ runtime) are repaired
//! automatically; security features (Smart App Control, Defender) are never
//! changed behind the user's back, only their Windows page is opened.

use crate::{installer, launch_check::Fix, manifest, state::AppState};
use anyhow::{anyhow, bail};
use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// Repaired: launching again should work.
    Fixed,
    /// A Windows settings page is open; the user decides.
    OpenedSettings,
}

/// Progress id for the VC++ runtime install (shown as a toast).
pub const VC_REDIST_ID: &str = "fix:vcredist";
const VC_REDIST_URL: &str = "https://aka.ms/vs/17/release/vc_redist.x64.exe";

pub async fn apply(app: &AppHandle, fix: &Fix) -> anyhow::Result<Outcome> {
    let state = app.state::<AppState>();
    match fix {
        Fix::ReinstallEmulator(id) => {
            let emu = manifest::emulator(id).ok_or_else(|| anyhow!("Bilinmeyen emülatör"))?;
            // Unpacks over the install: damaged files are replaced, saves stay.
            installer::install_emulator(
                app,
                emu,
                &state.dirs.emulators,
                &state.dirs.downloads,
                state.bios_dir().as_deref(),
                state.tier(),
            )
            .await?;
            Ok(Outcome::Fixed)
        }
        Fix::ReinstallCore(core) => {
            let retroarch =
                manifest::emulator("retroarch").ok_or_else(|| anyhow!("RetroArch yok"))?;
            let exe = installer::find_exe(retroarch, &state.dirs.emulators.join("retroarch"))
                .ok_or_else(|| anyhow!("RetroArch kurulu değil"))?;
            let cores = exe.parent().map(|d| d.join("cores")).unwrap_or_default();
            let _ = std::fs::remove_file(cores.join(format!("{core}_libretro.dll")));
            installer::install_core(app, core, &cores, &state.dirs.downloads).await?;
            Ok(Outcome::Fixed)
        }
        Fix::VcRedist => {
            let installer_path =
                installer::download_file(app, VC_REDIST_ID, VC_REDIST_URL, &state.dirs.downloads)
                    .await?;
            installer::emit(app, VC_REDIST_ID, "configuring", 1.0, None);
            let result = run_elevated(&installer_path, &["/install", "/quiet", "/norestart"]).await;
            let _ = std::fs::remove_file(&installer_path);
            match result {
                Ok(()) => {
                    installer::emit(app, VC_REDIST_ID, "done", 1.0, None);
                    Ok(Outcome::Fixed)
                }
                Err(e) => {
                    installer::emit(app, VC_REDIST_ID, "error", 0.0, Some(e.to_string()));
                    Err(e)
                }
            }
        }
        Fix::SmartAppControl => open_settings("windowsdefender://appbrowser"),
        Fix::Defender => open_settings("windowsdefender://threat"),
    }
}

/// Runs an installer through UAC (Windows asks the user) and waits for it.
async fn run_elevated(path: &std::path::Path, args: &[&str]) -> anyhow::Result<()> {
    let arg_list = args
        .iter()
        .map(|a| format!("'{a}'"))
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        "$p = Start-Process -FilePath '{}' -ArgumentList {arg_list} -Verb RunAs -Wait -PassThru; exit $p.ExitCode",
        path.to_string_lossy().replace('\'', "''")
    );
    let status = tokio::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .await?;
    // 3010: installed, restart pending. 1638: a newer version is already there.
    match status.code() {
        Some(0 | 3010 | 1638) => Ok(()),
        Some(1223) | Some(1) => bail!("Kurulum onaylanmadı"),
        code => bail!("Kurulum başarısız oldu (kod {code:?})"),
    }
}

fn open_settings(uri: &str) -> anyhow::Result<Outcome> {
    std::process::Command::new("explorer.exe")
        .arg(uri)
        .spawn()?;
    Ok(Outcome::OpenedSettings)
}
