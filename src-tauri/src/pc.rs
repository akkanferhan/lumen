//! Installed PC games from Steam and the Epic Games Launcher (plus GOG and
//! itch.io, see `gog` and `itch`), so Lumen is the one library on the device. Read from the launchers' own files; games are
//! started through the launcher's URL scheme (DRM, overlays and cloud saves
//! keep working).

use crate::library::Game;
use regex::Regex;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub const STEAM: &str = "steam";
pub const EPIC: &str = "epic";

pub fn is_pc(system_id: &str) -> bool {
    system_id == STEAM
        || system_id == EPIC
        || system_id == crate::xbox::XBOX_APP
        || system_id == crate::gog::GOG
        || system_id == crate::itch::ITCH
}

/// Steam tools that show up as installed apps but aren't games.
const STEAM_NOT_GAMES: &[&str] = &[
    "228980", "1070560", "1391110", "1628350", "1493710", "2180100",
];

pub fn scan() -> Vec<Game> {
    let mut games = Vec::new();
    if let Some(steam) = steam_root() {
        games.extend(scan_steam(&steam));
    }
    games.extend(scan_epic(&epic_manifests()));
    games.extend(crate::xbox::scan());
    games.extend(crate::gog::scan());
    if let Some(apps) = crate::itch::apps_dir() {
        games.extend(crate::itch::scan(&apps));
    }
    games
}

fn steam_root() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let path: String = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Software\Valve\Steam")
            .ok()?
            .get_value("SteamPath")
            .ok()?;
        Some(PathBuf::from(path))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn epic_manifests() -> PathBuf {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
    Path::new(&program_data).join(r"Epic\EpicGamesLauncher\Data\Manifests")
}

fn vdf_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let pattern = format!(r#""{key}"\s+"([^"]*)""#);
    Regex::new(&pattern)
        .ok()?
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
}

pub fn scan_steam(steam_root: &Path) -> Vec<Game> {
    static PATHS: OnceLock<Regex> = OnceLock::new();
    let paths = PATHS.get_or_init(|| Regex::new(r#""path"\s+"([^"]+)""#).unwrap());

    let folders_file = steam_root.join("steamapps").join("libraryfolders.vdf");
    let mut libraries: Vec<PathBuf> = fs::read_to_string(folders_file)
        .map(|text| {
            paths
                .captures_iter(&text)
                .map(|c| PathBuf::from(c[1].replace("\\\\", "\\")))
                .collect()
        })
        .unwrap_or_default();
    if libraries.is_empty() {
        libraries.push(steam_root.to_path_buf());
    }

    let mut games = Vec::new();
    for library in libraries {
        let Ok(entries) = fs::read_dir(library.join("steamapps")) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            let Ok(text) = fs::read_to_string(entry.path()) else {
                continue;
            };
            if let Some(game) = steam_game(&text) {
                games.push(game);
            }
        }
    }
    games
}

/// One `appmanifest_*.acf`; only fully installed games (StateFlags bit 4).
fn steam_game(acf: &str) -> Option<Game> {
    let appid = vdf_value(acf, "appid")?;
    let name = vdf_value(acf, "name")?;
    let flags: u32 = vdf_value(acf, "StateFlags")?.parse().ok()?;
    if flags & 4 == 0 || STEAM_NOT_GAMES.contains(&appid) || name.contains("Redistributable") {
        return None;
    }
    Some(pc_game(
        STEAM,
        appid,
        name,
        format!("steam://rungameid/{appid}"),
    ))
}

pub fn scan_epic(manifests: &Path) -> Vec<Game> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Item {
        display_name: String,
        app_name: String,
        catalog_namespace: String,
        catalog_item_id: String,
        #[serde(default, rename = "bIsIncompleteInstall")]
        incomplete: bool,
        #[serde(default)]
        app_categories: Vec<String>,
    }

    let Ok(entries) = fs::read_dir(manifests) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "item"))
        .filter_map(|e| serde_json::from_slice::<Item>(&fs::read(e.path()).ok()?).ok())
        // Skip half-downloaded games and add-ons (DLC manifests have no "games" category).
        .filter(|i| {
            !i.incomplete
                && (i.app_categories.is_empty() || i.app_categories.iter().any(|c| c == "games"))
        })
        .map(|i| {
            let uri = format!(
                "com.epicgames.launcher://apps/{}%3A{}%3A{}?action=launch&silent=true",
                i.catalog_namespace, i.catalog_item_id, i.app_name
            );
            pc_game(EPIC, &i.app_name, &i.display_name, uri)
        })
        .collect()
}

fn pc_game(store: &str, id: &str, name: &str, uri: String) -> Game {
    let mut game = crate::library::make_virtual_game(store, &format!("{store}:{id}"), name, uri);
    // Steam art is fetched by app id.
    game.thumb_name = id.to_string();
    game
}

/// Hands the launcher URI to Windows; the store client starts the game.
pub fn launch(game: &Game) -> anyhow::Result<()> {
    // DRM-free stores: `path` is the game's executable.
    if game.system_id == crate::gog::GOG {
        return crate::gog::launch(&game.path);
    }
    if game.system_id == crate::itch::ITCH {
        return crate::itch::launch(&game.path);
    }
    std::process::Command::new("explorer.exe")
        .arg(&game.path)
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_installed_steam_games_only() {
        let installed = "\"AppState\"\n{\n\t\"appid\"\t\t\"1145360\"\n\t\"name\"\t\t\"Hades\"\n\t\"StateFlags\"\t\t\"4\"\n}";
        let updating = installed
            .replace("\"4\"", "\"1026\"")
            .replace("1145360", "620");
        let redist = installed.replace("1145360", "228980");
        let game = steam_game(installed).unwrap();
        assert_eq!(game.title, "Hades");
        assert_eq!(game.path, PathBuf::from("steam://rungameid/1145360"));
        assert_eq!(game.thumb_name, "1145360");
        // 1026 = 2 | 1024: not fully installed.
        assert!(steam_game(&updating).is_none());
        assert!(steam_game(&redist).is_none());
    }

    #[test]
    fn reads_steam_library_folders() {
        let root = std::env::temp_dir().join(format!("lumen-steam-{}", std::process::id()));
        let lib = root.join("lib2");
        fs::create_dir_all(root.join("steamapps")).unwrap();
        fs::create_dir_all(lib.join("steamapps")).unwrap();
        fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n}}",
                lib.display()
            ),
        )
        .unwrap();
        fs::write(
            lib.join("steamapps/appmanifest_620.acf"),
            "\"appid\" \"620\"\n\"name\" \"Portal 2\"\n\"StateFlags\" \"4\"",
        )
        .unwrap();
        let games = scan_steam(&root);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].system_id, STEAM);
    }

    #[test]
    fn reads_epic_manifests() {
        let dir = std::env::temp_dir().join(format!("lumen-epic-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("a.item"),
            r#"{"DisplayName":"Alan Wake 2","AppName":"Dill","CatalogNamespace":"ns","CatalogItemId":"item","bIsIncompleteInstall":false,"AppCategories":["public","games","applications"]}"#,
        )
        .unwrap();
        fs::write(
            dir.join("dlc.item"),
            r#"{"DisplayName":"Some DLC","AppName":"x","CatalogNamespace":"ns","CatalogItemId":"i","AppCategories":["addons"]}"#,
        )
        .unwrap();
        let games = scan_epic(&dir);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Alan Wake 2");
        assert!(games[0].path.to_string_lossy().contains("ns%3Aitem%3ADill"));
    }
}
