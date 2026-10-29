//! Owned Steam games that aren't installed (Steam Web API, with the user's
//! own key and the account signed in to the Steam client). They show in the
//! Steam tab; picking one opens Steam's install dialog, and the game turns
//! into an installed one on the next scan with the same id, so favorites
//! and collections carry over.
//!
//! Only the user's own games are listed: owned Xbox and Epic games need
//! sign-ins those stores don't open to third-party apps, so those stores
//! show what's installed.

use crate::library::Game;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

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
    }
}
