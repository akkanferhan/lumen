//! Games installed by the itch.io app. Each install folder holds
//! `.itch/receipt.json.gz` with the game's title and cover; the app keeps
//! installs in `%APPDATA%\itch\apps` unless the user added other locations,
//! which aren't read. Receipts don't name the executable, so the likeliest
//! one is picked: the largest .exe near the top, skipping uninstallers,
//! crash reporters and redistributable installers.

use crate::library::Game;
use flate2::read::GzDecoder;
use serde::Deserialize;
use std::{
    io::Read,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

pub const ITCH: &str = "itch";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Receipt {
    game: ReceiptGame,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReceiptGame {
    id: u64,
    title: String,
    #[serde(default)]
    cover_url: Option<String>,
    #[serde(default)]
    classification: Option<String>,
}

pub fn apps_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("itch").join("apps"))
}

pub fn scan(apps: &Path) -> Vec<Game> {
    let Ok(entries) = std::fs::read_dir(apps) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let dir = e.path();
            let receipt = read_receipt(&dir.join(".itch").join("receipt.json.gz"))?;
            // Tools, soundtracks, comics and assets are on itch too.
            if receipt
                .game
                .classification
                .as_deref()
                .is_some_and(|c| c != "game")
            {
                return None;
            }
            let exe = main_exe(&dir)?;
            let mut game = crate::library::make_virtual_game(
                ITCH,
                &format!("itch:{}", receipt.game.id),
                &receipt.game.title,
                exe.to_string_lossy().into_owned(),
            );
            game.thumb_name = receipt.game.cover_url.unwrap_or_default();
            Some(game)
        })
        .collect()
}

fn read_receipt(path: &Path) -> Option<Receipt> {
    let mut json = String::new();
    GzDecoder::new(std::fs::File::open(path).ok()?)
        .read_to_string(&mut json)
        .ok()?;
    serde_json::from_str(&json).ok()
}

/// The largest .exe within two folders of the install, minus helpers.
fn main_exe(dir: &Path) -> Option<PathBuf> {
    const NOT_THE_GAME: &[&str] = &[
        "unins",
        "uninstall",
        "crash",
        "setup",
        "install",
        "vcredist",
        "vc_redist",
        "dxsetup",
        "dotnet",
        "oalinst",
        "ue4prereq",
        "launcher_prereq",
    ];
    WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.ends_with(".exe") && !NOT_THE_GAME.iter().any(|n| name.contains(n))
        })
        .filter(|e| !e.path().components().any(|c| c.as_os_str() == ".itch"))
        .max_by_key(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .map(|e| e.path().to_path_buf())
}

/// Starts an itch game from its own folder.
pub fn launch(exe: &Path) -> anyhow::Result<()> {
    let dir = exe.parent().unwrap_or(Path::new("."));
    std::process::Command::new(exe).current_dir(dir).spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};
    use std::io::Write;

    fn install(apps: &Path, folder: &str, receipt: &str, files: &[(&str, usize)]) {
        let dir = apps.join(folder);
        std::fs::create_dir_all(dir.join(".itch")).unwrap();
        let mut gz = GzEncoder::new(Vec::new(), Compression::default());
        gz.write_all(receipt.as_bytes()).unwrap();
        std::fs::write(dir.join(".itch/receipt.json.gz"), gz.finish().unwrap()).unwrap();
        for (name, size) in files {
            let path = dir.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, vec![0u8; *size]).unwrap();
        }
    }

    #[test]
    fn reads_games_and_picks_their_executable() {
        let apps = std::env::temp_dir().join(format!("lumen-itch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&apps);
        install(
            &apps,
            "celeste-classic",
            r#"{"game":{"id":42,"title":"Pico Climb","coverUrl":"https://img.itch.zone/c.png","classification":"game"}}"#,
            &[
                ("Pico Climb/PicoClimb.exe", 5000),
                ("Pico Climb/UnityCrashHandler64.exe", 9000),
                ("unins000.exe", 9000),
            ],
        );
        install(
            &apps,
            "soundtrack",
            r#"{"game":{"id":7,"title":"OST","classification":"soundtrack"}}"#,
            &[("player.exe", 10)],
        );
        let games = scan(&apps);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Pico Climb");
        assert_eq!(games[0].thumb_name, "https://img.itch.zone/c.png");
        assert!(
            games[0].path.ends_with("Pico Climb/PicoClimb.exe"),
            "{:?}",
            games[0].path
        );
        std::fs::remove_dir_all(&apps).unwrap();
    }
}
