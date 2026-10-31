//! ROM discovery: maps `<root>/<system folder>/...` files to games.

use crate::manifest::{manifest, System};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub system_id: String,
    pub title: String,
    /// File stem in the form thumbnails.libretro.com expects.
    pub thumb_name: String,
    pub path: PathBuf,
    /// Owned in a store (or in its catalog) but not installed: `path` is
    /// the store's install link.
    #[serde(default)]
    pub not_installed: bool,
    /// Downloaded cover (SteamGridDB) for games libretro has no art for.
    #[serde(default)]
    pub cover: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BiosStatus {
    pub label: String,
    pub optional: bool,
    /// A usable file is present (a wrong-size file doesn't count).
    pub found: bool,
    /// What the check made of the best file present, if any.
    pub verdict: Option<crate::bios_check::Verdict>,
    /// That file's name.
    pub file: Option<String>,
}

pub fn scan(root: &Path) -> Vec<Game> {
    let mut games = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return games;
    };

    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if let Some(system) = manifest()
            .systems
            .iter()
            .find(|s| s.folders.contains(&name))
        {
            scan_system(system, &dir, &mut games);
        }
    }

    games.sort_by_key(|g| g.title.to_lowercase());
    games
}

/// Joins loose multi-disc sets into playlists in every system folder that
/// takes them (see `discs`); returns what was written.
pub fn make_disc_playlists(root: &Path) -> Vec<crate::discs::Created> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut created = Vec::new();
    for entry in entries.flatten().filter(|e| e.path().is_dir()) {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if let Some(system) = manifest()
            .systems
            .iter()
            .find(|s| s.folders.contains(&name) && s.extensions.iter().any(|e| e == "m3u"))
        {
            created.extend(crate::discs::make_playlists(
                &entry.path(),
                &system.extensions,
            ));
        }
    }
    created
}

/// A game's id: stable as long as its file stays where it is.
pub fn game_id(path: &Path) -> String {
    format!("{:016x}", fnv1a(path.to_string_lossy().as_bytes()))
}

fn scan_system(system: &System, dir: &Path, out: &mut Vec<Game>) {
    let mut files = Vec::new();
    let mut walker = WalkDir::new(dir).min_depth(1).max_depth(4).into_iter();

    loop {
        let entry = match walker.next() {
            None => break,
            Some(Err(_)) => continue,
            Some(Ok(e)) => e,
        };
        let path = entry.path();

        if entry.file_type().is_dir() {
            if is_addon_dir(path) {
                walker.skip_current_dir();
                continue;
            }
            if let Some(marker) = &system.folder_marker {
                if path.join(marker).is_dir() {
                    out.push(make_game(system, path));
                    walker.skip_current_dir();
                }
            }
            continue;
        }

        if extension(path).is_some_and(|ext| system.extensions.contains(&ext)) {
            files.push(path.to_path_buf());
        }
    }

    // Tracks referenced by a .cue or discs listed in a .m3u are not separate games.
    let referenced = referenced_files(&files);
    out.extend(
        files
            .iter()
            .filter(|f| !referenced.contains(*f))
            .map(|f| make_game(system, f)),
    );
}

/// PS4 updates/patches sit next to the game (`CUSA00552-UPDATE`) and are read by the emulator.
fn is_addon_dir(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    name.ends_with("-update") || name.ends_with("-patch")
}

fn referenced_files(files: &[PathBuf]) -> HashSet<PathBuf> {
    files.iter().flat_map(|f| references(f)).collect()
}

/// Files a disc descriptor points at: `.cue` tracks, `.m3u` discs, `.gdi` tracks.
pub fn references(file: &Path) -> Vec<PathBuf> {
    let ext = extension(file);
    if !matches!(ext.as_deref(), Some("cue" | "m3u" | "gdi")) {
        return Vec::new();
    }
    let (Ok(text), Some(dir)) = (fs::read_to_string(file), file.parent()) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter_map(|line| match ext.as_deref() {
            Some("cue") => line
                .strip_prefix("FILE ")
                .and_then(|rest| rest.split('"').nth(1)),
            Some("m3u") if !line.is_empty() && !line.starts_with('#') => Some(line),
            // "<track> <lba> <type> <sector size> <file> <offset>"; the name may be quoted.
            Some("gdi") => line
                .split('"')
                .nth(1)
                .or_else(|| line.split_whitespace().nth(4)),
            _ => None,
        })
        .map(|name| dir.join(name))
        .collect()
}

/// Every file that makes up a game on disk: the file itself plus whatever it
/// references, followed through `.m3u` → `.cue` → tracks.
pub fn game_files(path: &Path) -> Vec<PathBuf> {
    let mut files = vec![path.to_path_buf()];
    let mut i = 0;
    while i < files.len() {
        for referenced in references(&files[i]) {
            if referenced.exists() && !files.contains(&referenced) {
                files.push(referenced);
            }
        }
        i += 1;
    }
    files
}

/// A game that isn't a file on disk (Steam / Epic): `path` holds its launch URI.
pub fn make_virtual_game(system_id: &str, id_seed: &str, title: &str, uri: String) -> Game {
    Game {
        id: format!("{:016x}", fnv1a(id_seed.as_bytes())),
        system_id: system_id.to_string(),
        title: title.trim().to_string(),
        thumb_name: String::new(),
        path: PathBuf::from(uri),
        not_installed: false,
        cover: None,
    }
}

fn make_game(system: &System, path: &Path) -> Game {
    // Folder dumps are named by product code (CUSA00552), so prefer the real title.
    let sfo_title = path.is_dir().then(|| folder_title(path)).flatten();
    let stem = sfo_title
        .clone()
        .or_else(|| {
            if path.is_dir() {
                path.file_name()
            } else {
                path.file_stem()
            }
            .map(|s| s.to_string_lossy().into_owned())
        })
        .unwrap_or_default();

    Game {
        id: game_id(path),
        system_id: system.id.clone(),
        title: clean_title(&stem),
        thumb_name: stem.replace(
            ['&', '*', '/', ':', '`', '<', '>', '?', '\\', '|', '"'],
            "_",
        ),
        path: path.to_path_buf(),
        not_installed: false,
        cover: None,
    }
}

/// Title from a PS3 (`PS3_GAME/PARAM.SFO`), PS4 (`sce_sys/param.sfo`) or
/// PS5 (`sce_sys/param.json`) folder dump.
fn folder_title(dir: &Path) -> Option<String> {
    ["PS3_GAME/PARAM.SFO", "sce_sys/param.sfo"]
        .iter()
        .find_map(|rel| fs::read(dir.join(rel)).ok())
        .and_then(|bytes| sfo_string(&bytes, "TITLE"))
        .or_else(|| {
            fs::read(dir.join("sce_sys/param.json"))
                .ok()
                .and_then(|bytes| param_json_title(&bytes))
        })
        .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|t| !t.is_empty())
}

/// `localizedParameters.<defaultLanguage>.titleName` from a PS5 `param.json`,
/// falling back to any language that has a title.
fn param_json_title(data: &[u8]) -> Option<String> {
    let root: serde_json::Value = serde_json::from_slice(data).ok()?;
    let localized = root.get("localizedParameters")?.as_object()?;
    let title = |lang: &serde_json::Value| lang.get("titleName")?.as_str().map(str::to_owned);
    localized
        .get("defaultLanguage")
        .and_then(|d| d.as_str())
        .and_then(|d| localized.get(d))
        .and_then(title)
        .or_else(|| localized.values().find_map(title))
}

/// Reads a UTF-8 value from a PlayStation SFO (PSF) key/value file.
pub(crate) fn sfo_string(data: &[u8], wanted: &str) -> Option<String> {
    let u16_at = |o: usize| {
        data.get(o..o + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
    };
    let u32_at = |o: usize| {
        data.get(o..o + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
    };

    if data.get(0..4)? != b"\0PSF" {
        return None;
    }
    let (key_table, data_table, count) = (u32_at(8)?, u32_at(12)?, u32_at(16)?);
    (0..count.min(256)).find_map(|i| {
        let entry = 20 + i * 16;
        let key_start = key_table + u16_at(entry)?;
        let key_end = key_start + data.get(key_start..)?.iter().position(|&b| b == 0)?;
        if &data[key_start..key_end] != wanted.as_bytes() {
            return None;
        }
        let len = u32_at(entry + 4)?;
        let start = data_table + u32_at(entry + 12)?;
        let value = data.get(start..start + len)?;
        let value = value.split(|&b| b == 0).next()?;
        Some(String::from_utf8_lossy(value).into_owned())
    })
}

/// "Legend of Zelda, The - A Link to the Past (USA) [!]" → "The Legend of Zelda - A Link to the Past"
pub fn clean_title(stem: &str) -> String {
    static TAGS: OnceLock<Regex> = OnceLock::new();
    static ARTICLE: OnceLock<Regex> = OnceLock::new();
    let tags = TAGS.get_or_init(|| Regex::new(r"\s*[\(\[][^\)\]]*[\)\]]").unwrap());
    let article = ARTICLE.get_or_init(|| Regex::new(r"^(.+?), (The|A|An)( - .*)?$").unwrap());

    let base = tags.replace_all(stem, "").replace('_', " ");
    let base = base.split_whitespace().collect::<Vec<_>>().join(" ");
    let title = article.replace(&base, "$2 $1$3").into_owned();
    if title.is_empty() {
        stem.to_string()
    } else {
        title
    }
}

/// The emulator/core that will run `system` with the BIOS files present.
pub struct Effective<'a> {
    pub emulator: &'a str,
    pub core: Option<&'a str>,
    /// True when running on the fallback because the user's BIOS is missing.
    pub fallback: bool,
}

pub fn effective<'a>(system: &'a System, bios_dir: Option<&Path>) -> Effective<'a> {
    let missing = bios_status(system, bios_dir)
        .iter()
        .any(|b| !b.found && !b.optional);
    match (&system.fallback, missing) {
        (Some(runner), true) => Effective {
            emulator: &runner.emulator,
            core: runner.core.as_deref(),
            fallback: true,
        },
        _ => Effective {
            emulator: &system.emulator,
            core: system.core.as_deref(),
            fallback: false,
        },
    }
}

pub fn bios_status(system: &System, bios_dir: Option<&Path>) -> Vec<BiosStatus> {
    system
        .bios
        .iter()
        .map(|req| {
            let candidates: Vec<(PathBuf, String)> = bios_dir
                .map(|dir| {
                    req.any_of
                        .iter()
                        .flat_map(|p| find_files(dir, p).into_iter().map(move |f| (f, p.clone())))
                        .collect()
                })
                .unwrap_or_default();
            let best = crate::bios_check::best(&candidates);
            BiosStatus {
                label: req.label.clone(),
                optional: req.optional,
                found: best.is_some_and(|(_, v)| v != crate::bios_check::Verdict::WrongSize),
                verdict: best.map(|(_, v)| v),
                file: best
                    .and_then(|(p, _)| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned()),
            }
        })
        .collect()
}

/// Every file matching a case-insensitive glob relative to `dir`.
pub fn find_files(dir: &Path, pattern: &str) -> Vec<PathBuf> {
    let full = format!(
        "{}/{}",
        glob::Pattern::escape(&dir.to_string_lossy()),
        pattern
    );
    let options = glob::MatchOptions {
        case_sensitive: false,
        ..Default::default()
    };
    glob::glob_with(&full, options)
        .map(|paths| paths.flatten().filter(|p| p.is_file()).collect())
        .unwrap_or_default()
}

/// Case-insensitive glob lookup relative to `dir`.
pub fn find_file(dir: &Path, pattern: &str) -> Option<PathBuf> {
    let full = format!(
        "{}/{}",
        glob::Pattern::escape(&dir.to_string_lossy()),
        pattern
    );
    let options = glob::MatchOptions {
        case_sensitive: false,
        ..Default::default()
    };
    glob::glob_with(&full, options).ok()?.flatten().next()
}

fn extension(path: &Path) -> Option<String> {
    path.extension().map(|e| e.to_string_lossy().to_lowercase())
}

/// Stable across Rust versions, unlike `DefaultHasher`; game ids key user data.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, b| {
        (hash ^ *b as u64).wrapping_mul(0x100000001b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_sfo_title() {
        // Minimal PSF: one TITLE entry.
        let key = b"TITLE\0";
        let value = b"Bloodborne\0";
        let key_table = 20 + 16;
        let data_table = key_table + key.len();
        let mut sfo = Vec::new();
        sfo.extend_from_slice(b"\0PSF");
        sfo.extend_from_slice(&0x0101u32.to_le_bytes());
        sfo.extend_from_slice(&(key_table as u32).to_le_bytes());
        sfo.extend_from_slice(&(data_table as u32).to_le_bytes());
        sfo.extend_from_slice(&1u32.to_le_bytes());
        sfo.extend_from_slice(&0u16.to_le_bytes()); // key offset
        sfo.extend_from_slice(&0x0204u16.to_le_bytes()); // utf8 format
        sfo.extend_from_slice(&(value.len() as u32).to_le_bytes());
        sfo.extend_from_slice(&(value.len() as u32).to_le_bytes());
        sfo.extend_from_slice(&0u32.to_le_bytes()); // data offset
        sfo.extend_from_slice(key);
        sfo.extend_from_slice(value);

        assert_eq!(
            super::sfo_string(&sfo, "TITLE").as_deref(),
            Some("Bloodborne")
        );
        assert_eq!(super::sfo_string(&sfo, "TITLE_ID"), None);
        assert_eq!(super::sfo_string(b"garbage", "TITLE"), None);
    }

    #[test]
    fn reads_ps5_param_json_title() {
        let json = br#"{"localizedParameters": {"defaultLanguage": "en-US",
            "ja-JP": {"titleName": "Japanese"}, "en-US": {"titleName": "Demo  Game"}}}"#;
        assert_eq!(super::param_json_title(json).as_deref(), Some("Demo  Game"));
        let other = br#"{"localizedParameters": {"fr-FR": {"titleName": "Jeu"}}}"#;
        assert_eq!(super::param_json_title(other).as_deref(), Some("Jeu"));
        assert_eq!(super::param_json_title(b"not json"), None);
    }

    #[test]
    fn game_files_follow_playlists_and_tracks() {
        let dir = std::env::temp_dir().join(format!("lumen-files-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for f in ["d1.bin", "d2.bin", "t1.bin", "t2.raw"] {
            fs::write(dir.join(f), b"x").unwrap();
        }
        fs::write(dir.join("d1.cue"), "FILE \"d1.bin\" BINARY\n").unwrap();
        fs::write(dir.join("d2.cue"), "FILE \"d2.bin\" BINARY\n").unwrap();
        fs::write(dir.join("game.m3u"), "d1.cue\nd2.cue\n").unwrap();
        fs::write(
            dir.join("dc.gdi"),
            "2\n1 0 4 2352 t1.bin 0\n2 600 0 2352 \"t2.raw\" 0\n",
        )
        .unwrap();

        let mut m3u: Vec<_> = game_files(&dir.join("game.m3u"))
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        m3u.sort();
        assert_eq!(m3u, ["d1.bin", "d1.cue", "d2.bin", "d2.cue", "game.m3u"]);
        assert_eq!(game_files(&dir.join("dc.gdi")).len(), 3);
    }

    #[test]
    fn cleans_no_intro_names() {
        assert_eq!(
            clean_title("Legend of Zelda, The - A Link to the Past (USA)"),
            "The Legend of Zelda - A Link to the Past"
        );
        assert_eq!(
            clean_title("Super Metroid (Japan, USA) (En,Ja) [!]"),
            "Super Metroid"
        );
        assert_eq!(clean_title("Crash_Bandicoot"), "Crash Bandicoot");
    }
}
