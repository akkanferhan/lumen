//! Checks BIOS files beyond "a file with that name exists". Every BIOS has a
//! fixed size, so a wrong size means a damaged or wrong file and it doesn't
//! count; a known MD5 confirms the dump. An unknown MD5 at the right size is
//! just a version not on the list and is used as is. Hashes are cached by
//! path, size and modification time.

use md5::{Digest, Md5};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::SystemTime,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    /// A known good dump.
    Verified,
    /// Right size, version not on the list.
    Unknown,
    /// Wrong size: damaged, truncated or not a BIOS.
    WrongSize,
}

/// Expected sizes in bytes for files matching a requirement pattern.
fn sizes(pattern: &str) -> &'static [u64] {
    let p = pattern.to_ascii_lowercase();
    if p.starts_with("gba_bios") {
        &[16 * 1024]
    } else if p.starts_with("sega_101") || p.starts_with("mpr-17933") || p.starts_with("ps1/") {
        &[512 * 1024]
    } else if p.starts_with("dc/dc_boot") {
        &[2 * 1024 * 1024]
    } else if p.starts_with("ps2/") {
        &[4 * 1024 * 1024]
    } else if p.starts_with("xbox/mcpx") {
        &[512]
    } else if p.starts_with("xbox/") {
        &[256 * 1024, 1024 * 1024]
    } else {
        &[]
    }
}

/// Known good dumps (libretro's BIOS documentation).
const KNOWN: &[&str] = &[
    "a860e8c0b6d573d191e4ec7db1b1e4f6", // gba_bios.bin
    "85ec9ca47d8f6807718151cbcca8b964", // sega_101.bin
    "3240872c70984b6cbfda1586cab68dbe", // mpr-17933.bin
    "e10c53c2f8b90bab96ead2d368858623", // dc_boot.bin
    "8dd7d5296a650fac7319bce665a6a53c", // scph5500.bin
    "490f666e1afb15b7362b406ed1cea246", // scph5501.bin
    "32736f17079d0b2b7024407c39bd3050", // scph5502.bin
    "924e392ed05558ffdb115408c263dccf", // scph1001.bin
    "1e68c231d0896b7eadcad1d7d8e76129", // scph7001.bin
    "6e3735ff4c7dc899ee98981385f6f3d0", // scph101.bin
    "d49c52a4102f6df7bcf8d0617ac475ed", // mcpx_1.0.bin
];

type Key = (PathBuf, u64, Option<SystemTime>);

fn md5_of(path: &Path, len: u64, modified: Option<SystemTime>) -> Option<String> {
    static CACHE: OnceLock<Mutex<HashMap<Key, String>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (path.to_path_buf(), len, modified);
    if let Some(hash) = cache.lock().unwrap().get(&key) {
        return Some(hash.clone());
    }
    let data = std::fs::read(path).ok()?;
    let hash: String = Md5::digest(&data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    cache.lock().unwrap().insert(key, hash.clone());
    Some(hash)
}

pub fn check(path: &Path, pattern: &str) -> Verdict {
    let Ok(meta) = std::fs::metadata(path) else {
        return Verdict::WrongSize;
    };
    let expected = sizes(pattern);
    if !expected.is_empty() && !expected.contains(&meta.len()) {
        return Verdict::WrongSize;
    }
    match md5_of(path, meta.len(), meta.modified().ok()) {
        Some(hash) if KNOWN.contains(&hash.as_str()) => Verdict::Verified,
        _ => Verdict::Unknown,
    }
}

/// The best file among those matching the requirement: a verified one, else
/// any of the right size; a wrong-size file only if nothing else exists.
pub fn best(candidates: &[(PathBuf, String)]) -> Option<(&Path, Verdict)> {
    let mut checked: Vec<(&Path, Verdict)> = candidates
        .iter()
        .map(|(path, pattern)| (path.as_path(), check(path, pattern)))
        .collect();
    checked.sort_by_key(|(_, v)| match v {
        Verdict::Verified => 0,
        Verdict::Unknown => 1,
        Verdict::WrongSize => 2,
    });
    checked.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_decides_damaged_and_md5_confirms() {
        let dir = std::env::temp_dir().join(format!("lumen-bios-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("ps1")).unwrap();
        let short = dir.join("ps1/half.bin");
        std::fs::write(&short, vec![0u8; 1000]).unwrap();
        assert_eq!(check(&short, "ps1/*.bin"), Verdict::WrongSize);
        let right = dir.join("ps1/other.bin");
        std::fs::write(&right, vec![1u8; 512 * 1024]).unwrap();
        assert_eq!(check(&right, "ps1/*.bin"), Verdict::Unknown);
        let candidates = vec![
            (short.clone(), "ps1/*.bin".to_string()),
            (right.clone(), "ps1/*.bin".to_string()),
        ];
        assert_eq!(
            best(&candidates).unwrap(),
            (right.as_path(), Verdict::Unknown)
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
