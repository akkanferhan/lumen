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
    let Ok(entries) = fs::read_dir(root) else { return games };

    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if let Some(system) = manifest().systems.iter().find(|s| s.folders.contains(&name)) {
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

fn referenced_files(files: &[PathBuf]) -> HashSet<PathBuf> {
    let mut referenced = HashSet::new();
    for file in files {
        let ext = extension(file);
        if !matches!(ext.as_deref(), Some("cue" | "m3u" | "gdi")) {
            continue;
        }
        let (Ok(text), Some(dir)) = (fs::read_to_string(file), file.parent()) else { continue };
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
    let stem = if path.is_dir() {
        path.file_name()
    } else {
        path.file_stem()
    }
    .map(|s| s.to_string_lossy().into_owned())
    .unwrap_or_default();

    Game {
        id: format!("{:016x}", fnv1a(path.to_string_lossy().as_bytes())),
        system_id: system.id.clone(),
        title: clean_title(&stem),
        thumb_name: stem.replace(['&', '*', '/', ':', '`', '<', '>', '?', '\\', '|', '"'], "_"),
        path: path.to_path_buf(),
    }
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
    if title.is_empty() { stem.to_string() } else { title }
}

pub fn bios_status(system: &System, bios_dir: Option<&Path>) -> Vec<BiosStatus> {
    system
        .bios
        .iter()
        .map(|req| BiosStatus {
            label: req.label.clone(),
            optional: req.optional,
            found: bios_dir.is_some_and(|dir| req.any_of.iter().any(|p| find_file(dir, p).is_some())),
        })
        .collect()
}

/// Case-insensitive glob lookup relative to `dir`.
pub fn find_file(dir: &Path, pattern: &str) -> Option<PathBuf> {
    let full = format!("{}/{}", glob::Pattern::escape(&dir.to_string_lossy()), pattern);
    let options = glob::MatchOptions { case_sensitive: false, ..Default::default() };
    glob::glob_with(&full, options).ok()?.flatten().next()
}

fn extension(path: &Path) -> Option<String> {
    path.extension().map(|e| e.to_string_lossy().to_lowercase())
}

/// Stable across Rust versions, unlike `DefaultHasher`; game ids key user data.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, b| (hash ^ *b as u64).wrapping_mul(0x100000001b3))
}

#[cfg(test)]
mod tests {
    use super::clean_title;

    #[test]
    fn cleans_no_intro_names() {
        assert_eq!(
            clean_title("Legend of Zelda, The - A Link to the Past (USA)"),
            "The Legend of Zelda - A Link to the Past"
        );
        assert_eq!(clean_title("Super Metroid (Japan, USA) (En,Ja) [!]"), "Super Metroid");
        assert_eq!(clean_title("Crash_Bandicoot"), "Crash Bandicoot");
    }
}
