//! Game facts (year, developer, genre, players, description) from the
//! libretro database: free, no API key, and keyed by the same No-Intro /
//! Redump names as the box art. Fetched once per system and cached on disk.

use crate::library::{clean_title, Game};
use crate::manifest::System;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::Path,
    sync::OnceLock,
    time::{Duration, SystemTime},
};

const BASE: &str = "https://raw.githubusercontent.com/libretro/libretro-database/master/metadat";
/// Field files to merge. Some systems keep everything in `developer`.
const FIELDS: &[&str] = &["releaseyear", "developer", "publisher", "genre", "maxusers"];
const REFRESH_AFTER: Duration = Duration::from_secs(30 * 24 * 3600);

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    pub year: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genre: Option<String>,
    pub players: Option<String>,
    pub description: Option<String>,
}

pub type SystemInfo = HashMap<String, GameInfo>;

/// Loads a system's database from the disk cache, refreshing it when stale.
/// Network failures fall back to whatever is cached.
pub async fn system_info(system: &System, cache_dir: &Path) -> SystemInfo {
    let path = cache_dir.join(format!("{}.json", system.id));
    let cached: Option<SystemInfo> = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());
    let fresh = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < REFRESH_AFTER);
    if let (Some(info), true) = (&cached, fresh) {
        return info.clone();
    }

    let mut info = SystemInfo::new();
    let mut fetched = false;
    for field in FIELDS {
        let url = format!("{BASE}/{field}/{}.dat", urlencode(&system.thumbnails));
        let Ok(response) = crate::installer::http().get(&url).send().await else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        if let Ok(text) = response.text().await {
            fetched = true;
            merge(&mut info, &text);
        }
    }
    if !fetched {
        return cached.unwrap_or_default();
    }
    let _ = std::fs::create_dir_all(cache_dir);
    if let Ok(bytes) = serde_json::to_vec(&info) {
        let _ = std::fs::write(&path, bytes);
    }
    info
}

/// Finds a game by its file name, then by its cleaned title.
pub fn lookup<'a>(info: &'a SystemInfo, game: &Game) -> Option<&'a GameInfo> {
    info.get(&game.thumb_name).or_else(|| {
        let wanted = game.title.to_lowercase();
        info.iter()
            .find(|(name, _)| clean_title(name).to_lowercase() == wanted)
            .map(|(_, v)| v)
    })
}

/// Parses clrmamepro `game ( … )` blocks and merges their fields into `info`.
fn merge(info: &mut SystemInfo, dat: &str) {
    static FIELD: OnceLock<Regex> = OnceLock::new();
    let field = FIELD.get_or_init(|| Regex::new(r#"^\s*([a-z_]+)\s+"(.*)"\s*$"#).unwrap());

    let mut key: Option<String> = None;
    let mut values: Vec<(String, String)> = Vec::new();
    let mut in_game = false;
    for line in dat.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("game (") {
            in_game = true;
            key = None;
            values.clear();
        } else if trimmed == ")" && in_game {
            in_game = false;
            if let Some(name) = key.take() {
                let entry = info.entry(name).or_default();
                for (k, v) in values.drain(..) {
                    let slot = match k.as_str() {
                        "releaseyear" => &mut entry.year,
                        "developer" => &mut entry.developer,
                        "publisher" => &mut entry.publisher,
                        "genre" => &mut entry.genre,
                        "maxusers" => &mut entry.players,
                        "description" => &mut entry.description,
                        _ => continue,
                    };
                    slot.get_or_insert(v);
                }
            }
        } else if in_game {
            if let Some(c) = field.captures(line) {
                let (k, v) = (c[1].to_string(), c[2].replace("\\\"", "\""));
                if k == "name" || k == "comment" {
                    key.get_or_insert(v);
                } else {
                    values.push((k, v));
                }
            }
        }
    }
}

fn urlencode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_fields_from_both_dat_styles() {
        let mut info = SystemInfo::new();
        merge(
            &mut info,
            "clrmamepro (\n\tname \"x\"\n)\n\ngame (\n\tcomment \"Chrono Trigger (USA)\"\n\treleaseyear \"1995\"\n\trom ( crc 2D206BF7 )\n)\n",
        );
        merge(
            &mut info,
            "game (\n\tname \"Chrono Trigger (USA)\"\n\tdeveloper \"Square\"\n\tdescription \"A \\\"time\\\" travel RPG\"\n)\n",
        );
        let ct = &info["Chrono Trigger (USA)"];
        assert_eq!(ct.year.as_deref(), Some("1995"));
        assert_eq!(ct.developer.as_deref(), Some("Square"));
        assert_eq!(ct.description.as_deref(), Some("A \"time\" travel RPG"));
        assert!(!info.contains_key("x"), "header block is not a game");
    }

    #[test]
    fn encodes_system_names_for_urls() {
        assert_eq!(
            urlencode("Sony - PlayStation 2"),
            "Sony%20-%20PlayStation%202"
        );
    }
}
