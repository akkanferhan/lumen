//! Box art from SteamGridDB for games libretro has none for (homebrew,
//! hacks, translations, loosely named files). Needs the user's own free API
//! key; each cover is downloaded once into `art/covers/<game id>.<ext>` and
//! shown from disk. Games SteamGridDB doesn't know are remembered, so a scan
//! doesn't ask again for a month.

use crate::library::{clean_title, Game};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const API: &str = "https://www.steamgriddb.com/api/v2";
/// Portrait covers, the shape of every other cover in Lumen.
const DIMENSIONS: &str = "600x900,342x482,660x930";
/// New lookups per scan, so a big library fills in over a few starts
/// instead of holding one scan up.
const PER_SCAN: usize = 40;
const RETRY_AFTER: Duration = Duration::from_secs(30 * 24 * 3600);

pub fn covers_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("art").join("covers")
}

/// The downloaded cover of a game, if there is one.
fn cached(dir: &Path, game_id: &str) -> Option<PathBuf> {
    ["png", "jpg", "webp"]
        .iter()
        .map(|ext| dir.join(format!("{game_id}.{ext}")))
        .find(|p| p.is_file())
}

/// Checks a key with one search; SteamGridDB answers 401 to a bad key.
pub async fn check_key(key: &str) -> anyhow::Result<()> {
    let response = request(key, &["search", "autocomplete", "mario"]).await?;
    anyhow::ensure!(
        response.status() != reqwest::StatusCode::UNAUTHORIZED,
        "SteamGridDB anahtarı kabul etmedi; anahtarı kontrol et"
    );
    anyhow::ensure!(
        response.status().is_success(),
        "SteamGridDB yanıt vermedi ({})",
        response.status()
    );
    Ok(())
}

async fn request(key: &str, segments: &[&str]) -> anyhow::Result<reqwest::Response> {
    let mut url = reqwest::Url::parse(API)?;
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("bad url"))?
        .extend(segments);
    if segments.first() == Some(&"grids") {
        url.query_pairs_mut()
            .append_pair("dimensions", DIMENSIONS)
            .append_pair("types", "static")
            .append_pair("nsfw", "false")
            .append_pair("humor", "false");
    }
    Ok(crate::installer::http()
        .get(url)
        .bearer_auth(key)
        .timeout(Duration::from_secs(15))
        .send()
        .await?)
}

async fn json(key: &str, segments: &[&str]) -> anyhow::Result<serde_json::Value> {
    Ok(request(key, segments)
        .await?
        .error_for_status()?
        .json()
        .await?)
}

/// Searches the title and downloads the first portrait cover.
async fn fetch(
    key: &str,
    title: &str,
    dir: &Path,
    game_id: &str,
) -> anyhow::Result<Option<PathBuf>> {
    let found = json(key, &["search", "autocomplete", title]).await?;
    let Some(id) = found["data"][0]["id"].as_u64() else {
        return Ok(None);
    };
    let grids = json(key, &["grids", "game", &id.to_string()]).await?;
    let Some(url) = grids["data"][0]["url"].as_str() else {
        return Ok(None);
    };
    let ext = url
        .rsplit('.')
        .next()
        .filter(|e| matches!(*e, "png" | "jpg" | "webp"))
        .unwrap_or("png");
    let bytes = crate::installer::http()
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{game_id}.{ext}"));
    std::fs::write(&path, &bytes)?;
    Ok(Some(path))
}

/// Gives the games at `missing` (no libretro art) their SteamGridDB cover:
/// from disk when downloaded before, else looked up with `key`.
pub async fn fill(games: &mut [Game], missing: &[usize], key: Option<&str>, data_dir: &Path) {
    let dir = covers_dir(data_dir);
    let misses_path = dir.join("not-found.json");
    let mut misses: HashMap<String, u64> = std::fs::read(&misses_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut lookups = 0;
    let mut changed = false;

    for &i in missing {
        let game = &mut games[i];
        if let Some(path) = cached(&dir, &game.id) {
            game.cover = Some(path);
            continue;
        }
        let Some(key) = key else { continue };
        let recently_missed = misses
            .get(&game.id)
            .is_some_and(|&at| now.saturating_sub(at) < RETRY_AFTER.as_secs());
        if recently_missed || lookups >= PER_SCAN {
            continue;
        }
        lookups += 1;
        match fetch(key, &clean_title(&game.title), &dir, &game.id).await {
            Ok(Some(path)) => game.cover = Some(path),
            Ok(None) => {
                misses.insert(game.id.clone(), now);
                changed = true;
            }
            // Offline or rate limited: try again next scan.
            Err(_) => {}
        }
    }
    if changed {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            &misses_path,
            serde_json::to_vec(&misses).unwrap_or_default(),
        );
    }
}
