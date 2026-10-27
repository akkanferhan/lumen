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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BiosStatus {
    pub label: String,
    pub optional: bool,
    pub found: bool,
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
    let mut referenced = HashSet::new();
    for file in files {
        let ext = extension(file);
        if !matches!(ext.as_deref(), Some("cue" | "m3u" | "gdi")) {
            continue;
        }
        let (Ok(text), Some(dir)) = (fs::read_to_string(file), file.parent()) else {
            continue;
        };
        for line in text.lines().map(str::trim) {
            let name = match ext.as_deref() {
                Some("cue") => line
                    .strip_prefix("FILE ")
                    .and_then(|rest| rest.split('"').nth(1)),
                Some("m3u") if !line.is_empty() && !line.starts_with('#') => Some(line),
                _ => None,
            };
            if let Some(name) = name {
                referenced.insert(dir.join(name));
            }
        }
    }
    referenced
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
        id: format!("{:016x}", fnv1a(path.to_string_lossy().as_bytes())),
        system_id: system.id.clone(),
        title: clean_title(&stem),
        thumb_name: stem.replace(
            ['&', '*', '/', ':', '`', '<', '>', '?', '\\', '|', '"'],
            "_",
        ),
        path: path.to_path_buf(),
    }
}

/// Title from a PS3 (`PS3_GAME/PARAM.SFO`) or PS4 (`sce_sys/param.sfo`) folder dump.
fn folder_title(dir: &Path) -> Option<String> {
    ["PS3_GAME/PARAM.SFO", "sce_sys/param.sfo"]
        .iter()
        .find_map(|rel| fs::read(dir.join(rel)).ok())
        .and_then(|bytes| sfo_string(&bytes, "TITLE"))
        .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|t| !t.is_empty())
}

/// Reads a UTF-8 value from a PlayStation SFO (PSF) key/value file.
fn sfo_string(data: &[u8], wanted: &str) -> Option<String> {
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
        .map(|req| BiosStatus {
            label: req.label.clone(),
            optional: req.optional,
            found: bios_dir
                .is_some_and(|dir| req.any_of.iter().any(|p| find_file(dir, p).is_some())),
        })
        .collect()
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
    use super::clean_title;

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
