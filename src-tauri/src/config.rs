//! Writes each emulator's own config so it starts fullscreen, skips first-run
//! wizards and finds the BIOS files. Runs after install and before every launch,
//! so BIOS files added later are picked up without user action.
//! Only the keys we own are touched; everything else the user changes is kept.

use crate::library::find_file;
use crate::manifest::Emulator;
use std::{fs, path::Path};

pub fn configure(emu: &Emulator, exe_dir: &Path, bios: Option<&Path>) -> anyhow::Result<()> {
    // Appends a BIOS folder entry only when the ROM root is known.
    let with_bios = |mut entries: Vec<(&'static str, &'static str, String)>, section: &'static str, key: &'static str, sub: &str| {
        if let Some(b) = bios {
            entries.push((section, key, b.join(sub).to_string_lossy().into_owned()));
        }
        entries
    };

    match emu.id.as_str() {
        "retroarch" => {
            let mut entries = vec![
                ("video_fullscreen", "true".to_string()),
                ("pause_nonactive", "false".to_string()),
                ("quit_press_twice", "false".to_string()),
                ("menu_driver", "ozone".to_string()),
            ];
            if let Some(b) = bios {
                entries.push(("system_directory", b.to_string_lossy().into_owned()));
            }
            set_cfg(&exe_dir.join("retroarch.cfg"), &entries)
        }
        "duckstation" => set_ini(
            &exe_dir.join("settings.ini"),
            &with_bios(
                vec![
                    ("Main", "SetupWizardIncomplete", "false".into()),
                    ("Main", "StartFullscreen", "true".into()),
                    ("Main", "ConfirmPowerOff", "false".into()),
                    ("AutoUpdater", "CheckAtStartup", "false".into()),
                ],
                "BIOS",
                "SearchDirectory",
                "ps1",
            ),
        ),
        "pcsx2" => set_ini(
            &exe_dir.join("inis").join("PCSX2.ini"),
            &with_bios(
                vec![
                    ("UI", "SetupWizardIncomplete", "false".into()),
                    ("UI", "StartFullscreen", "true".into()),
                    ("UI", "ConfirmShutdown", "false".into()),
                    ("AutoUpdater", "CheckAtStartup", "false".into()),
                ],
                "Folders",
                "Bios",
                "ps2",
            ),
        ),
        "dolphin" => set_ini(
            &exe_dir.join("User").join("Config").join("Dolphin.ini"),
            &[
                ("Display", "Fullscreen", "True".into()),
                ("Interface", "ConfirmStop", "False".into()),
                ("Analytics", "PermissionAsked", "True".into()),
                ("Analytics", "Enabled", "False".into()),
            ],
        ),
        "ppsspp" => set_ini(
            &exe_dir.join("memstick").join("PSP").join("SYSTEM").join("ppsspp.ini"),
            &[("General", "FirstRun", "False".into()), ("Graphics", "FullScreen", "True".into())],
        ),
        "xemu" => {
            // xemu reads xemu.toml next to the exe (portable mode); TOML sections
            // are close enough to INI for the flat keys we set.
            let mut entries = vec![("general", "show_welcome", "false".to_string())];
            if let Some(b) = bios {
                let quote = |p: std::path::PathBuf| format!("'{}'", p.to_string_lossy());
                if let Some(p) = find_file(b, "xbox/mcpx*.bin") {
                    entries.push(("sys.files", "bootrom_path", quote(p)));
                }
                if let Some(p) = find_file(b, "xbox/*bios*.bin").or_else(|| find_file(b, "xbox/complex*.bin")) {
                    entries.push(("sys.files", "flashrom_path", quote(p)));
                }
                if let Some(p) = find_file(b, "xbox/*.qcow2") {
                    entries.push(("sys.files", "hdd_path", quote(p)));
                }
            }
            set_ini(&exe_dir.join("xemu.toml"), &entries)
        }
        "shadps4" => {
            // shadPS4 looks for LLE system modules in its portable user dir.
            if let Some(b) = bios {
                let modules = exe_dir.join("user").join("sys_modules");
                copy_missing(&b.join("ps4"), &modules, "sprx")?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Copies `*.<ext>` files from `from` into `to`, skipping ones already there.
fn copy_missing(from: &Path, to: &Path, ext: &str) -> anyhow::Result<()> {
    let Ok(entries) = fs::read_dir(from) else { return Ok(()) };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
            continue;
        }
        let target = to.join(entry.file_name());
        if !target.exists() {
            fs::create_dir_all(to)?;
            fs::copy(&path, target)?;
        }
    }
    Ok(())
}

/// RetroArch style: `key = "value"` lines, no sections.
fn set_cfg(path: &Path, entries: &[(&str, String)]) -> anyhow::Result<()> {
    let mut lines = read_lines(path);
    for (key, value) in entries {
        let line = format!("{key} = \"{value}\"");
        match lines.iter().position(|l| key_of(l).is_some_and(|k| k == *key)) {
            Some(i) => lines[i] = line,
            None => lines.push(line),
        }
    }
    write_lines(path, &lines)
}

/// `[Section]` + `Key = Value` files. Keys are matched case-insensitively.
fn set_ini(path: &Path, entries: &[(&str, &str, String)]) -> anyhow::Result<()> {
    let mut lines = read_lines(path);
    for (section, key, value) in entries {
        let header = format!("[{section}]");
        let line = format!("{key} = {value}");
        let Some(start) = lines.iter().position(|l| l.trim().eq_ignore_ascii_case(&header)) else {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(header);
            lines.push(line);
            continue;
        };
        let end = lines[start + 1..]
            .iter()
            .position(|l| l.trim_start().starts_with('['))
            .map_or(lines.len(), |p| start + 1 + p);
        match (start + 1..end).find(|&i| key_of(&lines[i]).is_some_and(|k| k.eq_ignore_ascii_case(key))) {
            Some(i) => lines[i] = line,
            None => lines.insert(end, line),
        }
    }
    write_lines(path, &lines)
}

fn key_of(line: &str) -> Option<&str> {
    line.split_once('=').map(|(k, _)| k.trim())
}

fn read_lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path).unwrap_or_default().lines().map(String::from).collect()
}

fn write_lines(path: &Path, lines: &[String]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, lines.join("\n") + "\n")?;
    Ok(())
}
