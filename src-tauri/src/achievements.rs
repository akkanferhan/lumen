//! A game's RetroAchievements: which exist and which the user has. The game
//! is found by its RetroAchievements hash (rcheevos' rules, see `rom_hash`),
//! then the achievement list (`r=patch`) and the user's softcore and
//! hardcore unlocks (`r=unlocks`) come from the same endpoint the emulators
//! use, with the token Lumen already keeps. Disc systems hash their file
//! systems in involved ways and aren't covered yet.

use crate::retroachievements::Account;
use anyhow::{anyhow, bail};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io::Read, path::Path};

const URL: &str = "https://retroachievements.org/dorequest.php";
const BADGES: &str = "https://media.retroachievements.org/Badge";

/// Systems whose hash is the (unheadered) ROM's MD5, or the set name for arcade.
pub fn supported(system_id: &str) -> bool {
    matches!(
        system_id,
        "nes"
            | "snes"
            | "n64"
            | "gb"
            | "gbc"
            | "gba"
            | "genesis"
            | "sms"
            | "arcade"
            | "ngp"
            | "ngpc"
            | "ws"
            | "wsc"
            | "a2600"
    )
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub points: u64,
    pub badge: String,
    pub unlocked: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub game_id: u64,
    pub achievements: Vec<Achievement>,
}

/// The RetroAchievements hash of a ROM, following rcheevos (rc_hash_rom.c):
/// arcade hashes the set name; NES/FDS drop their 16-byte header, SNES a
/// 512-byte copier header; N64 is hashed in big-endian (z64) byte order.
/// A zip is hashed by its first file, which is what the core loads.
pub fn rom_hash(system_id: &str, path: &Path) -> anyhow::Result<String> {
    if system_id == "arcade" {
        let set = path.file_stem().ok_or_else(|| anyhow!("Geçersiz dosya"))?;
        return Ok(hex(&Md5::digest(set.to_string_lossy().as_bytes())));
    }
    let data = rom_bytes(path)?;
    let data: Vec<u8> = match system_id {
        "nes"
            if data.len() > 16
                && (data.starts_with(b"NES\x1a") || data.starts_with(b"FDS\x1a")) =>
        {
            data[16..].to_vec()
        }
        "snes" if data.len() % 0x2000 == 512 => data[512..].to_vec(),
        "n64" => to_big_endian_n64(data),
        _ => data,
    };
    Ok(hex(&Md5::digest(&data)))
}

fn rom_bytes(path: &Path) -> anyhow::Result<Vec<u8>> {
    let is_zip = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    if !is_zip {
        return Ok(std::fs::read(path)?);
    }
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path)?)?;
    let index = (0..archive.len())
        .find(|&i| archive.by_index(i).is_ok_and(|f| f.is_file()))
        .ok_or_else(|| anyhow!("Arşiv boş"))?;
    let mut data = Vec::new();
    archive.by_index(index)?.read_to_end(&mut data)?;
    Ok(data)
}

/// v64 swaps byte pairs; n64 (little endian) reverses each 32-bit word.
fn to_big_endian_n64(mut data: Vec<u8>) -> Vec<u8> {
    match data.first() {
        Some(0x37) => data
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .for_each(|c| c.swap(0, 1)),
        Some(0x40) => data
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .for_each(|c| c.reverse()),
        _ => {}
    }
    data
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub async fn progress(account: &Account, hash: &str) -> anyhow::Result<Option<Progress>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct GameId {
        success: bool,
        #[serde(rename = "GameID", default)]
        game_id: u64,
    }
    let found: GameId = post(&[("r", "gameid"), ("m", hash)]).await?;
    if !found.success || found.game_id == 0 {
        // Not a game RetroAchievements knows (or a hack, a bad dump).
        return Ok(None);
    }
    let game = found.game_id.to_string();
    let auth = [
        ("u", account.username.as_str()),
        ("t", account.token.as_str()),
    ];

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Patch {
        success: bool,
        error: Option<String>,
        patch_data: Option<PatchData>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct PatchData {
        #[serde(default)]
        achievements: Vec<Raw>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Raw {
        #[serde(rename = "ID")]
        id: u64,
        title: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        points: u64,
        #[serde(default)]
        badge_name: String,
        /// 3: official ("core"), 5: unofficial.
        #[serde(default)]
        flags: u64,
    }
    let patch: Patch =
        post(&[&[("r", "patch"), ("g", game.as_str())][..], &auth[..]].concat()).await?;
    if !patch.success {
        bail!(patch.error.unwrap_or_else(|| "Başarımlar alınamadı".into()));
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Unlocks {
        #[serde(default)]
        user_unlocks: Vec<u64>,
    }
    let mut unlocked = HashSet::new();
    for hardcore in ["0", "1"] {
        let unlocks: Unlocks = post(
            &[
                &[("r", "unlocks"), ("g", game.as_str()), ("h", hardcore)][..],
                &auth[..],
            ]
            .concat(),
        )
        .await?;
        unlocked.extend(unlocks.user_unlocks);
    }

    let achievements = patch
        .patch_data
        .map(|p| p.achievements)
        .unwrap_or_default()
        .into_iter()
        .filter(|a| a.flags == 3)
        .map(|a| Achievement {
            unlocked: unlocked.contains(&a.id),
            badge: format!(
                "{BADGES}/{}{}.png",
                a.badge_name,
                if unlocked.contains(&a.id) {
                    ""
                } else {
                    "_lock"
                }
            ),
            id: a.id,
            title: a.title,
            description: a.description,
            points: a.points,
        })
        .collect();
    Ok(Some(Progress {
        game_id: found.game_id,
        achievements,
    }))
}

async fn post<T: for<'de> Deserialize<'de>>(form: &[(&str, &str)]) -> anyhow::Result<T> {
    crate::installer::http()
        .post(URL)
        .form(form)
        .send()
        .await?
        .json()
        .await
        .map_err(|_| anyhow!("RetroAchievements'a ulaşılamadı"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str, data: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("lumen-ra-{}-{name}", std::process::id()));
        std::fs::write(&path, data).unwrap();
        path
    }

    #[test]
    fn hashes_follow_rcheevos() {
        let rom = b"0123456789abcdef";
        let plain = hex(&Md5::digest(rom));
        // NES: the iNES header is not part of the hash.
        let mut nes = b"NES\x1a".to_vec();
        nes.extend_from_slice(&[0u8; 12]);
        nes.extend_from_slice(rom);
        let path = temp("game.nes", &nes);
        assert_eq!(rom_hash("nes", &path).unwrap(), plain);
        // SNES: a 512-byte copier header is skipped.
        let mut snes = vec![0u8; 512];
        snes.extend_from_slice(&[7u8; 0x2000]);
        let path = temp("game.sfc", &snes);
        assert_eq!(
            rom_hash("snes", &path).unwrap(),
            hex(&Md5::digest([7u8; 0x2000]))
        );
        // N64: v64 and n64 orders hash like the z64 original.
        let z64 = [0x80u8, 0x37, 0x12, 0x40, 1, 2, 3, 4];
        let v64 = [0x37u8, 0x80, 0x40, 0x12, 2, 1, 4, 3];
        let n64 = [0x40u8, 0x12, 0x37, 0x80, 4, 3, 2, 1];
        let want = hex(&Md5::digest(z64));
        for (name, data) in [("a.z64", &z64), ("a.v64", &v64), ("a.n64", &n64)] {
            assert_eq!(rom_hash("n64", &temp(name, data)).unwrap(), want, "{name}");
        }
        // Arcade: the set name, not the zip's contents.
        let path = temp("dir", b"");
        let set = path.with_file_name("pacman.zip");
        assert_eq!(
            rom_hash("arcade", &set).unwrap(),
            hex(&Md5::digest(b"pacman"))
        );
    }

    #[test]
    fn hashes_the_rom_inside_a_zip() {
        let path = std::env::temp_dir().join(format!("lumen-ra-{}-rom.zip", std::process::id()));
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        zip.start_file::<_, ()>("game.gba", Default::default())
            .unwrap();
        std::io::Write::write_all(&mut zip, b"gba rom").unwrap();
        zip.finish().unwrap();
        assert_eq!(
            rom_hash("gba", &path).unwrap(),
            hex(&Md5::digest(b"gba rom"))
        );
    }
}
