//! Games the user can install but hasn't: owned Steam games (Steam Web API,
//! with the user's own key and the account signed in to the Steam client)
//! and the PC Game Pass catalog (public, no sign-in). They show in their own
//! tabs; picking one opens the store's install page — Steam or the Xbox app
//! does the download, and the game turns into an installed one on the next
//! scan with the same id, so favorites and collections carry over.
//!
//! Owned Xbox games other than Game Pass need a Microsoft account sign-in,
//! which Microsoft doesn't open to third-party apps; they aren't listed.

use crate::library::Game;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::Path,
    time::{Duration, SystemTime},
};

pub const GAMEPASS: &str = "gamepass";
/// "All PC Games" list of the PC Game Pass catalog.
const GAMEPASS_LIST: &str =
    "https://catalog.gamepass.com/sigls/v2?id=fdd9e2a7-0fee-49f6-ad69-4354098401ff&language=en-us&market=US";
const GAMEPASS_REFRESH: Duration = Duration::from_secs(24 * 3600);

/// The SteamID64 of the account signed in to the Steam client (the most
/// recent one in `config/loginusers.vdf`).
pub fn steam_user(steam_root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(steam_root.join("config").join("loginusers.vdf")).ok()?;
    parse_login_users(&text)
}

fn parse_login_users(text: &str) -> Option<String> {
    let block = Regex::new(r#""(\d{17})"\s*\{([^}]*)\}"#).ok()?;
    let most_recent = Regex::new(r#"(?i)"MostRecent"\s*"1""#).ok()?;
    let users: Vec<(String, bool)> = block
        .captures_iter(text)
        .map(|c| (c[1].to_string(), most_recent.is_match(&c[2])))
        .collect();
    users
        .iter()
        .find(|(_, recent)| *recent)
        .or_else(|| users.first())
        .map(|(id, _)| id.clone())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Owned {
    pub id: String,
    pub name: String,
}

/// Owned games from `IPlayerService/GetOwnedGames`. An empty list with a
/// valid key usually means the profile's game details are private.
pub async fn steam_owned(key: &str, steam_id: &str) -> anyhow::Result<Vec<Owned>> {
    #[derive(Deserialize)]
    struct Body {
        response: Response,
    }
    #[derive(Deserialize, Default)]
    struct Response {
        #[serde(default)]
        games: Vec<Raw>,
    }
    #[derive(Deserialize)]
    struct Raw {
        appid: u64,
        #[serde(default)]
        name: String,
    }
    let url = format!(
        "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/?key={key}&steamid={steam_id}&include_appinfo=1&include_played_free_games=1"
    );
    let response = crate::installer::http().get(&url).send().await?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED
        || response.status() == reqwest::StatusCode::FORBIDDEN
    {
        anyhow::bail!("Steam anahtarı geçersiz");
    }
    let body: Body = response
        .error_for_status()?
        .json()
        .await
        .map_err(|_| anyhow::anyhow!("Steam'den yanıt alınamadı"))?;
    Ok(body
        .response
        .games
        .into_iter()
        .filter(|g| !g.name.is_empty())
        .map(|g| Owned {
            id: g.appid.to_string(),
            name: g.name,
        })
        .collect())
}

/// Owned games that aren't installed, as library entries that install.
pub fn steam_uninstalled(owned: &[Owned], installed: &HashSet<String>) -> Vec<Game> {
    owned
        .iter()
        .filter(|g| !installed.contains(&g.id))
        .map(|g| {
            let mut game = crate::library::make_virtual_game(
                crate::pc::STEAM,
                &format!("{}:{}", crate::pc::STEAM, g.id),
                &g.name,
                format!("steam://install/{}", g.id),
            );
            // Steam art is fetched by app id, installed or not.
            game.thumb_name = g.id.clone();
            game.not_installed = true;
            game
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogGame {
    pub id: String,
    pub title: String,
    pub poster: Option<String>,
}

/// The PC Game Pass catalog with titles and posters, cached for a day.
pub async fn gamepass(cache: &Path) -> Vec<CatalogGame> {
    let cached: Option<Vec<CatalogGame>> = std::fs::read(cache)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());
    let fresh = std::fs::metadata(cache)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < GAMEPASS_REFRESH);
    if let (Some(list), true) = (&cached, fresh) {
        return list.clone();
    }
    match fetch_gamepass().await {
        Some(list) if !list.is_empty() => {
            if let Ok(bytes) = serde_json::to_vec(&list) {
                let _ = std::fs::write(cache, bytes);
            }
            list
        }
        _ => cached.unwrap_or_default(),
    }
}

async fn fetch_gamepass() -> Option<Vec<CatalogGame>> {
    let list: Vec<serde_json::Value> = crate::installer::http()
        .get(GAMEPASS_LIST)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    // The first entry describes the list itself.
    let ids: Vec<String> = list
        .iter()
        .filter_map(|e| e["id"].as_str().map(String::from))
        .collect();
    let mut games = Vec::new();
    for chunk in ids.chunks(20) {
        let url = format!(
            "https://displaycatalog.mp.microsoft.com/v7.0/products?bigIds={}&market=US&languages=en-us",
            chunk.join(",")
        );
        let Ok(response) = crate::installer::http().get(&url).send().await else {
            continue;
        };
        let Ok(body) = response.json::<serde_json::Value>().await else {
            continue;
        };
        for product in body["Products"].as_array().into_iter().flatten() {
            let (Some(id), Some(title)) = (
                product["ProductId"].as_str(),
                product["LocalizedProperties"][0]["ProductTitle"].as_str(),
            ) else {
                continue;
            };
            let images = product["LocalizedProperties"][0]["Images"].as_array();
            let pick = |purpose: &str| {
                images
                    .into_iter()
                    .flatten()
                    .find(|im| im["ImagePurpose"] == purpose)
                    .and_then(|im| im["Uri"].as_str())
                    .map(|uri| {
                        if uri.starts_with("//") {
                            format!("https:{uri}")
                        } else {
                            uri.to_string()
                        }
                    })
            };
            games.push(CatalogGame {
                id: id.to_string(),
                title: title.to_string(),
                poster: pick("Poster").or_else(|| pick("BoxArt")),
            });
        }
    }
    games.sort_by_key(|g| g.title.to_lowercase());
    Some(games)
}

/// Catalog games not installed through the Xbox app, opening their store page.
pub fn gamepass_uninstalled(catalog: &[CatalogGame], installed: &HashSet<String>) -> Vec<Game> {
    catalog
        .iter()
        .filter(|g| !installed.contains(&g.id.to_uppercase()))
        .map(|g| {
            let mut game = crate::library::make_virtual_game(
                GAMEPASS,
                &format!("{GAMEPASS}:{}", g.id),
                &g.title,
                format!("ms-windows-store://pdp/?productid={}", g.id),
            );
            game.thumb_name = g.poster.clone().unwrap_or_default();
            game.not_installed = true;
            game
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_signed_in_steam_account() {
        let vdf = r#""users"
{
	"76561198000000001"
	{
		"AccountName"		"old"
		"MostRecent"		"0"
	}
	"76561198000000002"
	{
		"AccountName"		"me"
		"MostRecent"		"1"
	}
}"#;
        assert_eq!(parse_login_users(vdf).as_deref(), Some("76561198000000002"));
        assert_eq!(parse_login_users("nothing"), None);
    }

    #[test]
    fn lists_only_what_is_not_installed() {
        let owned = vec![
            Owned {
                id: "620".into(),
                name: "Portal 2".into(),
            },
            Owned {
                id: "400".into(),
                name: "Portal".into(),
            },
        ];
        let installed = HashSet::from(["400".to_string()]);
        let games = steam_uninstalled(&owned, &installed);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Portal 2");
        assert!(games[0].not_installed);
        assert_eq!(games[0].path.to_string_lossy(), "steam://install/620");
        // Same id an installed copy gets, so user data follows the game.
        let installed_copy =
            crate::library::make_virtual_game("steam", "steam:620", "Portal 2", String::new());
        assert_eq!(games[0].id, installed_copy.id);

        let catalog = vec![
            CatalogGame {
                id: "9NPDN9R45JX4".into(),
                title: "1000xRESIST".into(),
                poster: None,
            },
            CatalogGame {
                id: "9P8LR42PTRGJ".into(),
                title: "33 Immortals".into(),
                poster: Some("https://x/p.jpg".into()),
            },
        ];
        let games = gamepass_uninstalled(&catalog, &HashSet::from(["9NPDN9R45JX4".to_string()]));
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].thumb_name, "https://x/p.jpg");
        assert!(games[0]
            .path
            .to_string_lossy()
            .starts_with("ms-windows-store://pdp/?productid="));
    }
}
