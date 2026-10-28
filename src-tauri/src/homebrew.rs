//! Free homebrew games whose licenses allow redistribution, from Homebrew Hub
//! (see scripts/homebrew-catalog.mjs). Downloads go straight into the
//! system's ROM folder; only URLs from this embedded catalog are allowed.

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

/// Where the game lands: `<rom root>/<system folder>/<file>`.
pub fn target(game: &Homebrew, rom_root: &Path) -> Option<PathBuf> {
    let system = crate::manifest::system(&game.system)?;
    // Never trust a catalog file name with path parts.
    let name = Path::new(&game.file_name).file_name()?;
    let folder = system
        .folders
        .iter()
        .map(|f| rom_root.join(f))
        .find(|p| p.is_dir())
        .unwrap_or_else(|| rom_root.join(&system.folders[0]));
    Some(folder.join(name))
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
            assert!(
                game.url.starts_with("https://hh3.gbdev.io/static/"),
                "{}",
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
    }
}
