//! Free games: homebrew whose licenses allow redistribution (Homebrew Hub and
//! authors' own releases) and classics their rights holders made free (see
//! scripts/homebrew-catalog.mjs). Downloads go straight into the system's ROM
//! folder; only URLs from this embedded catalog are allowed.

use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Homebrew {
    pub slug: String,
    pub title: String,
    pub system: String,
    pub developer: String,
    pub year: Option<String>,
    pub license: String,
    pub description: String,
    pub url: String,
    pub file_name: String,
    pub cover: Option<String>,
    pub source: Option<String>,
    /// "homebrew", or "classic" for commercial-era games made free by their rights holder.
    #[serde(default = "default_collection")]
    pub collection: String,
    /// The download is an archive holding `file_name` (e.g. a cue with its tracks).
    #[serde(default)]
    pub extract: bool,
    /// For `extract`: where `file_name` sits inside the archive, if in a subfolder.
    #[serde(default)]
    pub archive_path: Option<String>,
}

fn default_collection() -> String {
    "homebrew".into()
}

pub fn catalog() -> &'static [Homebrew] {
    static CATALOG: OnceLock<Vec<Homebrew>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../homebrew.json")).expect("homebrew.json is invalid")
    })
}

pub fn find(slug: &str) -> Option<&'static Homebrew> {
    catalog().iter().find(|g| g.slug == slug)
}

/// The catalog entry a library game was installed from, matched by system and file name.
pub fn for_game(system_id: &str, path: &Path) -> Option<&'static Homebrew> {
    let name = path.file_name()?;
    catalog()
        .iter()
        .find(|g| g.system == system_id && Path::new(&g.file_name).file_name() == Some(name))
}

/// Where the game lands: `<rom root>/<system folder>/<file>`, or
/// `<system folder>/<slug>/<file>` for games unpacked from an archive.
pub fn target(game: &Homebrew, rom_root: &Path) -> Option<PathBuf> {
    let system = crate::manifest::system(&game.system)?;
    // Never trust a catalog file name with path parts.
    let name = Path::new(&game.file_name).file_name()?;
    let inner = match &game.archive_path {
        // Relative paths inside the unpacked folder only.
        Some(p) => {
            let p = Path::new(p);
            p.components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
                .then(|| p.to_path_buf())?
        }
        None => PathBuf::from(name),
    };
    let folder = system
        .folders
        .iter()
        .map(|f| rom_root.join(f))
        .find(|p| p.is_dir())
        .unwrap_or_else(|| rom_root.join(&system.folders[0]));
    Some(if game.extract {
        folder.join(&game.slug).join(inner)
    } else {
        folder.join(name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_valid_and_points_at_known_systems() {
        assert!(!catalog().is_empty());
        let mut slugs: Vec<_> = catalog().iter().map(|g| &g.slug).collect();
        slugs.sort();
        let total = slugs.len();
        slugs.dedup();
        assert_eq!(slugs.len(), total, "duplicate slugs in homebrew.json");
        for game in catalog() {
            assert!(
                crate::manifest::system(&game.system).is_some(),
                "{}",
                game.slug
            );
            assert!(game.url.starts_with("https://"), "{}", game.slug);
            assert!(
                game.collection == "homebrew" || game.collection == "classic",
                "{}",
                game.slug
            );
            assert!(
                game.archive_path.is_none() || game.extract,
                "{}: archivePath without extract",
                game.slug
            );
            let rom_root = Path::new("roms");
            let target = target(game, rom_root).unwrap_or_else(|| panic!("{}", game.slug));
            assert!(
                target.starts_with(rom_root),
                "{} escapes the ROM folder",
                game.slug
            );
            assert!(!game.license.is_empty(), "{} has no license", game.slug);
            let ext = Path::new(&game.file_name)
                .extension()
                .unwrap()
                .to_string_lossy()
                .to_lowercase();
            let system = crate::manifest::system(&game.system).unwrap();
            assert!(
                system.extensions.contains(&ext),
                "{}: .{ext} not scanned for {}",
                game.slug,
                game.system
            );
        }
    }

    #[test]
    fn target_stays_inside_the_system_folder() {
        let mut game = catalog()[0].clone();
        game.file_name = "../../evil.gb".into();
        let path = target(&game, Path::new("/roms")).unwrap();
        assert_eq!(path.file_name().unwrap(), "evil.gb");
        assert!(path.starts_with("/roms"));

        game.extract = true;
        game.archive_path = Some("Game (v1)/game.sfc".into());
        let path = target(&game, Path::new("/roms")).unwrap();
        assert!(path.ends_with(format!("{}/Game (v1)/game.sfc", game.slug)));
        game.archive_path = Some("../../evil.sfc".into());
        assert_eq!(target(&game, Path::new("/roms")), None);
    }
}
