//! Persistent app state: settings, scanned library and per-game user data.
//! Everything lives as small JSON files in the app data directory.

use crate::device::{self, Tier, TierSetting};
use crate::library::Game;
use crate::power::PowerSetting;
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
    /// ASUS power mode while playing.
    pub power: PowerSetting,
    /// Save backups (on by default).
    pub backup: crate::backup::BackupSettings,
    /// RetroAchievements hardcore mode (no save states, no cheats).
    pub ra_hardcore: bool,
    /// UI theme id (see src/lib/themes.ts).
    pub theme: String,
    /// Display filter for RetroArch systems.
    pub filter: crate::filters::FilterSetting,
    /// Select + Start in game opens the quick menu (hold: save and quit).
    pub quick_combo: bool,
    /// A small touch button over the game opens the quick menu.
    pub overlay_button: bool,
    /// Battery and clock beside the in-game menu button.
    pub overlay_status: bool,
    /// Kids mode: one collection, no settings, a daily play limit.
    pub kids: crate::kids::Kids,
    /// ROG Ally: the back buttons (M1/M2) open the in-game menu; the game
    /// never sees them.
    pub back_buttons: bool,
    /// Bumped when the in-game menu shortcuts' defaults change (1: the
    /// back buttons replace Select + Start on the Ally).
    pub combo_version: u32,
}

const COMBO_VERSION: u32 = 1;

impl Default for Settings {
    fn default() -> Self {
        Self {
            rom_root: None,
            sounds: true,
            graphics: TierSetting::Auto,
            power: PowerSetting::Auto,
            backup: crate::backup::BackupSettings::enabled_by_default(),
            ra_hardcore: false,
            theme: "lumen".into(),
            filter: crate::filters::FilterSetting::Original,
            // The Ally has back buttons for it instead (see `back_buttons`).
            quick_combo: crate::device::device().model.is_none(),
            overlay_button: true,
            overlay_status: true,
            kids: crate::kids::Kids::default(),
            back_buttons: true,
            combo_version: COMBO_VERSION,
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
    /// Emulator resume state written at the end of the last session.
    pub resume_state: Option<PathBuf>,
    /// ASUS power mode for this game; None follows its system.
    pub power: Option<crate::power::PowerMode>,
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
    /// Emulator process, for handing focus back after the quick menu.
    pub pid: Option<u32>,
    /// Emulator id, to pick how the quick menu pauses it.
    pub emulator: String,
    /// The process is frozen by Lumen (quick menu on an emulator that can't
    /// pause on focus loss); must be thawed before resuming or closing.
    pub suspended: bool,
    /// Lumen's extra shortcuts this game supports (rewind, discs…).
    pub controls: crate::controls::Controls,
    /// The emulator's folder (its screenshots land under it).
    pub exe_dir: PathBuf,
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
    /// Per-system game facts, loaded on first use.
    pub metadata: Mutex<HashMap<String, Arc<crate::metadata::SystemInfo>>>,
    /// RetroAchievements login. Kept in secrets.json, which isn't backed up.
    pub ra: Mutex<Option<crate::retroachievements::Account>>,
    /// The fix found for the last failed launch, with its game id.
    pub launch_fix: Mutex<Option<(String, crate::launch_check::Fix)>>,
    /// Fixes already tried per game since its last good launch (no loops).
    pub fixes_tried: Mutex<HashSet<(String, crate::launch_check::Fix)>>,
    /// The user's own game lists, as tabs in the library.
    pub collections: Mutex<Vec<Collection>>,
    /// Steam Web API key for listing owned games. Kept in steam.json, which
    /// isn't backed up.
    pub steam_key: Mutex<Option<String>>,
    /// SteamGridDB API key for covers libretro lacks; kept out of backups.
    pub sgdb_key: Mutex<Option<String>>,
}

#[derive(Serialize, Deserialize)]
struct SteamKey {
    key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    /// Game ids, in the order they were added.
    pub games: Vec<String>,
}

impl AppState {
    pub fn load(app: &AppHandle) -> anyhow::Result<Self> {
        Self::at(app.path().app_data_dir()?)
    }

    /// State rooted at `data` (the app data dir; a temp dir in tests).
    pub fn at(data: PathBuf) -> anyhow::Result<Self> {
        let dirs = Dirs {
            emulators: data.join("emulators"),
            downloads: data.join("downloads"),
            data,
        };
        fs::create_dir_all(&dirs.emulators)?;
        fs::create_dir_all(&dirs.downloads)?;

        let mut settings: Settings =
            read_json(&dirs.data.join("settings.json")).unwrap_or_default();
        // Select + Start reaches the game too (Start pauses it); on the Ally
        // the back buttons take over, once, for settings saved before them.
        if settings.combo_version < COMBO_VERSION {
            if crate::device::device().model.is_some() {
                settings.quick_combo = false;
                settings.back_buttons = true;
            }
            settings.combo_version = COMBO_VERSION;
            let _ = write_json(&dirs.data.join("settings.json"), &settings);
        }

        Ok(Self {
            settings: Mutex::new(settings),
            library: Mutex::new(read_json(&dirs.data.join("library.json")).unwrap_or_default()),
            user: Mutex::new(read_json(&dirs.data.join("userdata.json")).unwrap_or_default()),
            running: Arc::new(Mutex::new(None)),
            installing: Mutex::new(HashSet::new()),
            metadata: Mutex::new(HashMap::new()),
            ra: Mutex::new(read_json(&dirs.data.join("secrets.json"))),
            launch_fix: Mutex::new(None),
            fixes_tried: Mutex::new(HashSet::new()),
            collections: Mutex::new(
                read_json(&dirs.data.join("collections.json")).unwrap_or_default(),
            ),
            steam_key: Mutex::new(
                read_json::<SteamKey>(&dirs.data.join("steam.json")).map(|k| k.key),
            ),
            sgdb_key: Mutex::new(
                read_json::<SteamKey>(&dirs.data.join("steamgriddb.json")).map(|k| k.key),
            ),
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

    pub fn save_ra(&self) -> anyhow::Result<()> {
        let path = self.dirs.data.join("secrets.json");
        match &*self.ra.lock().unwrap() {
            Some(account) => write_json(&path, account),
            None => {
                let _ = fs::remove_file(path);
                Ok(())
            }
        }
    }

    /// Saves the Steam key, or removes the file when signed out (None).
    pub fn save_sgdb_key(&self) -> anyhow::Result<()> {
        let path = self.dirs.data.join("steamgriddb.json");
        match self.sgdb_key.lock().unwrap().clone() {
            Some(key) => write_json(&path, &SteamKey { key }),
            None => {
                let _ = fs::remove_file(path);
                Ok(())
            }
        }
    }

    pub fn save_steam_key(&self) -> anyhow::Result<()> {
        let path = self.dirs.data.join("steam.json");
        match self.steam_key.lock().unwrap().clone() {
            Some(key) => write_json(&path, &SteamKey { key }),
            None => {
                let _ = fs::remove_file(path);
                Ok(())
            }
        }
    }

    pub fn save_collections(&self) -> anyhow::Result<()> {
        write_json(
            &self.dirs.data.join("collections.json"),
            &*self.collections.lock().unwrap(),
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
