//! Persistent app state: settings, scanned library and per-game user data.
//! Everything lives as small JSON files in the app data directory.

use crate::device::{self, Tier, TierSetting};
use crate::library::Game;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub rom_root: Option<PathBuf>,
    pub sounds: bool,
    /// Emulator graphics preset level.
    pub graphics: TierSetting,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            rom_root: None,
            sounds: true,
            graphics: TierSetting::Auto,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserData {
    pub favorite: bool,
    /// Unix seconds.
    pub last_played: Option<u64>,
    pub play_seconds: u64,
}

pub struct Dirs {
    pub data: PathBuf,
    pub emulators: PathBuf,
    pub downloads: PathBuf,
}

pub struct RunningGame {
    pub game_id: String,
    /// Sending on this asks the emulator to close (and save) before it is killed.
    pub stop: Option<oneshot::Sender<()>>,
}

pub type Running = Arc<Mutex<Option<RunningGame>>>;

pub struct AppState {
    pub dirs: Dirs,
    pub settings: Mutex<Settings>,
    pub library: Mutex<Vec<Game>>,
    pub user: Mutex<HashMap<String, UserData>>,
    pub running: Running,
    /// Emulator / core ids with an install in flight.
    pub installing: Mutex<HashSet<String>>,
}

impl AppState {
    pub fn load(app: &AppHandle) -> anyhow::Result<Self> {
        let data = app.path().app_data_dir()?;
        let dirs = Dirs {
            emulators: data.join("emulators"),
            downloads: data.join("downloads"),
            data,
        };
        fs::create_dir_all(&dirs.emulators)?;
        fs::create_dir_all(&dirs.downloads)?;

        Ok(Self {
            settings: Mutex::new(read_json(&dirs.data.join("settings.json")).unwrap_or_default()),
            library: Mutex::new(read_json(&dirs.data.join("library.json")).unwrap_or_default()),
            user: Mutex::new(read_json(&dirs.data.join("userdata.json")).unwrap_or_default()),
            running: Arc::new(Mutex::new(None)),
            installing: Mutex::new(HashSet::new()),
            dirs,
        })
    }

    pub fn save_settings(&self) -> anyhow::Result<()> {
        write_json(
            &self.dirs.data.join("settings.json"),
            &*self.settings.lock().unwrap(),
        )
    }

    pub fn save_library(&self) -> anyhow::Result<()> {
        write_json(
            &self.dirs.data.join("library.json"),
            &*self.library.lock().unwrap(),
        )
    }

    pub fn save_user(&self) -> anyhow::Result<()> {
        write_json(
            &self.dirs.data.join("userdata.json"),
            &*self.user.lock().unwrap(),
        )
    }

    pub fn rom_root(&self) -> Option<PathBuf> {
        self.settings.lock().unwrap().rom_root.clone()
    }

    pub fn tier(&self) -> Tier {
        device::tier(self.settings.lock().unwrap().graphics)
    }

    /// BIOS files live next to the ROM folders so users find them in one place.
    pub fn bios_dir(&self) -> Option<PathBuf> {
        self.rom_root().map(|r| r.join("bios"))
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
}

/// Write via a temp file so a crash never leaves a half-written file behind.
fn write_json<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}
