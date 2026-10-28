//! Games installed by the Xbox app (Game Pass and purchases). Modern PC Game
//! Pass titles are GDK packages in a per-drive folder recorded in
//! `<drive>:\.GamingRoot`; each has `Content\MicrosoftGame.config` with its
//! identity, executable id, name and Store id.

use crate::library::Game;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const XBOX_APP: &str = "xboxapp";

/// One installed game, before it becomes a library entry.
#[derive(Debug, Clone, PartialEq)]
pub struct XboxGame {
    pub title: String,
    pub family_name: String,
    pub app_id: String,
    pub store_id: Option<String>,
}

impl XboxGame {
    /// `shell:AppsFolder\<family>!<app>` is how the Xbox app itself starts games.
    pub fn launch_uri(&self) -> String {
        format!(r"shell:AppsFolder\{}!{}", self.family_name, self.app_id)
    }
}

pub fn scan() -> Vec<Game> {
    gaming_roots()
        .iter()
        .flat_map(|root| fs::read_dir(root).into_iter().flatten().flatten())
        .filter_map(|dir| {
            let folder = dir.file_name().to_string_lossy().into_owned();
            let config =
                fs::read_to_string(dir.path().join("Content").join("MicrosoftGame.config")).ok()?;
            parse_config(&config, &folder)
        })
        .map(|g| {
            let mut game = crate::library::make_virtual_game(
                XBOX_APP,
                &format!("xbox:{}", g.family_name),
                &g.title,
                g.launch_uri(),
            );
            // Filled with the Store poster URL by `posters`; empty means no art.
            game.thumb_name = g.store_id.unwrap_or_default();
            game
        })
        .collect()
}

/// Folders the Xbox app installs into, one `.GamingRoot` per drive.
fn gaming_roots() -> Vec<PathBuf> {
    ('C'..='Z')
        .filter_map(|letter| {
            let drive = PathBuf::from(format!("{letter}:\\"));
            let bytes = fs::read(drive.join(".GamingRoot")).ok()?;
            Some(
                parse_gaming_root(&bytes)
                    .into_iter()
                    .map(move |rel| drive.join(rel)),
            )
        })
        .flatten()
        .filter(|p| p.is_dir())
        .collect()
}

/// `RGBX` + u32 count + UTF-16LE NUL-terminated folder paths (relative to the drive).
fn parse_gaming_root(bytes: &[u8]) -> Vec<String> {
    if bytes.len() < 8 || &bytes[..4] != b"RGBX" {
        return Vec::new();
    }
    let units: Vec<u16> = bytes[8..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    String::from_utf16_lossy(&units)
        .split('\0')
        .map(|s| s.trim_start_matches(['\\', '/']).to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_config(xml: &str, folder: &str) -> Option<XboxGame> {
    let attr = |element: &str, name: &str| -> Option<String> {
        let tag = Regex::new(&format!(r"<{element}\b[^>]*>"))
            .ok()?
            .find(xml)?
            .as_str()
            .to_string();
        let value = Regex::new(&format!(r#"\b{name}\s*=\s*"([^"]*)""#))
            .ok()?
            .captures(&tag)?
            .get(1)?
            .as_str()
            .to_string();
        Some(unescape(&value))
    };
    let name = attr("Identity", "Name")?;
    let publisher = attr("Identity", "Publisher")?;
    let app_id = attr("Executable", "Id").unwrap_or_else(|| "Game".into());
    // Localized names come as ms-resource: references; fall back to the folder.
    let title = attr("ShellVisuals", "DefaultDisplayName")
        .filter(|t| !t.is_empty() && !t.starts_with("ms-resource:"))
        .unwrap_or_else(|| folder.to_string());
    let store_id = Regex::new(r"<StoreId>\s*([A-Za-z0-9]+)\s*</StoreId>")
        .ok()?
        .captures(xml)
        .map(|c| c[1].to_string());

    Some(XboxGame {
        title,
        family_name: format!("{name}_{}", publisher_id(&publisher)),
        app_id,
        store_id,
    })
}

fn unescape(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Package publisher id: Crockford base32 of the first 8 bytes of
/// SHA-256(UTF-16LE(publisher)), 13 characters (64 bits + one zero bit).
fn publisher_id(publisher: &str) -> String {
    const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";
    let utf16: Vec<u8> = publisher
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let hash = Sha256::digest(&utf16);
    let bits = u64::from_be_bytes(hash[..8].try_into().expect("8 bytes")) as u128;
    let padded = bits << 1; // 65 bits = 13 groups of 5
    (0..13)
        .rev()
        .map(|i| ALPHABET[((padded >> (i * 5)) & 0x1f) as usize] as char)
        .collect()
}

/// Store poster (2:3) URLs by Store id, from the public display catalog.
pub async fn posters(
    store_ids: &[String],
    cache: &Path,
) -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;
    let mut known: HashMap<String, String> = fs::read(cache)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let missing: Vec<&String> = store_ids
        .iter()
        .filter(|id| !known.contains_key(*id))
        .collect();

    for chunk in missing.chunks(20) {
        let ids: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
        let url = format!(
            "https://displaycatalog.mp.microsoft.com/v7.0/products?bigIds={}&market=US&languages=en-us",
            ids.join(",")
        );
        let Ok(response) = crate::installer::http().get(&url).send().await else {
            continue;
        };
        let Ok(body) = response.json::<serde_json::Value>().await else {
            continue;
        };
        for product in body["Products"].as_array().into_iter().flatten() {
            let Some(id) = product["ProductId"].as_str() else {
                continue;
            };
            let images = product["LocalizedProperties"][0]["Images"].as_array();
            let pick = |purpose: &str| {
                images
                    .into_iter()
                    .flatten()
                    .find(|im| im["ImagePurpose"] == purpose)
                    .and_then(|im| im["Uri"].as_str())
            };
            if let Some(uri) = pick("Poster").or_else(|| pick("BoxArt")) {
                let uri = if uri.starts_with("//") {
                    format!("https:{uri}")
                } else {
                    uri.to_string()
                };
                known.insert(id.to_string(), uri);
            }
        }
    }
    if let Ok(bytes) = serde_json::to_vec(&known) {
        let _ = fs::write(cache, bytes);
    }
    known
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publisher_id_matches_windows() {
        // Microsoft's own packages end in _8wekyb3d8bbwe.
        assert_eq!(
            publisher_id(
                "CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US"
            ),
            "8wekyb3d8bbwe"
        );
    }

    #[test]
    fn parses_gaming_root() {
        let mut bytes = b"RGBX".to_vec();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend("\\XboxGames\0".encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(parse_gaming_root(&bytes), vec!["XboxGames".to_string()]);
        assert!(parse_gaming_root(b"nope").is_empty());
    }

    #[test]
    fn parses_microsoft_game_config() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Game configVersion="1">
  <Identity Name="BethesdaSoftworks.ProjectGold" Publisher="CN=1A0A4A1F-4F1E-4B5B-8E3A-8F7B6C5D4E3F" Version="1.0.0.0"/>
  <StoreId>9NCJSXWZTP88</StoreId>
  <ExecutableList>
    <Executable Name="Starfield.exe" Id="Game" TargetDeviceFamily="PC"/>
  </ExecutableList>
  <ShellVisuals DefaultDisplayName="Starfield" PublisherDisplayName="Bethesda Softworks" Square150x150Logo="Logo.png"/>
</Game>"#;
        let game = parse_config(xml, "Starfield").unwrap();
        assert_eq!(game.title, "Starfield");
        assert_eq!(game.app_id, "Game");
        assert_eq!(game.store_id.as_deref(), Some("9NCJSXWZTP88"));
        assert!(game
            .family_name
            .starts_with("BethesdaSoftworks.ProjectGold_"));
        assert_eq!(
            game.family_name.len(),
            "BethesdaSoftworks.ProjectGold_".len() + 13
        );
        assert!(game.launch_uri().ends_with("!Game"));
    }

    #[test]
    fn localized_names_fall_back_to_folder() {
        let xml = r#"<Game configVersion="1"><Identity Name="A.B" Publisher="CN=X"/><ShellVisuals DefaultDisplayName="ms-resource:Name"/></Game>"#;
        assert_eq!(
            parse_config(xml, "Forza Horizon 5").unwrap().title,
            "Forza Horizon 5"
        );
    }
}
