//! Matches games to libretro's box art when the file name isn't the exact
//! No-Intro / Redump name ("Spiderman.cue" vs "Spider-Man (USA)"). The art
//! server's own file listing is the index: it covers every game with art,
//! unlike the metadata databases. Listings are cached for a month.

use crate::library::{clean_title, Game};
use crate::manifest::System;
use regex::Regex;
use std::{
    collections::HashSet,
    path::Path,
    sync::OnceLock,
    time::{Duration, SystemTime},
};

const BASE: &str = "https://thumbnails.libretro.com";
const REFRESH_AFTER: Duration = Duration::from_secs(30 * 24 * 3600);

/// Names (without .png) that have box art for `system`. Network failures fall
/// back to the cache, then to nothing.
pub async fn boxart_names(system: &System, cache_dir: &Path) -> Vec<String> {
    let path = cache_dir.join(format!("{}.json", system.id));
    let cached: Option<Vec<String>> = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());
    let fresh = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < REFRESH_AFTER);
    if let (Some(names), true) = (&cached, fresh) {
        return names.clone();
    }

    let url = format!(
        "{BASE}/{}/Named_Boxarts/",
        system.thumbnails.replace(' ', "%20")
    );
    let listing = match crate::installer::http().get(&url).send().await {
        Ok(r) if r.status().is_success() => r.text().await.ok(),
        _ => None,
    };
    let Some(names) = listing
        .map(|html| parse_listing(&html))
        .filter(|n| !n.is_empty())
    else {
        return cached.unwrap_or_default();
    };
    let _ = std::fs::create_dir_all(cache_dir);
    if let Ok(bytes) = serde_json::to_vec(&names) {
        let _ = std::fs::write(&path, bytes);
    }
    names
}

/// `href="Spider-Man%20(USA).png"` → `Spider-Man (USA)`.
fn parse_listing(html: &str) -> Vec<String> {
    static HREF: OnceLock<Regex> = OnceLock::new();
    let href = HREF.get_or_init(|| Regex::new(r#"href="([^"/?]+)\.png""#).unwrap());
    href.captures_iter(html)
        .filter_map(|c| percent_decode(&c[1]))
        .collect()
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Letters and digits of the cleaned title: "Spider-Man (USA)" and
/// "spiderman" both become "spiderman".
fn key(name: &str) -> String {
    clean_title(name)
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The art name for a file stem, or None when nothing fits. Among regional
/// versions, the file's own region wins, then USA, World and Europe.
pub fn best_match<'a>(names: &'a [String], stem: &str) -> Option<&'a String> {
    let wanted = key(stem);
    if wanted.is_empty() {
        return None;
    }
    let candidates: Vec<&String> = names.iter().filter(|n| key(n) == wanted).collect();
    let tags = |s: &str| -> Vec<String> {
        s.split(['(', ')'])
            .skip(1)
            .step_by(2)
            .map(|t| t.trim().to_lowercase())
            .collect()
    };
    let own = tags(stem);
    let score = |name: &String| {
        let theirs = tags(name);
        let shared = own.iter().filter(|t| theirs.contains(t)).count();
        let region = ["usa", "world", "europe"]
            .iter()
            .position(|r| theirs.iter().any(|t| t.split(", ").any(|p| p == *r)))
            .map_or(0, |p| 3 - p);
        // Fewer extra tags (no "Rev 1", "Demo") is the plainer release.
        (shared, region, usize::MAX - theirs.len())
    };
    candidates.into_iter().max_by_key(|n| score(n))
}

/// Points games whose file names have no exact art at the closest art name.
/// Store games keep their catalog art.
/// Returns the games libretro has no box art for at all.
pub async fn resolve(games: &mut [Game], cache_dir: &Path) -> Vec<usize> {
    let mut missing = Vec::new();
    let systems: HashSet<String> = games.iter().map(|g| g.system_id.clone()).collect();
    for system_id in systems {
        let Some(system) = crate::manifest::system(&system_id) else {
            continue;
        };
        let pending: Vec<usize> = games
            .iter()
            .enumerate()
            .filter(|(_, g)| {
                g.system_id == system_id
                    && crate::homebrew::for_game(&g.system_id, &g.path).is_none()
            })
            .map(|(i, _)| i)
            .collect();
        if pending.is_empty() {
            continue;
        }
        let names = boxart_names(system, cache_dir).await;
        // No listing (offline, first start): nothing is known to be missing.
        if names.is_empty() {
            continue;
        }
        let exact: HashSet<&str> = names.iter().map(String::as_str).collect();
        for i in pending {
            let game = &mut games[i];
            if exact.contains(game.thumb_name.as_str()) {
                continue;
            }
            match best_match(&names, &game.thumb_name) {
                Some(name) => game.thumb_name = name.clone(),
                None => missing.push(i),
            }
        }
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_the_art_server_listing() {
        let html = r#"<a href="../">../</a><a href="Spider-Man%20(USA).png">x</a>
            <a href="Miss%20Spider's%20Tea%20Party%20(USA).png">y</a>"#;
        assert_eq!(
            parse_listing(html),
            names(&["Spider-Man (USA)", "Miss Spider's Tea Party (USA)"])
        );
    }

    #[test]
    fn matches_loose_file_names_preferring_the_right_region() {
        let art = names(&[
            "Spider-Man (Europe)",
            "Spider-Man (Japan)",
            "Spider-Man (USA)",
            "Spider-Man 2 - Enter - Electro (USA)",
        ]);
        assert_eq!(best_match(&art, "Spiderman").unwrap(), "Spider-Man (USA)");
        assert_eq!(best_match(&art, "spider man").unwrap(), "Spider-Man (USA)");
        assert_eq!(
            best_match(&art, "Spider-Man (Europe)").unwrap(),
            "Spider-Man (Europe)"
        );
        assert_eq!(
            best_match(&art, "Spider-Man 2 - Enter - Electro").unwrap(),
            "Spider-Man 2 - Enter - Electro (USA)"
        );
        assert_eq!(best_match(&art, "Batman"), None);
    }

    #[test]
    fn plainer_releases_win() {
        let art = names(&[
            "Crash Bandicoot (USA) (Demo)",
            "Crash Bandicoot (USA)",
            "Crash Bandicoot (Europe, Australia)",
        ]);
        assert_eq!(
            best_match(&art, "crash bandicoot").unwrap(),
            "Crash Bandicoot (USA)"
        );
    }
}
