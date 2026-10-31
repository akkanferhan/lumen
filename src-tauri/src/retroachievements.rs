//! One RetroAchievements login for every emulator. Lumen logs in once
//! (the password is never stored), keeps the API token, and writes it into
//! each emulator in that emulator's own format before launch.

use crate::config::{set_cfg, set_ini};
use anyhow::{anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

const LOGIN_URL: &str = "https://retroachievements.org/dorequest.php";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub username: String,
    pub token: String,
    #[serde(default)]
    pub score: Option<u64>,
}

/// What emulators get: the account (None = signed out) and hardcore mode.
#[derive(Debug, Clone, Copy)]
pub struct Cheevos<'a> {
    pub account: Option<&'a Account>,
    pub hardcore: bool,
}

/// `r=login2` with a password (or, for refreshing, a token). Same request
/// rcheevos makes inside the emulators.
pub async fn login(username: &str, secret: Secret<'_>) -> anyhow::Result<Account> {
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Response {
        success: bool,
        user: Option<String>,
        token: Option<String>,
        score: Option<u64>,
        error: Option<String>,
    }
    let (key, value) = match secret {
        Secret::Password(p) => ("p", p),
        Secret::Token(t) => ("t", t),
    };
    let response: Response = crate::installer::http()
        .post(LOGIN_URL)
        .form(&[("r", "login2"), ("u", username), (key, value)])
        .send()
        .await?
        .json()
        .await
        .map_err(|_| {
            anyhow!(crate::tr!(
                "RetroAchievements'a ulaşılamadı",
                "Couldn't reach RetroAchievements"
            ))
        })?;
    if !response.success {
        bail!(response
            .error
            .unwrap_or_else(|| crate::tr!("Giriş başarısız", "Sign-in failed")));
    }
    Ok(Account {
        username: response.user.unwrap_or_else(|| username.to_string()),
        token: response.token.ok_or_else(|| {
            anyhow!(crate::tr!(
                "RetroAchievements token göndermedi",
                "RetroAchievements didn't send a token"
            ))
        })?,
        score: response.score,
    })
}

pub enum Secret<'a> {
    Password(&'a str),
    Token(&'a str),
}

/// Writes (or clears) the login in one emulator. Hardcore is always written:
/// the emulators' own defaults differ.
pub fn apply(emulator: &str, exe_dir: &Path, cheevos: Cheevos) -> anyhow::Result<()> {
    let enabled = cheevos.account.is_some();
    let (user, token) = cheevos
        .account
        .map(|a| (a.username.as_str(), a.token.as_str()))
        .unwrap_or(("", ""));
    let bool_str = |b: bool| if b { "true" } else { "false" }.to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();

    match emulator {
        "retroarch" => {
            let credentials = [
                ("cheevos_username", user.to_string()),
                ("cheevos_token", token.to_string()),
                // Token login is used when the password is empty.
                ("cheevos_password", String::new()),
            ];
            set_cfg(&exe_dir.join("retroarch.cfg"), &credentials)?;
            // Newer RetroArch builds keep credentials in a keychain file.
            let keychain = exe_dir.join("retroarch-keychain.cfg");
            if keychain.exists() {
                set_cfg(&keychain, &credentials)?;
            }
            set_cfg(
                &exe_dir.join("lumen.cfg"),
                &[
                    ("cheevos_enable", bool_str(enabled)),
                    ("cheevos_hardcore_mode_enable", bool_str(cheevos.hardcore)),
                ],
            )
        }
        "duckstation" => set_ini(
            &exe_dir.join("settings.ini"),
            &[
                ("Cheevos", "Enabled", bool_str(enabled)),
                ("Cheevos", "ChallengeMode", bool_str(cheevos.hardcore)),
                ("Cheevos", "Username", user.to_string()),
                // DuckStation rejects a plain token; it must be encrypted its way.
                (
                    "Cheevos",
                    "Token",
                    if enabled {
                        duckstation_token(token, user)
                    } else {
                        String::new()
                    },
                ),
                (
                    "Cheevos",
                    "LoginTimestamp",
                    if enabled { now } else { String::new() },
                ),
            ],
        ),
        "pcsx2" => {
            let inis = exe_dir.join("inis");
            set_ini(
                &inis.join("PCSX2.ini"),
                &[
                    ("Achievements", "Enabled", bool_str(enabled)),
                    ("Achievements", "ChallengeMode", bool_str(cheevos.hardcore)),
                    ("Achievements", "Username", user.to_string()),
                    (
                        "Achievements",
                        "LoginTimestamp",
                        if enabled { now } else { String::new() },
                    ),
                ],
            )?;
            set_ini(
                &inis.join("secrets.ini"),
                &[("Achievements", "Token", token.to_string())],
            )
        }
        "ppsspp" => {
            let system = exe_dir.join("memstick").join("PSP").join("SYSTEM");
            set_ini(
                &system.join("ppsspp.ini"),
                &[
                    (
                        "Achievements",
                        "AchievementsEnable",
                        if enabled { "True" } else { "False" }.into(),
                    ),
                    (
                        "Achievements",
                        "AchievementsChallengeMode",
                        if cheevos.hardcore { "True" } else { "False" }.into(),
                    ),
                    ("Achievements", "AchievementsUserName", user.to_string()),
                ],
            )?;
            // Raw token bytes, no newline.
            std::fs::create_dir_all(&system)?;
            std::fs::write(system.join("ppsspp_retroachievements.dat"), token)?;
            Ok(())
        }
        "dolphin" => set_ini(
            &exe_dir
                .join("User")
                .join("Config")
                .join("RetroAchievements.ini"),
            &[
                (
                    "Achievements",
                    "Enabled",
                    if enabled { "True" } else { "False" }.into(),
                ),
                (
                    "Achievements",
                    "HardcoreEnabled",
                    if cheevos.hardcore { "True" } else { "False" }.into(),
                ),
                ("Achievements", "Username", user.to_string()),
                ("Achievements", "ApiToken", token.to_string()),
            ],
        ),
        _ => Ok(()),
    }
}

/// DuckStation's token cipher in portable mode: key = SHA-256(username)
/// stretched with 100 more SHA-256 rounds; AES-128-CBC with key[0..16] and
/// IV key[16..32]; token zero-padded to 16 bytes; base64.
pub fn duckstation_token(token: &str, username: &str) -> String {
    use aes::cipher::{generic_array::GenericArray, BlockEncrypt, KeyInit};
    use base64::Engine;

    if token.is_empty() || username.is_empty() {
        return String::new();
    }
    let mut key: [u8; 32] = Sha256::digest(username.as_bytes()).into();
    for _ in 0..100 {
        key = Sha256::digest(key).into();
    }
    let cipher = aes::Aes128::new(GenericArray::from_slice(&key[..16]));

    let mut data = token.as_bytes().to_vec();
    data.resize(data.len().div_ceil(16) * 16, 0);
    let mut previous: [u8; 16] = key[16..].try_into().expect("16-byte IV");
    for block in data.chunks_mut(16) {
        for (b, p) in block.iter_mut().zip(previous) {
            *b ^= p;
        }
        cipher.encrypt_block(GenericArray::from_mut_slice(block));
        previous.copy_from_slice(block);
    }
    base64::engine::general_purpose::STANDARD.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duckstation_token_matches_openssl() {
        // openssl enc -aes-128-cbc -nopad with the same derived key/IV.
        assert_eq!(
            duckstation_token("abcdEFGH12345678xyz", "Player1"),
            "luy/hCEOyseQICChOvdx07t4DsYLL8lJ+gn9fadG0PA="
        );
        assert_eq!(duckstation_token("", "Player1"), "");
    }

    #[test]
    fn writes_and_clears_logins() {
        let dir = std::env::temp_dir().join(format!("lumen-ra-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let account = Account {
            username: "Player1".into(),
            token: "tok".into(),
            score: None,
        };
        let on = Cheevos {
            account: Some(&account),
            hardcore: false,
        };

        apply("dolphin", &dir, on).unwrap();
        let ini = std::fs::read_to_string(dir.join("User/Config/RetroAchievements.ini")).unwrap();
        assert!(ini.contains("ApiToken = tok") && ini.contains("Enabled = True"));
        assert!(ini.contains("HardcoreEnabled = False"));

        apply("ppsspp", &dir, on).unwrap();
        let dat =
            std::fs::read(dir.join("memstick/PSP/SYSTEM/ppsspp_retroachievements.dat")).unwrap();
        assert_eq!(dat, b"tok");

        apply(
            "dolphin",
            &dir,
            Cheevos {
                account: None,
                hardcore: false,
            },
        )
        .unwrap();
        let ini = std::fs::read_to_string(dir.join("User/Config/RetroAchievements.ini")).unwrap();
        assert!(ini.contains("Enabled = False") && !ini.contains("ApiToken = tok"));
    }
}
