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
    /// For folder games: the file inside the folder handed to the emulator.
    #[serde(default)]
    pub launch_file: Option<String>,
    /// Runs the system without the user's BIOS (open-source HLE BIOS) when a
    /// required BIOS file for the main emulator is missing.
    #[serde(default)]
    pub fallback: Option<Runner>,
    /// ASUS power mode the system needs: silent for 2D, turbo for 6th/7th gen.
    #[serde(default = "default_power")]
    pub power: crate::power::PowerMode,
}

fn default_power() -> crate::power::PowerMode {
    crate::power::PowerMode::Performance
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Runner {
    pub emulator: String,
    #[serde(default)]
    pub core: Option<String>,
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
    /// Projects that publish outside GitHub: the first `pattern` match in the
    /// fetched page (HTML or JSON) is the download link.
    Page {
        page: String,
        pattern: String,
        /// Prepended when the match is a relative path.
        #[serde(default)]
        base: Option<String>,
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
pub const BIOS_SUBDIRS: &[&str] = &["ps1", "ps2", "ps3", "ps4", "xbox", "dc"];

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    /// Real Windows asset names (Oct 2026). If a project renames its files,
    /// update the fixture and the pattern together.
    const GITHUB_ASSETS: &[(&str, &str)] = &[
        ("duckstation", "duckstation-windows-x64-release.zip"),
        ("pcsx2", "pcsx2-v2.8.2-windows-x64-Qt.7z"),
        ("melonds", "melonDS-1.1-windows-x86_64.zip"),
        ("azahar", "azahar-windows-msvc-2126.2.zip"),
        ("cemu", "cemu-2.6-windows-x64.zip"),
        ("rpcs3", "rpcs3-v0.0.43-20412-744126ca_win64_msvc.7z"),
        ("shadps4", "shadps4-win64-sdl-2026-0.20.0.zip"),
        ("sharpemu", "sharpemu-0.0.6-mono-win-x64.zip"),
        ("xemu", "xemu-win-x86_64-release.zip"),
        ("xenia", "xenia_canary_windows_.zip"),
        ("vita3k", "windows-latest.zip"),
    ];

    /// Assets in the same releases that must not be picked.
    const WRONG_ASSETS: &[(&str, &str)] = &[
        ("duckstation", "duckstation-windows-arm64-release.zip"),
        ("pcsx2", "pcsx2-v2.8.2-windows-x64-Qt-symbols.7z"),
        ("melonds", "melonDS-1.1-windows-aarch64.zip"),
        ("azahar", "azahar-libretro-windows-x86_64-2126.2.zip"),
        ("rpcs3", "rpcs3-v0.0.43-20412-744126ca_win64_msvc.7z.sha256"),
        ("shadps4", "shadps4-linux-sdl-0.20.0.zip"),
        ("sharpemu", "sharpemu-0.0.6-mono-linux-x64.tar.gz"),
        ("sharpemu", "sharpemu-0.0.6-mono-osx-x64.tar.gz"),
        ("xemu", "xemu-win-aarch64-release.zip"),
        ("vita3k", "windows-arm64-latest.zip"),
    ];

    fn asset_regex(id: &str) -> Regex {
        match &emulator(id)
            .unwrap_or_else(|| panic!("unknown emulator {id}"))
            .source
        {
            Source::Github { asset, .. } => Regex::new(asset).unwrap(),
            other => panic!("{id} is not a GitHub source: {other:?}"),
        }
    }

    #[test]
    fn every_system_points_at_a_known_emulator() {
        for system in &manifest().systems {
            assert!(
                emulator(&system.emulator).is_some(),
                "{} → {}",
                system.id,
                system.emulator
            );
            assert!(!system.folders.is_empty(), "{} has no folders", system.id);
        }
    }

    #[test]
    fn all_patterns_compile() {
        for emu in &manifest().emulators {
            match &emu.source {
                Source::Github { asset, .. } => assert!(Regex::new(asset).is_ok(), "{}", emu.id),
                Source::Page { pattern, .. } => assert!(Regex::new(pattern).is_ok(), "{}", emu.id),
                Source::Url { .. } => {}
            }
        }
    }

    #[test]
    fn github_patterns_pick_the_windows_build() {
        for (id, name) in GITHUB_ASSETS {
            assert!(asset_regex(id).is_match(name), "{id} should match {name}");
        }
        for (id, name) in WRONG_ASSETS {
            assert!(
                !asset_regex(id).is_match(name),
                "{id} should not match {name}"
            );
        }
    }

    #[test]
    fn page_patterns_find_links() {
        let dolphin = r#"{"artifacts":[{"system":"Windows x64","url":"https://dl.dolphin-emu.org/releases/2609a/dolphin-2609a-x64.7z"},{"url":"https://dl.dolphin-emu.org/releases/2609a/dolphin-2609a-ARM64.7z"}]}"#;
        let ppsspp = r#"<a href="/files/1_20_4/ppsspp_win.zip">Download</a>"#;
        for (id, page, expected) in [
            (
                "dolphin",
                dolphin,
                "https://dl.dolphin-emu.org/releases/2609a/dolphin-2609a-x64.7z",
            ),
            ("ppsspp", ppsspp, "/files/1_20_4/ppsspp_win.zip"),
        ] {
            let Source::Page { pattern, .. } = &emulator(id).unwrap().source else {
                panic!("{id}")
            };
            assert_eq!(
                Regex::new(pattern).unwrap().find(page).map(|m| m.as_str()),
                Some(expected)
            );
        }
    }
}
