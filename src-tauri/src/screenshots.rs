//! Screenshots of the running game, kept per game in
//! `screenshots/<game id>/<unix ms>.png` and shown on the game's sheet.
//! RetroArch and DuckStation save their own frame (Lumen presses their
//! screenshot key, see `controls`, and moves the file over); other
//! emulators get the screen captured, minus Lumen's menu button. A capture
//! that comes back black (an emulator GDI can't see) is not kept.

use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shot {
    pub path: PathBuf,
    /// Unix seconds.
    pub taken_at: u64,
}

pub fn dir(data_dir: &Path, game_id: &str) -> PathBuf {
    data_dir.join("screenshots").join(game_id)
}

/// A game's screenshots, newest first.
pub fn list(data_dir: &Path, game_id: &str) -> Vec<Shot> {
    let mut shots: Vec<Shot> = std::fs::read_dir(dir(data_dir, game_id))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .filter_map(|path| {
            let ms: u64 = path.file_stem()?.to_str()?.parse().ok()?;
            Some(Shot {
                path,
                taken_at: ms / 1000,
            })
        })
        .collect();
    shots.sort_by(|a, b| b.taken_at.cmp(&a.taken_at).then(b.path.cmp(&a.path)));
    shots
}

/// Deletes one screenshot; only files inside the screenshots folder.
pub fn delete(data_dir: &Path, path: &Path) -> anyhow::Result<()> {
    let root = data_dir.join("screenshots");
    anyhow::ensure!(
        path.starts_with(&root) && path.extension().is_some_and(|e| e == "png"),
        crate::tr!(
            "Bu dosya bir ekran görüntüsü değil",
            "This file isn't a screenshot"
        )
    );
    std::fs::remove_file(path)?;
    Ok(())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Takes a screenshot of the running game (blocking; call off the UI
/// thread). Returns where it was saved.
pub fn take(app: &AppHandle) -> anyhow::Result<PathBuf> {
    let state = app.state::<crate::state::AppState>();
    let (game_id, emulator, pid, exe_dir) = {
        let running = state.running.lock().unwrap();
        let game = running
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!(crate::tr!("Oyun çalışmıyor", "No game is running")))?;
        (
            game.game_id.clone(),
            game.emulator.clone(),
            game.pid,
            game.exe_dir.clone(),
        )
    };
    let target_dir = dir(&state.dirs.data, &game_id);
    std::fs::create_dir_all(&target_dir)?;
    let target = target_dir.join(format!("{}.png", now_ms()));

    let in_front = pid.is_some_and(crate::windows::is_foreground);
    if in_front && matches!(emulator.as_str(), "retroarch" | "duckstation") {
        if let Some(file) = emulator_screenshot(&exe_dir) {
            // Moved across drives if need be.
            if std::fs::rename(&file, &target).is_err() {
                std::fs::copy(&file, &target)?;
                let _ = std::fs::remove_file(&file);
            }
            return Ok(target);
        }
    }

    // Lumen's menu button would be in the picture.
    crate::overlay::hide(app);
    std::thread::sleep(Duration::from_millis(120));
    let frame = crate::windows::capture_screen();
    if in_front {
        crate::overlay::show(app);
    }
    let (width, height, pixels) = frame.ok_or_else(|| {
        anyhow::anyhow!(crate::tr!(
            "Ekran yakalanamadı",
            "Couldn't capture the screen"
        ))
    })?;
    anyhow::ensure!(
        !mostly_black(&pixels),
        crate::tr!(
            "Bu emülatörün görüntüsü yakalanamıyor",
            "This emulator's picture can't be captured"
        )
    );
    write_png(&target, width, height, &pixels)?;
    Ok(target)
}

/// Presses the emulator's screenshot key and waits for the file it writes
/// under `exe_dir/screenshots`.
fn emulator_screenshot(exe_dir: &Path) -> Option<PathBuf> {
    let since = SystemTime::now() - Duration::from_millis(200);
    crate::controls::tap(crate::controls::Hotkey::Screenshot);
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(100));
        let newest = walkdir::WalkDir::new(exe_dir.join("screenshots"))
            .max_depth(3)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.into_path())))
            .filter(|(modified, _)| *modified >= since)
            .max_by_key(|(modified, _)| *modified);
        if let Some((_, path)) = newest {
            // Give the emulator a moment to finish writing it.
            std::thread::sleep(Duration::from_millis(250));
            return Some(path);
        }
    }
    None
}

/// Black (or nearly) on nineteen of twenty samples: a failed capture.
fn mostly_black(rgba: &[u8]) -> bool {
    let pixels = rgba.len() / 4;
    if pixels == 0 {
        return true;
    }
    let step = (pixels / 2000).max(1);
    let (mut dark, mut total) = (0, 0);
    for i in (0..pixels).step_by(step) {
        let px = &rgba[i * 4..i * 4 + 3];
        if px.iter().all(|&c| c < 12) {
            dark += 1;
        }
        total += 1;
    }
    dark * 20 >= total * 19
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> anyhow::Result<()> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_shots_newest_first_and_refuses_other_files() {
        let data = std::env::temp_dir().join(format!("lumen-shots-{}", std::process::id()));
        let shots = dir(&data, "g1");
        std::fs::create_dir_all(&shots).unwrap();
        write_png(&shots.join("1000.png"), 2, 1, &[255; 8]).unwrap();
        write_png(&shots.join("5000.png"), 2, 1, &[255; 8]).unwrap();
        std::fs::write(shots.join("notes.txt"), "x").unwrap();

        let listed = list(&data, "g1");
        assert_eq!(
            listed.iter().map(|s| s.taken_at).collect::<Vec<_>>(),
            [5, 1]
        );
        assert!(delete(&data, &data.join("settings.json")).is_err());
        delete(&data, &listed[0].path).unwrap();
        assert_eq!(list(&data, "g1").len(), 1);
        std::fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn black_frames_are_failed_captures() {
        assert!(mostly_black(&[0u8; 4000]));
        let mut frame = vec![0u8; 4000];
        frame.iter_mut().step_by(3).for_each(|c| *c = 200);
        assert!(!mostly_black(&frame));
    }
}
