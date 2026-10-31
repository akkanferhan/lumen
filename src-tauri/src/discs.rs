//! Multi-disc games. Discs named "Game (USA) (Disc 1).cue", "(Disc 2)"…
//! become one game: Lumen writes `Game (USA).m3u` listing them, which the
//! library shows instead of the discs and which lets the emulator switch
//! discs (the quick menu's "Diski değiştir"). Only for systems whose
//! emulators open `.m3u` playlists (PS1, Saturn, Dreamcast).

use regex::Regex;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

/// A playlist Lumen just wrote, with the discs it replaces in the library.
#[derive(Debug)]
pub struct Created {
    pub playlist: PathBuf,
    pub discs: Vec<PathBuf>,
}

/// "Name (USA) (Disc 2)" → ("Name (USA)", 2); also "Disk", "CD" and "(Disc 2 of 3)".
fn disc_number(stem: &str) -> Option<(String, u32)> {
    static DISC: OnceLock<Regex> = OnceLock::new();
    let re = DISC.get_or_init(|| {
        Regex::new(r"(?i)^(.*?)\s*[\(\[](?:disc|disk|cd)\s*(\d+)(?:\s*of\s*\d+)?[\)\]](.*)$")
            .unwrap()
    });
    let caps = re.captures(stem)?;
    let title = format!("{}{}", &caps[1], &caps[3]);
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    Some((title, caps[2].parse().ok()?))
}

/// Writes a playlist for every set of two or more discs in `dir` (one
/// system's folder, searched like the library does) that has none yet.
pub fn make_playlists(dir: &Path, extensions: &[String]) -> Vec<Created> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(dir)
        .min_depth(1)
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let ext = entry
            .path()
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase());
        if ext.is_some_and(|e| e != "m3u" && extensions.contains(&e)) {
            files.push(entry.into_path());
        }
    }
    // Discs some playlist already lists (the user's own, or one from before).
    let playlists: Vec<PathBuf> = walkdir::WalkDir::new(dir)
        .min_depth(1)
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .map(|e| e.into_path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("m3u")))
        .collect();
    let listed: Vec<PathBuf> = playlists
        .iter()
        .flat_map(|p| crate::library::references(p))
        .collect();

    // (folder, title) → disc number → file
    let mut sets: BTreeMap<(PathBuf, String), BTreeMap<u32, PathBuf>> = BTreeMap::new();
    for file in files.iter().filter(|f| !listed.contains(f)) {
        let Some(stem) = file.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
            continue;
        };
        let Some((title, number)) = disc_number(&stem) else {
            continue;
        };
        let folder = file.parent().unwrap_or(dir).to_path_buf();
        sets.entry((folder, title))
            .or_default()
            .insert(number, file.clone());
    }

    let mut created = Vec::new();
    for ((folder, title), discs) in sets {
        if discs.len() < 2 {
            continue;
        }
        let playlist = folder.join(format!("{title}.m3u"));
        if playlist.exists() {
            continue;
        }
        let lines: Vec<String> = discs
            .values()
            .filter_map(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        if fs::write(&playlist, lines.join("\n") + "\n").is_ok() {
            created.push(Created {
                playlist,
                discs: discs.into_values().collect(),
            });
        }
    }
    created
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_disc_numbers() {
        assert_eq!(
            disc_number("Final Fantasy VII (USA) (Disc 2)"),
            Some(("Final Fantasy VII (USA)".into(), 2))
        );
        assert_eq!(
            disc_number("Metal Gear Solid (Europe) (Disc 1 of 2) (Rev 1)"),
            Some(("Metal Gear Solid (Europe) (Rev 1)".into(), 1))
        );
        assert_eq!(disc_number("Shenmue [CD3]"), Some(("Shenmue".into(), 3)));
        assert_eq!(disc_number("Crash Bandicoot (USA)"), None);
    }

    #[test]
    fn writes_one_playlist_per_set() {
        let dir = std::env::temp_dir().join(format!("lumen-discs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for name in [
            "FF7 (USA) (Disc 2).cue",
            "FF7 (USA) (Disc 1).cue",
            "FF7 (USA) (Disc 3).cue",
            "Lone (Disc 1).chd",
            "Crash (USA).cue",
        ] {
            fs::write(dir.join(name), "").unwrap();
        }
        let exts = vec!["cue".to_string(), "chd".to_string(), "m3u".to_string()];

        let created = make_playlists(&dir, &exts);
        assert_eq!(created.len(), 1, "{created:?}");
        assert_eq!(created[0].playlist, dir.join("FF7 (USA).m3u"));
        assert_eq!(
            fs::read_to_string(dir.join("FF7 (USA).m3u")).unwrap(),
            "FF7 (USA) (Disc 1).cue\nFF7 (USA) (Disc 2).cue\nFF7 (USA) (Disc 3).cue\n"
        );
        // Already listed: nothing new the second time.
        assert!(make_playlists(&dir, &exts).is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }
}
