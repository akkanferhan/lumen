//! PS Vita via Vita3K. Unlike other emulators, Vita3K installs a game once
//! (from a .vpk/.zip or a dumped folder) and then runs it by title ID. It
//! also needs the Vita system software and font package, which Sony
//! distributes for free; Lumen fetches both from Sony's update servers.

use crate::installer;
use anyhow::{anyhow, bail};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use tauri::AppHandle;

pub(crate) const FIRMWARE: &str = "http://dus01.psv.update.playstation.net/update/psv/image/2022_0209/rel_f2c7b12fe85496ec88a0391b514d6e3b/PSVUPDAT.PUP";
pub(crate) const FONTS: &str = "http://dus01.psp2.update.playstation.net/update/psp2/image/2019_0924/sd_8b5f60b56c3da8365b973dba570c53a5/PSP2UPDAT.PUP";

/// Vita3K's file system in portable mode (`<exe>/portable/fs`).
pub fn fs_root(exe_dir: &Path) -> PathBuf {
    exe_dir.join("portable").join("fs")
}

/// Title ID from a dumped folder or from inside a .vpk / .zip.
pub fn title_id(game: &Path) -> Option<String> {
    let sfo = if game.is_dir() {
        fs::read(game.join("sce_sys").join("param.sfo")).ok()?
    } else {
        let mut archive = zip::ZipArchive::new(fs::File::open(game).ok()?).ok()?;
        // .vpk has it at the root; NoNpDrm zips nest it under app/<ID>/.
        let name = archive
            .file_names()
            .filter(|n| n.to_lowercase().ends_with("sce_sys/param.sfo"))
            .min_by_key(|n| n.len())?
            .to_string();
        let mut bytes = Vec::new();
        archive.by_name(&name).ok()?.read_to_end(&mut bytes).ok()?;
        bytes
    };
    crate::library::sfo_string(&sfo, "TITLE_ID")
        .filter(|id| id.chars().all(|c| c.is_ascii_alphanumeric()))
}

pub fn is_installed(exe_dir: &Path, title_id: &str) -> bool {
    fs_root(exe_dir)
        .join("ux0")
        .join("app")
        .join(title_id)
        .is_dir()
}

/// What Vita3K gets in place of `{rom}`: the installed title, or the content
/// to install (and then run) on first launch.
pub fn rom_args(exe_dir: &Path, game: &Path) -> Vec<String> {
    match title_id(game) {
        Some(id) if is_installed(exe_dir, &id) => vec!["-r".into(), id],
        _ => vec![game.to_string_lossy().into_owned()],
    }
}

/// Installs the system software and fonts once. `--firmware` installs and exits.
pub async fn ensure_firmware(app: &AppHandle, exe: &Path, downloads: &Path) -> anyhow::Result<()> {
    let exe_dir = exe
        .parent()
        .ok_or_else(|| anyhow!("Geçersiz emülatör yolu"))?;
    let root = fs_root(exe_dir);
    for (id, url, marker) in [
        ("firmware:vita", FIRMWARE, "vs0"),
        ("firmware:vita-fonts", FONTS, "sa0"),
    ] {
        if root.join(marker).is_dir() {
            continue;
        }
        let pup = installer::download_file(app, id, url, downloads).await?;
        installer::emit(app, id, "configuring", 1.0, None);
        let status = tokio::process::Command::new(exe)
            .arg("--firmware")
            .arg(&pup)
            .current_dir(exe_dir)
            .status()
            .await?;
        let _ = fs::remove_file(&pup);
        if !status.success() || !root.join(marker).is_dir() {
            installer::emit(
                app,
                id,
                "error",
                0.0,
                Some("PS Vita firmware kurulamadı".into()),
            );
            bail!("PS Vita firmware kurulamadı");
        }
        installer::emit(app, id, "done", 1.0, None);
    }
    Ok(())
}

/// Removes an installed title from Vita3K (`-d` deletes app, DLC and saves'
/// shader cache; Lumen backs saves up beforehand when backups are on).
pub fn delete_title(exe: &Path, title_id: &str) -> anyhow::Result<()> {
    let exe_dir = exe
        .parent()
        .ok_or_else(|| anyhow!("Geçersiz emülatör yolu"))?;
    std::process::Command::new(exe)
        .args(["-d", title_id])
        .current_dir(exe_dir)
        .status()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn sfo(title_id: &str) -> Vec<u8> {
        let key = b"TITLE_ID\0";
        let value = format!("{title_id}\0").into_bytes();
        let key_table = 20 + 16;
        let data_table = key_table + key.len();
        let mut out = b"\0PSF".to_vec();
        out.extend_from_slice(&0x0101u32.to_le_bytes());
        out.extend_from_slice(&(key_table as u32).to_le_bytes());
        out.extend_from_slice(&(data_table as u32).to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0x0204u16.to_le_bytes());
        out.extend_from_slice(&(value.len() as u32).to_le_bytes());
        out.extend_from_slice(&(value.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(key);
        out.extend_from_slice(&value);
        out
    }

    #[test]
    fn reads_title_id_from_vpk_and_folder() {
        let dir = std::env::temp_dir().join(format!("lumen-vita-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("PCSE00000/sce_sys")).unwrap();
        fs::write(dir.join("PCSE00000/sce_sys/param.sfo"), sfo("PCSE00000")).unwrap();
        assert_eq!(
            title_id(&dir.join("PCSE00000")).as_deref(),
            Some("PCSE00000")
        );

        let vpk = dir.join("game.vpk");
        let mut zip = zip::ZipWriter::new(fs::File::create(&vpk).unwrap());
        zip.start_file::<_, ()>("eboot.bin", Default::default())
            .unwrap();
        zip.write_all(b"x").unwrap();
        zip.start_file::<_, ()>("sce_sys/param.sfo", Default::default())
            .unwrap();
        zip.write_all(&sfo("VITA00001")).unwrap();
        zip.finish().unwrap();
        assert_eq!(title_id(&vpk).as_deref(), Some("VITA00001"));

        // Not installed yet: Vita3K gets the file to install.
        assert_eq!(
            rom_args(&dir, &vpk),
            vec![vpk.to_string_lossy().into_owned()]
        );
        fs::create_dir_all(fs_root(&dir).join("ux0/app/VITA00001")).unwrap();
        assert_eq!(
            rom_args(&dir, &vpk),
            vec!["-r".to_string(), "VITA00001".to_string()]
        );
    }
}
