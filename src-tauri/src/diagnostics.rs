//! "Sorun bildir": one zip on the desktop with what's needed to look into
//! a problem — Lumen's logs, the last RetroArch launch log, the device,
//! installed emulator versions, library counts and settings. Keys and
//! accounts (Steam, SteamGridDB, RetroAchievements) and play data stay out;
//! the kids-mode PIN hash is blanked.

use crate::state::AppState;
use serde_json::json;
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};

/// Writes the report into `dir` as `<name>.zip`; returns its path.
pub fn export(state: &AppState, dir: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let path = dir.join(format!("{name}.zip"));
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path)?);
    let options = zip::write::SimpleFileOptions::default();

    zip.start_file("bilgi.json", options)?;
    zip.write_all(serde_json::to_string_pretty(&info(state))?.as_bytes())?;

    for (inside, file) in logs(state) {
        if let Ok(bytes) = std::fs::read(&file) {
            zip.start_file(inside, options)?;
            zip.write_all(&bytes)?;
        }
    }
    zip.finish()?;
    Ok(path)
}

fn info(state: &AppState) -> serde_json::Value {
    let mut settings = serde_json::to_value(&*state.settings.lock().unwrap()).unwrap_or_default();
    if let Some(kids) = settings.get_mut("kids") {
        kids["pinHash"] = json!(kids["pinHash"].as_str().map(|_| "(set)"));
    }
    let emulators: BTreeMap<&str, Option<String>> = crate::manifest::manifest()
        .emulators
        .iter()
        .map(|e| {
            let dir = state.dirs.emulators.join(&e.id);
            (
                e.id.as_str(),
                dir.is_dir()
                    .then(|| crate::installer::installed_version(&dir).unwrap_or_default()),
            )
        })
        .collect();
    let mut games: BTreeMap<String, usize> = BTreeMap::new();
    for game in state.library.lock().unwrap().iter() {
        *games.entry(game.system_id.clone()).or_default() += 1;
    }
    let device = crate::device::device();
    json!({
        "lumen": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "device": { "cpu": device.cpu, "model": device.model, "tier": state.tier() },
        "powerAvailable": crate::power::available(),
        "emulators": emulators,
        "games": games,
        "running": state.running.lock().unwrap().as_ref().map(|g| g.emulator.clone()),
        "settings": settings,
    })
}

/// (name in the zip, file on disk) for every log Lumen keeps.
fn logs(state: &AppState) -> Vec<(String, PathBuf)> {
    let data = &state.dirs.data;
    let mut logs = vec![
        ("ally.log".to_string(), data.join("ally.log")),
        ("crash.log".to_string(), data.join("crash.log")),
    ];
    // RetroArch's log of the last launch, wherever its build unpacked.
    if let Some(log) = walkdir::WalkDir::new(state.dirs.emulators.join("retroarch"))
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .find(|e| e.file_name() == "lumen-launch.log")
    {
        logs.push(("retroarch-launch.log".to_string(), log.into_path()));
    }
    logs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_has_logs_and_no_secrets() {
        let data = std::env::temp_dir().join(format!("lumen-diag-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        let state = AppState::at(data.clone()).unwrap();
        std::fs::write(data.join("ally.log"), "gamepad mode sent").unwrap();
        std::fs::write(data.join("steam.json"), r#"{"key":"SECRET"}"#).unwrap();
        state.settings.lock().unwrap().kids.pin_hash = Some("abc".into());

        let zip_path = export(&state, &data, "rapor").unwrap();
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&zip_path).unwrap()).unwrap();
        let names: Vec<String> = zip.file_names().map(String::from).collect();
        assert!(names.contains(&"ally.log".to_string()), "{names:?}");
        assert!(!names.iter().any(|n| n.contains("steam")));
        let mut info = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("bilgi.json").unwrap(), &mut info).unwrap();
        assert!(info.contains("(set)") && !info.contains("abc") && !info.contains("SECRET"));
        std::fs::remove_dir_all(&data).unwrap();
    }
}
