//! Installed GOG games, read from the registry keys GOG's installers write
//! (`GOG.com\Games\<id>`: name, executable, working folder). GOG games are
//! DRM-free, so they start straight from their executable; Galaxy isn't
//! needed. Art comes from GOG's public product API and is cached.

use crate::library::Game;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub const GOG: &str = "gog";

pub struct Installed {
    pub id: String,
    pub name: String,
    pub exe: PathBuf,
}

pub fn scan() -> Vec<Game> {
    installed()
        .into_iter()
        .filter(|g| g.exe.is_file())
        .map(|g| {
            let mut game = crate::library::make_virtual_game(
                GOG,
                &format!("gog:{}", g.id),
                &g.name,
                g.exe.to_string_lossy().into_owned(),
            );
            // Replaced by the art URL in `art`; the product id until then.
            game.thumb_name = g.id;
            game
        })
        .collect()
}

#[cfg(windows)]
fn installed() -> Vec<Installed> {
    use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
    let Ok(games) =
        RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\WOW6432Node\GOG.com\Games")
    else {
        return Vec::new();
    };
    games
        .enum_keys()
        .flatten()
        .filter_map(|id| {
            let key = games.open_subkey(&id).ok()?;
            let name: String = key.get_value("gameName").ok()?;
            let exe: String = key.get_value("exe").ok()?;
            // DLC entries point at their base game's id.
            let depends: String = key.get_value("dependsOn").unwrap_or_default();
            depends.is_empty().then(|| Installed {
                id,
                name,
                exe: PathBuf::from(exe),
            })
        })
        .collect()
}

#[cfg(not(windows))]
fn installed() -> Vec<Installed> {
    Vec::new()
}

/// GOG product id → logo art URL, from the public product API (cached).
pub async fn art(ids: &[String], cache: &Path) -> HashMap<String, String> {
    let mut known: HashMap<String, String> = std::fs::read(cache)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let mut changed = false;
    let missing: Vec<&String> = ids.iter().filter(|id| !known.contains_key(*id)).collect();
    for id in missing {
        let url = format!("https://api.gog.com/products/{id}");
        let Ok(response) = crate::installer::http().get(&url).send().await else {
            continue;
        };
        let Ok(body) = response.json::<serde_json::Value>().await else {
            continue;
        };
        let image = body["images"]["logo2x"]
            .as_str()
            .or_else(|| body["images"]["background"].as_str());
        if let Some(image) = image {
            // The API gives protocol-relative URLs.
            let url = if image.starts_with("//") {
                format!("https:{image}")
            } else {
                image.to_string()
            };
            known.insert(id.clone(), url);
            changed = true;
        }
    }
    if changed {
        if let Ok(bytes) = serde_json::to_vec(&known) {
            let _ = std::fs::write(cache, bytes);
        }
    }
    known
}

/// Starts a GOG game from its own folder.
pub fn launch(exe: &Path) -> anyhow::Result<()> {
    let dir = exe.parent().unwrap_or(Path::new("."));
    std::process::Command::new(exe).current_dir(dir).spawn()?;
    Ok(())
}
