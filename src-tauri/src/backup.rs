//! Save backups: each emulator's save and save-state folders (plus Lumen's
//! own play data) are mirrored to a folder of the user's choice, by default
//! inside OneDrive so they reach the cloud. Copies are incremental: only new
//! or changed files are written, so a backup after every session is cheap.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// Where each emulator keeps saves, relative to its install (portable) dir.
/// Install dirs are versioned by build, so these are matched anywhere below it.
pub fn save_dirs(emulator: &str) -> &'static [&'static str] {
    match emulator {
        "retroarch" => &["saves", "states"],
        "duckstation" => &["memcards", "savestates"],
        "pcsx2" => &["memcards", "sstates"],
        "dolphin" => &["User/GC", "User/Wii/title", "User/StateSaves"],
        "ppsspp" => &["memstick/PSP/SAVEDATA", "memstick/PSP/PPSSPP_STATE"],
        "azahar" => &["user/sdmc", "user/states"],
        "cemu" => &["portable/mlc01/usr/save"],
        "rpcs3" => &["dev_hdd0/home/00000001/savedata"],
        "shadps4" => &["user/home"],
        "sharpemu" => &["user/savedata"],
        "xenia" => &["content"],
        "vita3k" => &["portable/fs/ux0/user/00/savedata"],
        // xemu keeps saves inside its HDD image; melonDS writes .sav beside the ROM.
        _ => &[],
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BackupSettings {
    pub enabled: bool,
    pub dir: Option<PathBuf>,
    /// Unix seconds of the last successful backup.
    pub last_backup: Option<u64>,
    /// Before each game, newer saves for its emulator are pulled from the
    /// backup folder (played on another device sharing the cloud folder).
    pub sync: bool,
}

impl BackupSettings {
    pub fn enabled_by_default() -> Self {
        Self {
            enabled: true,
            dir: None,
            last_backup: None,
            sync: false,
        }
    }

    pub fn target(&self) -> PathBuf {
        self.dir.clone().unwrap_or_else(default_dir)
    }
}

/// A cloud drive's synced folder on this PC.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudFolder {
    pub name: &'static str,
    /// Where Lumen's backup goes inside it.
    pub path: PathBuf,
}

/// Cloud drives with a synced folder here: OneDrive, Google Drive (its
/// own drive letter, "My Drive"), Dropbox and iCloud Drive.
pub fn cloud_folders() -> Vec<CloudFolder> {
    let env = |name: &str| std::env::var_os(name).map(PathBuf::from);
    let home = env("USERPROFILE");
    let mut found: Vec<(&'static str, PathBuf)> = Vec::new();
    for var in ["OneDrive", "OneDriveConsumer"] {
        if let Some(dir) = env(var) {
            found.push(("OneDrive", dir));
        }
    }
    for letter in b'D'..=b'Z' {
        for name in ["My Drive", "Drive'ım"] {
            found.push((
                "Google Drive",
                PathBuf::from(format!("{}:\\{name}", letter as char)),
            ));
        }
    }
    if let Some(home) = &home {
        found.push(("Google Drive", home.join("Google Drive")));
    }
    // Dropbox says where it syncs in info.json.
    for base in [env("APPDATA"), env("LOCALAPPDATA")].into_iter().flatten() {
        let info: Option<serde_json::Value> = fs::read(base.join("Dropbox").join("info.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        if let Some(path) = info.as_ref().and_then(|i| i["personal"]["path"].as_str()) {
            found.push(("Dropbox", PathBuf::from(path)));
        }
    }
    if let Some(home) = &home {
        found.push(("Dropbox", home.join("Dropbox")));
        found.push(("iCloud Drive", home.join("iCloudDrive")));
    }

    let mut folders: Vec<CloudFolder> = Vec::new();
    for (name, dir) in found {
        if dir.is_dir() && !folders.iter().any(|f| f.name == name) {
            folders.push(CloudFolder {
                name,
                path: dir.join("Lumen Yedek"),
            });
        }
    }
    folders
}

/// OneDrive when it's set up (synced to the cloud), Documents otherwise.
pub fn default_dir() -> PathBuf {
    let base = std::env::var_os("OneDrive")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .or_else(|| std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join("Documents")))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Documents")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Lumen Yedek")
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub files: u32,
    pub bytes: u64,
}

impl std::ops::AddAssign for Report {
    fn add_assign(&mut self, other: Self) {
        self.files += other.files;
        self.bytes += other.bytes;
    }
}

/// Backs up one emulator's saves: `<backup>/<emulator>/<save dir>/…`.
/// `install_dir` is the emulator's executable folder (or an ancestor of it).
pub fn backup_emulator(
    emulator: &str,
    install_dir: &Path,
    backup_root: &Path,
) -> std::io::Result<Report> {
    let mut report = Report::default();
    for rel in save_dirs(emulator) {
        if let Some(source) = find_dir(install_dir, rel) {
            report += mirror(&source, &backup_root.join(emulator).join(rel))?;
        }
    }
    Ok(report)
}

/// Copies saves back from the backup, only where the backup copy is newer.
pub fn restore_emulator(
    emulator: &str,
    install_dir: &Path,
    backup_root: &Path,
) -> std::io::Result<Report> {
    let mut report = Report::default();
    for rel in save_dirs(emulator) {
        let backup = backup_root.join(emulator).join(rel);
        if !backup.is_dir() {
            continue;
        }
        let target = find_dir(install_dir, rel).unwrap_or_else(|| install_dir.join(rel));
        report += mirror(&backup, &target)?;
    }
    Ok(report)
}

/// Lumen's own play data (favorites, play time, settings).
pub fn backup_lumen(data_dir: &Path, backup_root: &Path) -> std::io::Result<Report> {
    let mut report = Report::default();
    for name in ["userdata.json", "settings.json"] {
        report += copy_if_newer(&data_dir.join(name), &backup_root.join("lumen").join(name))?;
    }
    Ok(report)
}

/// Emulator archives often unpack into a versioned subfolder; saves live
/// beside the executable, so look a couple of levels down.
fn find_dir(install_dir: &Path, rel: &str) -> Option<PathBuf> {
    walkdir::WalkDir::new(install_dir)
        .max_depth(3)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_dir())
        .map(|e| e.path().join(rel))
        .find(|p| p.is_dir())
}

fn mirror(from: &Path, to: &Path) -> std::io::Result<Report> {
    let mut report = Report::default();
    for entry in walkdir::WalkDir::new(from).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(from) else {
            continue;
        };
        report += copy_if_newer(entry.path(), &to.join(rel))?;
    }
    Ok(report)
}

fn copy_if_newer(from: &Path, to: &Path) -> std::io::Result<Report> {
    let Ok(source) = fs::metadata(from) else {
        return Ok(Report::default());
    };
    let modified = |m: &fs::Metadata| m.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    if let Ok(existing) = fs::metadata(to) {
        if existing.len() == source.len() && modified(&existing) >= modified(&source) {
            return Ok(Report::default());
        }
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(from, to)?;
    // Keep the source time so later comparisons see the copy as current.
    if let Ok(file) = fs::File::options().write(true).open(to) {
        let _ = file.set_modified(modified(&source));
    }
    Ok(Report {
        files: 1,
        bytes: source.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lumen-backup-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn backs_up_incrementally_and_restores() {
        let install = temp("install");
        let backup = temp("backup");
        // Versioned subfolder, like many emulator archives.
        let cards = install.join("duckstation-v1").join("memcards");
        fs::create_dir_all(&cards).unwrap();
        fs::write(cards.join("shared_card_1.mcd"), b"save-1").unwrap();

        let first = backup_emulator("duckstation", &install, &backup).unwrap();
        assert_eq!(first.files, 1);
        let copy = backup.join("duckstation/memcards/shared_card_1.mcd");
        assert_eq!(fs::read(&copy).unwrap(), b"save-1");

        // Unchanged: nothing copied.
        assert_eq!(
            backup_emulator("duckstation", &install, &backup)
                .unwrap()
                .files,
            0
        );

        // Lost locally: restore brings it back.
        fs::remove_file(cards.join("shared_card_1.mcd")).unwrap();
        let restored = restore_emulator("duckstation", &install, &backup).unwrap();
        assert_eq!(restored.files, 1);
        assert_eq!(
            fs::read(cards.join("shared_card_1.mcd")).unwrap(),
            b"save-1"
        );
    }

    #[test]
    fn unknown_emulators_back_up_nothing() {
        let install = temp("none");
        assert_eq!(
            backup_emulator("xemu", &install, &temp("none-b"))
                .unwrap()
                .files,
            0
        );
    }
}
