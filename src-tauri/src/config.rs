//! Writes each emulator's own config so it starts fullscreen, skips first-run
//! wizards and finds the BIOS files. Runs after install and before every launch,
//! so BIOS files added later are picked up without user action.
//! Only the keys we own are touched; everything else the user changes is kept.

use crate::library::find_file;
use crate::manifest::Emulator;
use std::{fs, path::Path};

pub fn configure(emu: &Emulator, exe_dir: &Path, bios: Option<&Path>) -> anyhow::Result<()> {
    // Appends a BIOS folder entry only when the ROM root is known.
    let with_bios = |mut entries: Vec<(&'static str, &'static str, String)>,
                     section: &'static str,
                     key: &'static str,
                     sub: &str| {
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
        "duckstation" => {
            let ini = exe_dir.join("settings.ini");
            add_section_if_missing(&ini, "Pad1", &sdl_dualshock("AnalogController"))?;
            set_ini(
                &ini,
                &with_bios(
                    vec![
                        ("Main", "SetupWizardIncomplete", "false".into()),
                        ("Main", "StartFullscreen", "true".into()),
                        ("Main", "ConfirmPowerOff", "false".into()),
                        ("AutoUpdater", "CheckAtStartup", "false".into()),
                        ("InputSources", "SDL", "true".into()),
                    ],
                    "BIOS",
                    "SearchDirectory",
                    "ps1",
                ),
            )
        }
        "pcsx2" => {
            let ini = exe_dir.join("inis").join("PCSX2.ini");
            add_section_if_missing(&ini, "Pad1", &sdl_dualshock("DualShock2"))?;
            set_ini(
                &ini,
                &with_bios(
                    vec![
                        ("UI", "SetupWizardIncomplete", "false".into()),
                        ("UI", "StartFullscreen", "true".into()),
                        ("UI", "ConfirmShutdown", "false".into()),
                        ("AutoUpdater", "CheckAtStartup", "false".into()),
                        ("InputSources", "SDL", "true".into()),
                    ],
                    "Folders",
                    "Bios",
                    "ps2",
                ),
            )
        }
        "dolphin" => {
            let config = exe_dir.join("User").join("Config");
            add_section_if_missing(&config.join("GCPadNew.ini"), "GCPad1", &xinput_gcpad())?;
            set_ini(
                &config.join("Dolphin.ini"),
                &[
                    ("Display", "Fullscreen", "True".into()),
                    ("Interface", "ConfirmStop", "False".into()),
                    ("Analytics", "PermissionAsked", "True".into()),
                    ("Analytics", "Enabled", "False".into()),
                ],
            )
        }
        "ppsspp" => set_ini(
            &exe_dir
                .join("memstick")
                .join("PSP")
                .join("SYSTEM")
                .join("ppsspp.ini"),
            &[
                ("General", "FirstRun", "False".into()),
                ("Graphics", "FullScreen", "True".into()),
            ],
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
                if let Some(p) =
                    find_file(b, "xbox/*bios*.bin").or_else(|| find_file(b, "xbox/complex*.bin"))
                {
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

/// First controller, Xbox layout (ROG Ally), as DuckStation/PCSX2 SDL bindings.
/// Key names match their `[Pad1]` sections; both use the same SDL binding syntax.
fn sdl_dualshock(controller_type: &str) -> Vec<(&'static str, String)> {
    let mut pad = vec![("Type", controller_type.to_string())];
    pad.extend(
        [
            ("Up", "DPadUp"),
            ("Right", "DPadRight"),
            ("Down", "DPadDown"),
            ("Left", "DPadLeft"),
            ("Triangle", "Y"),
            ("Circle", "B"),
            ("Cross", "A"),
            ("Square", "X"),
            ("Select", "Back"),
            ("Start", "Start"),
            ("L1", "LeftShoulder"),
            ("R1", "RightShoulder"),
            ("L2", "+LeftTrigger"),
            ("R2", "+RightTrigger"),
            ("L3", "LeftStick"),
            ("R3", "RightStick"),
            ("LLeft", "-LeftX"),
            ("LRight", "+LeftX"),
            ("LDown", "+LeftY"),
            ("LUp", "-LeftY"),
            ("RLeft", "-RightX"),
            ("RRight", "+RightX"),
            ("RDown", "+RightY"),
            ("RUp", "-RightY"),
            ("Analog", "Guide"),
            ("LargeMotor", "LargeMotor"),
            ("SmallMotor", "SmallMotor"),
        ]
        .map(|(key, input)| (key, format!("SDL-0/{input}"))),
    );
    pad
}

/// First XInput controller as a GameCube pad (Dolphin `GCPadNew.ini`).
fn xinput_gcpad() -> Vec<(&'static str, String)> {
    let mut pad = vec![("Device", "XInput/0/Gamepad".to_string())];
    pad.extend(
        [
            ("Buttons/A", "Button A"),
            ("Buttons/B", "Button B"),
            ("Buttons/X", "Button X"),
            ("Buttons/Y", "Button Y"),
            ("Buttons/Z", "Shoulder R"),
            ("Buttons/Start", "Start"),
            ("Main Stick/Up", "Left Y+"),
            ("Main Stick/Down", "Left Y-"),
            ("Main Stick/Left", "Left X-"),
            ("Main Stick/Right", "Left X+"),
            ("C-Stick/Up", "Right Y+"),
            ("C-Stick/Down", "Right Y-"),
            ("C-Stick/Left", "Right X-"),
            ("C-Stick/Right", "Right X+"),
            ("Triggers/L", "Trigger L"),
            ("Triggers/R", "Trigger R"),
            ("Triggers/L-Analog", "Trigger L"),
            ("Triggers/R-Analog", "Trigger R"),
            ("D-Pad/Up", "Pad N"),
            ("D-Pad/Down", "Pad S"),
            ("D-Pad/Left", "Pad W"),
            ("D-Pad/Right", "Pad E"),
            ("Rumble/Motor", "Motor L"),
        ]
        .map(|(key, input)| (key, format!("`{input}`"))),
    );
    pad
}

/// Writes a whole section only when the file doesn't have it yet, so mappings
/// the user changes in the emulator are never overwritten.
fn add_section_if_missing(
    path: &Path,
    section: &str,
    entries: &[(&str, String)],
) -> anyhow::Result<()> {
    let header = format!("[{section}]");
    if read_lines(path)
        .iter()
        .any(|l| l.trim().eq_ignore_ascii_case(&header))
    {
        return Ok(());
    }
    let entries: Vec<_> = entries
        .iter()
        .map(|(k, v)| (section, *k, v.clone()))
        .collect();
    set_ini(path, &entries)
}

/// Copies `*.<ext>` files from `from` into `to`, skipping ones already there.
fn copy_missing(from: &Path, to: &Path, ext: &str) -> anyhow::Result<()> {
    let Ok(entries) = fs::read_dir(from) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case(ext))
        {
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
        match lines
            .iter()
            .position(|l| key_of(l).is_some_and(|k| k == *key))
        {
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
        let Some(start) = lines
            .iter()
            .position(|l| l.trim().eq_ignore_ascii_case(&header))
        else {
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
        match (start + 1..end)
            .find(|&i| key_of(&lines[i]).is_some_and(|k| k.eq_ignore_ascii_case(key)))
        {
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
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(String::from)
        .collect()
}

fn write_lines(path: &Path, lines: &[String]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, lines.join("\n") + "\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("lumen-config-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("settings.ini")
    }

    #[test]
    fn set_ini_updates_in_place_and_keeps_other_keys() {
        let path = temp_file("set");
        fs::write(
            &path,
            "[Main]\nStartFullscreen = false\nTheme = dark\n\n[BIOS]\nPath = x\n",
        )
        .unwrap();
        set_ini(
            &path,
            &[
                ("Main", "StartFullscreen", "true".into()),
                ("UI", "Scale", "2".into()),
            ],
        )
        .unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("StartFullscreen = true"));
        assert!(text.contains("Theme = dark"));
        assert!(text.contains("[BIOS]\nPath = x"));
        assert!(text.contains("[UI]\nScale = 2"));
        assert_eq!(text.matches("StartFullscreen").count(), 1);
    }

    #[test]
    fn controller_section_is_written_once() {
        let path = temp_file("pad");
        add_section_if_missing(&path, "Pad1", &sdl_dualshock("DualShock2")).unwrap();
        let first = fs::read_to_string(&path).unwrap();
        assert!(first.contains("Cross = SDL-0/A"));
        assert!(first.contains("L2 = SDL-0/+LeftTrigger"));

        // A user remap survives the next launch.
        fs::write(&path, first.replace("Cross = SDL-0/A", "Cross = SDL-0/B")).unwrap();
        add_section_if_missing(&path, "Pad1", &sdl_dualshock("DualShock2")).unwrap();
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("Cross = SDL-0/B"));
    }

    #[test]
    fn gcpad_uses_dolphin_xinput_names() {
        let pad = xinput_gcpad();
        assert!(pad.contains(&("Device", "XInput/0/Gamepad".into())));
        assert!(pad.contains(&("Buttons/A", "`Button A`".into())));
        assert!(pad.contains(&("D-Pad/Up", "`Pad N`".into())));
    }
}
