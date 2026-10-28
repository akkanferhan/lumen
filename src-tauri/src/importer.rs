//! Imports the user's own game dumps from any folder (USB stick, Downloads)
//! into the ROM folder. Each game's system is detected from its extension and,
//! where an extension is shared (iso, cue, zip…), from the file's own header.
//! Games the detector can't place are listed with their possible systems so
//! the user picks one; nothing is copied without a system.

use crate::manifest::{manifest, System};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// The game file (or folder, for PS3/PS4/PS5/Vita dumps).
    pub path: PathBuf,
    pub name: String,
    /// Detected system, if any.
    pub system_id: Option<String>,
    /// Every system whose emulator accepts this file, for the user to choose from.
    pub choices: Vec<String>,
    /// Bytes to copy, including cue tracks and playlist discs.
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub path: PathBuf,
    pub system_id: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Copy,
    Move,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub imported: usize,
    /// Already in the ROM folder under the same name.
    pub skipped: usize,
    /// "name: reason" for every game that failed.
    pub failed: Vec<String>,
}

/// Lists the games under `source`. Unrelated files (saves, readmes) are left out.
pub fn scan(source: &Path) -> Vec<Candidate> {
    let mut files = Vec::new();
    let mut out = Vec::new();
    let mut walker = WalkDir::new(source).max_depth(6).into_iter();

    while let Some(entry) = walker.next() {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if entry.file_type().is_dir() {
            if let Some(system) = folder_system(path) {
                out.push(candidate(
                    path,
                    Some(system.to_string()),
                    vec![system.to_string()],
                ));
                walker.skip_current_dir();
            }
            continue;
        }
        if !choices(path).is_empty() {
            files.push(path.to_path_buf());
        }
    }

    // Tracks of a .cue, discs of a .m3u and .gdi tracks travel with their game.
    let referenced: HashSet<PathBuf> = files
        .iter()
        .flat_map(|f| crate::library::references(f))
        .collect();
    for file in files.iter().filter(|f| !referenced.contains(*f)) {
        let choices = choices(file);
        let system = detect(file, &choices);
        out.push(candidate(file, system, choices));
    }
    out.sort_by_key(|c| c.name.to_lowercase());
    out
}

fn candidate(path: &Path, system_id: Option<String>, choices: Vec<String>) -> Candidate {
    Candidate {
        path: path.to_path_buf(),
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        system_id,
        choices,
        size: files_of(path).iter().map(|(f, _)| file_size(f)).sum(),
    }
}

/// Systems whose emulator accepts this file's extension.
fn choices(path: &Path) -> Vec<String> {
    let Some(ext) = extension(path) else {
        return Vec::new();
    };
    manifest()
        .systems
        .iter()
        .filter(|s| s.extensions.contains(&ext))
        .map(|s| s.id.clone())
        .collect()
}

/// PS3 (`PS3_GAME`), PS5 (`sce_sys/param.json`), and PS4 or Vita
/// (`sce_sys/param.sfo`, told apart by the title ID) folder dumps.
fn folder_system(dir: &Path) -> Option<&'static str> {
    if dir.join("PS3_GAME").is_dir() {
        return Some("ps3");
    }
    let sce_sys = dir.join("sce_sys");
    if !sce_sys.is_dir() {
        return None;
    }
    if sce_sys.join("param.json").is_file() {
        return Some("ps5");
    }
    let sfo = fs::read(sce_sys.join("param.sfo")).ok()?;
    let title_id = crate::library::sfo_string(&sfo, "TITLE_ID")?;
    // PS4 discs and downloads are CUSA/PLAS; Vita titles are PCSx/NPxx.
    if title_id.starts_with("CUSA") || title_id.starts_with("PLAS") {
        Some("ps4")
    } else {
        Some("psvita")
    }
}

/// The system a file belongs to, or None when the user has to choose.
fn detect(path: &Path, choices: &[String]) -> Option<String> {
    let ext = extension(path)?;
    let found = match ext.as_str() {
        "iso" => iso_system(path),
        "bin" => cartridge_system(path),
        "cue" => crate::library::references(path)
            .first()
            .and_then(|track| disc_system(track)),
        "m3u" => {
            return crate::library::references(path)
                .first()
                .and_then(|disc| detect(disc, &self::choices(disc)))
                .filter(|s| choices.contains(s))
        }
        "pbp" => pbp_system(path),
        "zip" | "7z" => archive_system(path),
        _ if choices.len() == 1 => return Some(choices[0].clone()),
        _ => None,
    };
    found.map(str::to_string).filter(|s| choices.contains(s))
}

/// GameCube, Wii, Xbox, Xbox 360, PS1, PS2, PS3 or PSP disc image.
fn iso_system(path: &Path) -> Option<&'static str> {
    let mut f = File::open(path).ok()?;
    let head = read_at(&mut f, 0, 0x20)?;
    if head[0x1C..0x20] == [0xC2, 0x33, 0x9F, 0x3D] {
        return Some("gc");
    }
    if head[0x18..0x1C] == [0x5D, 0x1C, 0x9E, 0xA3] {
        return Some("wii");
    }
    if let Some(system) = xbox_system(&mut f) {
        return Some(system);
    }
    if read_at(&mut f, 0x800, 0x10).is_some_and(|b| contains(&b, b"PlayStation3")) {
        return Some("ps3");
    }
    let pvd = read_at(&mut f, 0x8000, 0x28)?;
    if &pvd[1..6] != b"CD001" {
        return None;
    }
    let system_id = &pvd[8..0x28];
    if contains(system_id, b"PSP GAME") {
        return Some("psp");
    }
    if contains(system_id, b"PS3VOLUME") {
        return Some("ps3");
    }
    if contains(system_id, b"PLAYSTATION") {
        // SYSTEM.CNF says BOOT2 on PS2 discs and BOOT on PS1 discs; it sits
        // near the start of the disc.
        let start = read_at_most(&mut f, 0, 8 << 20);
        return Some(if contains(&start, b"BOOT2") {
            "ps2"
        } else {
            "ps1"
        });
    }
    None
}

/// Xbox and Xbox 360 discs use XDVDFS; the root directory names the boot
/// executable (default.xbe vs default.xex). Trimmed images put the volume at
/// 0x10000, full Redump-style images after a video partition.
fn xbox_system(f: &mut File) -> Option<&'static str> {
    const MAGIC: &[u8] = b"MICROSOFT*XBOX*MEDIA";
    for base in [0u64, 0x1830_0000, 0x0FD9_0000, 0x0208_0000] {
        let Some(volume) = read_at(f, base + 0x10000, 0x20) else {
            continue;
        };
        if &volume[..MAGIC.len()] != MAGIC {
            continue;
        }
        let sector = u32::from_le_bytes(volume[20..24].try_into().ok()?) as u64;
        let size = u32::from_le_bytes(volume[24..28].try_into().ok()?).min(1 << 20) as usize;
        let root = read_at(f, base + sector * 2048, size).unwrap_or_default();
        let root = root.to_ascii_lowercase();
        return Some(if contains(&root, b"default.xex") {
            "x360"
        } else if contains(&root, b"default.xbe") || base == 0x1830_0000 {
            "xbox"
        } else {
            return None;
        });
    }
    None
}

/// A disc track from a .cue: Saturn, or PS1 (raw 2352-byte or 2048-byte sectors).
fn disc_system(track: &Path) -> Option<&'static str> {
    let mut f = File::open(track).ok()?;
    let head = read_at(&mut f, 0, 0x20)?;
    if contains(&head, b"SEGA SEGASATURN") {
        return Some("saturn");
    }
    for pvd in [16 * 2352 + 24, 16 * 2048] {
        if let Some(b) = read_at(&mut f, pvd, 0x28) {
            if &b[1..6] == b"CD001" && contains(&b[8..0x28], b"PLAYSTATION") {
                return Some("ps1");
            }
        }
    }
    None
}

/// Mega Drive carts carry "SEGA" at 0x100; other .bin files are left to the user.
fn cartridge_system(path: &Path) -> Option<&'static str> {
    let mut f = File::open(path).ok()?;
    read_at(&mut f, 0x100, 4)
        .filter(|b| b == b"SEGA")
        .map(|_| "genesis")
}

/// PS1 Classics EBOOT.PBP files hold a PSISOIMG/PSTITLEI image; the rest are PSP.
fn pbp_system(path: &Path) -> Option<&'static str> {
    let mut f = File::open(path).ok()?;
    let head = read_at(&mut f, 0, 0x28)?;
    if &head[..4] != b"\0PBP" {
        return None;
    }
    let psar = u32::from_le_bytes(head[0x24..0x28].try_into().ok()?) as u64;
    let magic = read_at(&mut f, psar, 8).unwrap_or_default();
    Some(if magic == b"PSISOIMG" || magic == b"PSTITLEI" {
        "ps1"
    } else {
        "psp"
    })
}

/// A zip/7z holding one cartridge ROM belongs to that ROM's system; a Vita
/// package has `sce_sys/param.sfo`; anything else is taken as an arcade set.
fn archive_system(path: &Path) -> Option<&'static str> {
    let names: Vec<String> = if extension(path).as_deref() == Some("7z") {
        sevenz_rust::Archive::open(path)
            .ok()?
            .files
            .iter()
            .filter(|e| !e.is_directory())
            .map(|e| e.name().to_lowercase())
            .collect()
    } else {
        zip::ZipArchive::new(File::open(path).ok()?)
            .ok()?
            .file_names()
            .filter(|n| !n.ends_with('/'))
            .map(str::to_lowercase)
            .collect()
    };
    if names.iter().any(|n| n.ends_with("sce_sys/param.sfo")) {
        return Some("psvita");
    }
    let roms: Vec<&'static System> = names
        .iter()
        .filter_map(|n| {
            let ext = Path::new(n).extension()?.to_str()?;
            let systems: Vec<_> = manifest()
                .systems
                .iter()
                .filter(|s| s.extensions.iter().any(|e| e == ext) && !matches!(ext, "zip" | "7z"))
                .collect();
            (systems.len() == 1).then(|| systems[0])
        })
        .collect();
    match roms.as_slice() {
        [] if !names.is_empty() => Some("arcade"),
        [system, rest @ ..] if rest.iter().all(|s| s.id == system.id) => Some(system.id.as_str()),
        _ => None,
    }
}

/// Copies (or moves) each game into `<rom root>/<system folder>/`.
/// `progress` receives the share of bytes done, 0.0–1.0.
pub fn import(
    items: &[Item],
    rom_root: &Path,
    mode: Mode,
    mut progress: impl FnMut(f32),
) -> Report {
    let mut report = Report::default();
    let total: u64 = items
        .iter()
        .flat_map(|i| files_of(&i.path))
        .map(|(f, _)| file_size(&f))
        .sum::<u64>()
        .max(1);
    let mut done = 0u64;

    for item in items {
        let name = item
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(system) = crate::manifest::system(&item.system_id) else {
            report.failed.push(format!("{name}: bilinmeyen sistem"));
            continue;
        };
        let folder = system
            .folders
            .iter()
            .map(|f| rom_root.join(f))
            .find(|p| p.is_dir())
            .unwrap_or_else(|| rom_root.join(&system.folders[0]));
        let files = files_of(&item.path);
        if folder.join(&files[0].1).exists() {
            report.skipped += 1;
            done += files.iter().map(|(f, _)| file_size(f)).sum::<u64>();
            progress(done as f32 / total as f32);
            continue;
        }
        let result = files.iter().try_for_each(|(from, rel)| {
            transfer(from, &folder.join(rel), mode, &mut |n| {
                done += n;
                progress(done as f32 / total as f32);
            })
        });
        match result {
            Ok(()) => {
                if mode == Mode::Move && item.path.is_dir() {
                    let _ = fs::remove_dir_all(&item.path);
                }
                report.imported += 1;
            }
            Err(e) => report.failed.push(format!("{name}: {e}")),
        }
    }
    report
}

/// Every file of a game with its path relative to where the game lands:
/// a folder dump keeps its folder name, a disc keeps its tracks beside it.
fn files_of(path: &Path) -> Vec<(PathBuf, PathBuf)> {
    if path.is_dir() {
        let parent = path.parent().unwrap_or(path);
        let mut files: Vec<_> = WalkDir::new(path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| {
                let rel = e.path().strip_prefix(parent).ok()?.to_path_buf();
                Some((e.path().to_path_buf(), rel))
            })
            .collect();
        // The first entry stands for the whole game: the folder itself, which
        // `transfer` skips and `import` checks for an earlier copy.
        let folder = path.file_name().map(PathBuf::from).unwrap_or_default();
        files.insert(0, (path.to_path_buf(), folder));
        return files;
    }
    let dir = path.parent().unwrap_or(Path::new(""));
    crate::library::game_files(path)
        .into_iter()
        .map(|f| {
            let rel = f
                .strip_prefix(dir)
                .map(Path::to_path_buf)
                .unwrap_or_else(|_| f.file_name().map(PathBuf::from).unwrap_or_default());
            (f, rel)
        })
        .collect()
}

fn transfer(
    from: &Path,
    to: &Path,
    mode: Mode,
    on_bytes: &mut dyn FnMut(u64),
) -> std::io::Result<()> {
    if from.is_dir() {
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    let size = file_size(from);
    // Same drive: a move is instant.
    if mode == Mode::Move && fs::rename(from, to).is_ok() {
        on_bytes(size);
        return Ok(());
    }
    // Copy into a temporary name so a cancelled copy never looks like a game.
    let partial = to.with_extension(format!(
        "{}.part",
        to.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    let copied = (|| {
        let mut src = File::open(from)?;
        let mut dst = File::create(&partial)?;
        let mut buf = vec![0u8; 4 << 20];
        loop {
            let n = src.read(&mut buf)?;
            if n == 0 {
                break;
            }
            dst.write_all(&buf[..n])?;
            on_bytes(n as u64);
        }
        dst.flush()
    })();
    if let Err(e) = copied {
        let _ = fs::remove_file(&partial);
        return Err(e);
    }
    fs::rename(&partial, to)?;
    if mode == Mode::Move {
        fs::remove_file(from)?;
    }
    Ok(())
}

fn read_at(f: &mut File, offset: u64, len: usize) -> Option<Vec<u8>> {
    f.seek(SeekFrom::Start(offset)).ok()?;
    let mut buf = vec![0u8; len];
    f.read_exact(&mut buf).ok()?;
    Some(buf)
}

fn read_at_most(f: &mut File, offset: u64, len: u64) -> Vec<u8> {
    let mut buf = Vec::new();
    if f.seek(SeekFrom::Start(offset)).is_ok() {
        let _ = f.take(len).read_to_end(&mut buf);
    }
    buf
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path)
        .ok()
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .unwrap_or(0)
}

fn extension(path: &Path) -> Option<String> {
    path.extension().map(|e| e.to_string_lossy().to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lumen-import-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A zero-filled file with `patches` written at their offsets.
    fn image(path: &Path, len: usize, patches: &[(usize, &[u8])]) {
        let mut data = vec![0u8; len];
        for (offset, bytes) in patches {
            data[*offset..offset + bytes.len()].copy_from_slice(bytes);
        }
        fs::write(path, data).unwrap();
    }

    fn pvd(system: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; 0x28];
        b[0] = 1;
        b[1..6].copy_from_slice(b"CD001");
        b[8..8 + system.len()].copy_from_slice(system);
        b
    }

    fn sfo(title_id: &str) -> Vec<u8> {
        let key = b"TITLE_ID\0";
        let value = format!("{title_id}\0");
        let (key_table, data_table) = (20 + 16, 20 + 16 + key.len());
        let mut sfo = b"\0PSF".to_vec();
        sfo.extend_from_slice(&0x0101u32.to_le_bytes());
        sfo.extend_from_slice(&(key_table as u32).to_le_bytes());
        sfo.extend_from_slice(&(data_table as u32).to_le_bytes());
        sfo.extend_from_slice(&1u32.to_le_bytes());
        sfo.extend_from_slice(&0u16.to_le_bytes());
        sfo.extend_from_slice(&0x0204u16.to_le_bytes());
        sfo.extend_from_slice(&(value.len() as u32).to_le_bytes());
        sfo.extend_from_slice(&(value.len() as u32).to_le_bytes());
        sfo.extend_from_slice(&0u32.to_le_bytes());
        sfo.extend_from_slice(key);
        sfo.extend_from_slice(value.as_bytes());
        sfo
    }

    fn detected(dir: &Path, name: &str) -> Option<String> {
        scan(dir)
            .into_iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("{name} not listed"))
            .system_id
    }

    #[test]
    fn detects_systems_from_headers() {
        let dir = temp_dir("detect");
        image(
            &dir.join("cube.iso"),
            0x9000,
            &[(0x1C, &[0xC2, 0x33, 0x9F, 0x3D])],
        );
        image(
            &dir.join("wii.iso"),
            0x9000,
            &[(0x18, &[0x5D, 0x1C, 0x9E, 0xA3])],
        );
        image(&dir.join("psp.iso"), 0x9000, &[(0x8000, &pvd(b"PSP GAME"))]);
        image(
            &dir.join("ps2.iso"),
            0xA000,
            &[(0x8000, &pvd(b"PLAYSTATION")), (0x9000, b"BOOT2 = cdrom0:")],
        );
        image(
            &dir.join("ps1.iso"),
            0x9000,
            &[(0x8000, &pvd(b"PLAYSTATION"))],
        );
        // Xbox 360: XDVDFS volume at 0x10000 whose root (sector 33) names default.xex.
        let mut volume = b"MICROSOFT*XBOX*MEDIA".to_vec();
        volume.extend_from_slice(&33u32.to_le_bytes());
        volume.extend_from_slice(&64u32.to_le_bytes());
        image(
            &dir.join("x360.iso"),
            0x11000,
            &[(0x10000, &volume), (33 * 2048 + 14, b"default.xex")],
        );
        image(
            &dir.join("sonic.bin"),
            0x200,
            &[(0x100, b"SEGA MEGA DRIVE")],
        );
        image(&dir.join("mystery.bin"), 0x200, &[]);
        image(&dir.join("game.sfc"), 16, &[]);
        image(&dir.join("disc.chd"), 16, &[]);

        image(
            &dir.join("ps1 (Track 1).bin"),
            0xA000,
            &[(16 * 2352 + 24, &pvd(b"PLAYSTATION"))],
        );
        fs::write(dir.join("ps1.cue"), "FILE \"ps1 (Track 1).bin\" BINARY\n").unwrap();
        image(
            &dir.join("saturn.bin"),
            0x100,
            &[(0x10, b"SEGA SEGASATURN")],
        );
        fs::write(dir.join("saturn.cue"), "FILE \"saturn.bin\" BINARY\n").unwrap();

        let mut pbp = vec![0u8; 0x40];
        pbp[..4].copy_from_slice(b"\0PBP");
        pbp[0x24..0x28].copy_from_slice(&0x30u32.to_le_bytes());
        pbp[0x30..0x38].copy_from_slice(b"PSISOIMG");
        fs::write(dir.join("EBOOT.PBP"), pbp).unwrap();

        let mut zip = zip::ZipWriter::new(File::create(dir.join("mario.zip")).unwrap());
        zip.start_file::<_, ()>("Mario (USA).sfc", Default::default())
            .unwrap();
        zip.write_all(b"rom").unwrap();
        zip.finish().unwrap();
        let mut zip = zip::ZipWriter::new(File::create(dir.join("pacman.zip")).unwrap());
        zip.start_file::<_, ()>("pacman.6e", Default::default())
            .unwrap();
        zip.write_all(b"rom").unwrap();
        zip.finish().unwrap();

        let ps5 = dir.join("PPSA01234/sce_sys");
        fs::create_dir_all(&ps5).unwrap();
        fs::write(ps5.join("param.json"), "{}").unwrap();
        let ps4 = dir.join("CUSA00552/sce_sys");
        fs::create_dir_all(&ps4).unwrap();
        fs::write(ps4.join("param.sfo"), sfo("CUSA00552")).unwrap();
        let vita = dir.join("PCSE00000/sce_sys");
        fs::create_dir_all(&vita).unwrap();
        fs::write(vita.join("param.sfo"), sfo("PCSE00000")).unwrap();
        fs::write(dir.join("readme.txt"), "not a game").unwrap();

        let expect = [
            ("cube.iso", Some("gc")),
            ("wii.iso", Some("wii")),
            ("psp.iso", Some("psp")),
            ("ps2.iso", Some("ps2")),
            ("ps1.iso", Some("ps1")),
            ("x360.iso", Some("x360")),
            ("sonic.bin", Some("genesis")),
            ("mystery.bin", None),
            ("game.sfc", Some("snes")),
            ("disc.chd", None),
            ("ps1.cue", Some("ps1")),
            ("saturn.cue", Some("saturn")),
            ("EBOOT.PBP", Some("ps1")),
            ("mario.zip", Some("snes")),
            ("pacman.zip", Some("arcade")),
            ("PPSA01234", Some("ps5")),
            ("CUSA00552", Some("ps4")),
            ("PCSE00000", Some("psvita")),
        ];
        for (name, system) in expect {
            assert_eq!(detected(&dir, name).as_deref(), system, "{name}");
        }
        let all = scan(&dir);
        assert!(all.iter().all(|c| c.name != "readme.txt"));
        // Cue tracks are part of their game, not games of their own.
        assert!(all
            .iter()
            .all(|c| c.name != "ps1 (Track 1).bin" && c.name != "saturn.bin"));
        let chd = all.iter().find(|c| c.name == "disc.chd").unwrap();
        assert!(
            chd.choices.contains(&"ps2".to_string())
                && chd.choices.contains(&"dreamcast".to_string())
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn imports_games_with_their_tracks_and_skips_existing_ones() {
        let source = temp_dir("source");
        let root = temp_dir("root");
        fs::write(source.join("game.cue"), "FILE \"tracks/game.bin\" BINARY\n").unwrap();
        fs::create_dir_all(source.join("tracks")).unwrap();
        fs::write(source.join("tracks/game.bin"), vec![7u8; 10_000]).unwrap();
        fs::write(source.join("zelda.sfc"), b"rom").unwrap();
        fs::create_dir_all(source.join("PPSA01234/sce_sys")).unwrap();
        fs::write(source.join("PPSA01234/sce_sys/param.json"), "{}").unwrap();
        fs::write(source.join("PPSA01234/eboot.bin"), b"elf").unwrap();
        // An existing "superfamicom" folder is reused instead of creating "snes".
        fs::create_dir_all(root.join("superfamicom")).unwrap();

        let items = vec![
            Item {
                path: source.join("game.cue"),
                system_id: "ps1".into(),
            },
            Item {
                path: source.join("zelda.sfc"),
                system_id: "snes".into(),
            },
            Item {
                path: source.join("PPSA01234"),
                system_id: "ps5".into(),
            },
        ];
        let mut last = 0.0;
        let report = import(&items, &root, Mode::Copy, |p| last = p);
        assert_eq!(
            (report.imported, report.skipped),
            (3, 0),
            "{:?}",
            report.failed
        );
        assert!((last - 1.0).abs() < f32::EPSILON);
        assert_eq!(
            fs::read(root.join("ps1/tracks/game.bin")).unwrap().len(),
            10_000
        );
        assert!(root.join("ps1/game.cue").is_file());
        assert!(root.join("superfamicom/zelda.sfc").is_file());
        assert!(root.join("ps5/PPSA01234/eboot.bin").is_file());
        assert!(source.join("zelda.sfc").exists(), "copy keeps the source");

        let again = import(&items[1..2], &root, Mode::Move, |_| {});
        assert_eq!((again.imported, again.skipped), (0, 1));

        fs::write(source.join("metroid.sfc"), b"rom").unwrap();
        let moved = import(
            &[Item {
                path: source.join("metroid.sfc"),
                system_id: "snes".into(),
            }],
            &root,
            Mode::Move,
            |_| {},
        );
        assert_eq!(moved.imported, 1);
        assert!(!source.join("metroid.sfc").exists());
        assert!(root.join("superfamicom/metroid.sfc").is_file());
        fs::remove_dir_all(&source).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }
}
