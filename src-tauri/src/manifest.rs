//! Static catalog of supported systems and the emulators that run them.
//! The same JSON is imported by the frontend, so it is the single source of truth.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub systems: Vec<System>,
    pub emulators: Vec<Emulator>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct System {
    pub id: String,
    pub name: String,
    pub short: String,
    pub maker: String,
    pub year: u16,
    pub accent: String,
    pub extensions: Vec<String>,
    /// Folder names (lowercase) under the ROM root that belong to this system.
    /// The first one is created during setup.
    pub folders: Vec<String>,
    pub emulator: String,
    /// libretro core name when `emulator` is RetroArch.
    #[serde(default)]
    pub core: Option<String>,
    /// System name used by thumbnails.libretro.com.
    pub thumbnails: String,
    #[serde(default)]
    pub bios: Vec<BiosRequirement>,
    /// A directory containing this child is a game by itself (e.g. PS3 `PS3_GAME`).
    #[serde(default)]
    pub folder_marker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BiosRequirement {
    pub label: String,
    /// Glob patterns relative to the BIOS directory; any match satisfies it.
    pub any_of: Vec<String>,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Emulator {
    pub id: String,
    pub name: String,
    pub source: Source,
    /// Glob patterns (relative to the install dir) to locate the executable, in priority order.
    pub exe: Vec<String>,
    /// Launch arguments; `{rom}` and `{core}` are substituted.
    pub args: Vec<String>,
    /// Empty marker files that switch the emulator into portable mode.
    #[serde(default)]
    pub portable_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Source {
    Url {
        url: String,
    },
    Github {
        repo: String,
        /// Regex matched against release asset names.
        asset: String,
        /// Release tag; latest release when omitted.
        #[serde(default)]
        tag: Option<String>,
    },
}

pub fn manifest() -> &'static Manifest {
    static MANIFEST: OnceLock<Manifest> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        serde_json::from_str(include_str!("../emulators.json")).expect("emulators.json is invalid")
    })
}

pub fn system(id: &str) -> Option<&'static System> {
    manifest().systems.iter().find(|s| s.id == id)
}

pub fn emulator(id: &str) -> Option<&'static Emulator> {
    manifest().emulators.iter().find(|e| e.id == id)
}

/// Directories inside the BIOS folder that standalone emulators point at.
pub const BIOS_SUBDIRS: &[&str] = &["ps1", "ps2", "ps3", "xbox", "dc"];
