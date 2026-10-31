//! Removing games from the device. Emulated games go to the Recycle Bin
//! (saves are kept, so re-adding a game continues where it left off); store
//! games are uninstalled by their own store.

use crate::library::{game_files, Game};
use crate::pc::{EPIC, STEAM};
use crate::xbox::XBOX_APP;
use anyhow::{anyhow, bail};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Method {
    /// Files moved to the Recycle Bin by Lumen.
    RecycleBin,
    /// The store shows its own confirmation (Steam).
    StoreDialog,
    /// Windows removes the package (Xbox app).
    WindowsPackage,
    /// The store has no direct link; its library is opened (Epic).
    OpenStore,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub method: Method,
    /// Bytes freed, when Lumen can tell.
    pub bytes: Option<u64>,
    pub files: usize,
}

pub fn plan(game: &Game) -> Plan {
    match game.system_id.as_str() {
        STEAM => Plan {
            method: Method::StoreDialog,
            bytes: None,
            files: 0,
        },
        EPIC => Plan {
            method: Method::OpenStore,
            bytes: None,
            files: 0,
        },
        XBOX_APP => Plan {
            method: Method::WindowsPackage,
            bytes: None,
            files: 0,
        },
        _ => {
            let files = game_files(&game.path);
            Plan {
                method: Method::RecycleBin,
                bytes: Some(files.iter().map(|f| size(f)).sum()),
                files: files.len(),
            }
        }
    }
}

/// Carries out the plan. Returns true when the game is gone right away (so the
/// library can drop it), false when a store still has to finish the job.
pub fn uninstall(game: &Game) -> anyhow::Result<bool> {
    match game.system_id.as_str() {
        STEAM => {
            // steam://rungameid/<id> → steam://uninstall/<id>
            let id = game
                .path
                .to_string_lossy()
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_string();
            if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
                bail!("Steam oyun kimliği okunamadı");
            }
            open(&format!("steam://uninstall/{id}"))?;
            Ok(false)
        }
        EPIC => {
            open("com.epicgames.launcher://apps")?;
            Ok(false)
        }
        XBOX_APP => {
            remove_package(&package_family(&game.path.to_string_lossy())?)?;
            Ok(true)
        }
        _ => {
            let files = game_files(&game.path);
            if files.iter().all(|f| !f.exists()) {
                return Ok(true);
            }
            trash::delete_all(files.iter().filter(|f| f.exists()))
                .map_err(|e| anyhow!("Geri Dönüşüm Kutusu'na taşınamadı: {e}"))?;
            Ok(true)
        }
    }
}

/// `shell:AppsFolder\<family>!<app>` → `<family>`, accepting only package-name
/// characters since the value is handed to PowerShell.
fn package_family(uri: &str) -> anyhow::Result<String> {
    let family = uri
        .strip_prefix(r"shell:AppsFolder\")
        .and_then(|rest| rest.split('!').next())
        .ok_or_else(|| anyhow!("Xbox paket adı okunamadı"))?;
    let valid = !family.is_empty()
        && family
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !valid {
        bail!("Geçersiz Xbox paket adı");
    }
    Ok(family.to_string())
}

#[cfg(windows)]
fn remove_package(family: &str) -> anyhow::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = format!(
        "Get-AppxPackage -PackageFamilyName '{family}' | Remove-AppxPackage -ErrorAction Stop"
    );
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    if !output.status.success() {
        bail!(
            "Xbox oyunu kaldırılamadı: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(not(windows))]
fn remove_package(_family: &str) -> anyhow::Result<()> {
    bail!("Xbox oyunları sadece Windows'ta kaldırılabilir")
}

fn open(uri: &str) -> anyhow::Result<()> {
    std::process::Command::new(if cfg!(windows) {
        "explorer.exe"
    } else {
        "open"
    })
    .arg(uri)
    .spawn()?;
    Ok(())
}

fn size(path: &Path) -> u64 {
    if path.is_dir() {
        walkdir::WalkDir::new(path)
            .into_iter()
            .flatten()
            .filter_map(|e| e.metadata().ok())
            .filter(|m| m.is_file())
            .map(|m| m.len())
            .sum()
    } else {
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn package_family_is_validated() {
        assert_eq!(
            package_family(r"shell:AppsFolder\BethesdaSoftworks.ProjectGold_3275kfvn8vcwc!Game")
                .unwrap(),
            "BethesdaSoftworks.ProjectGold_3275kfvn8vcwc"
        );
        assert!(package_family(r"shell:AppsFolder\x'; Remove-Item C:\ -Recurse; '!Game").is_err());
        assert!(package_family("steam://rungameid/620").is_err());
    }

    #[test]
    fn plans_by_source() {
        let game = |system: &str, path: &str| Game {
            id: "x".into(),
            system_id: system.into(),
            title: "T".into(),
            thumb_name: String::new(),
            path: PathBuf::from(path),
            not_installed: false,
            cover: None,
        };
        assert_eq!(
            plan(&game("steam", "steam://rungameid/620")).method,
            Method::StoreDialog
        );
        assert_eq!(
            plan(&game("epic", "com.epicgames.launcher://apps/x")).method,
            Method::OpenStore
        );
        assert_eq!(
            plan(&game("xboxapp", r"shell:AppsFolder\A.B_c!Game")).method,
            Method::WindowsPackage
        );

        let dir = std::env::temp_dir().join(format!("lumen-uninstall-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rom = dir.join("game.sfc");
        std::fs::write(&rom, vec![0u8; 2048]).unwrap();
        let p = plan(&game("snes", &rom.to_string_lossy()));
        assert_eq!(
            (p.method, p.bytes, p.files),
            (Method::RecycleBin, Some(2048), 1)
        );
    }
}
