//! Writes each emulator's own config so it starts fullscreen, skips first-run
//! wizards and finds the BIOS files. Runs after install and before every launch,
//! so BIOS files added later are picked up without user action.
//! Only the keys we own are touched; everything else the user changes is kept.

use crate::device::Tier;
use crate::formats;
use crate::library::find_file;
use crate::manifest::Emulator;
use crate::presets;
use std::{fs, path::Path};

pub fn configure(
    emu: &Emulator,
    exe_dir: &Path,
    bios: Option<&Path>,
    tier: Tier,
) -> anyhow::Result<()> {
    presets::apply(emu, exe_dir, tier)?;

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
            add_section_if_missing(
                &config.join("WiimoteNew.ini"),
                "Wiimote1",
                &xinput_wiimote(),
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
            // xemu.toml next to the exe is what turns on portable mode.
            let mut entries = Vec::new();
            if let Some(b) = bios {
                let path = |p: std::path::PathBuf| p.to_string_lossy().into_owned();
                if let Some(p) = find_file(b, "xbox/mcpx*.bin") {
                    entries.push(formats::entry(&["sys", "files"], "bootrom_path", path(p)));
                }
                if let Some(p) =
                    find_file(b, "xbox/*bios*.bin").or_else(|| find_file(b, "xbox/complex*.bin"))
                {
                    entries.push(formats::entry(&["sys", "files"], "flashrom_path", path(p)));
                }
                if let Some(p) = find_file(b, "xbox/*.qcow2") {
                    entries.push(formats::entry(&["sys", "files"], "hdd_path", path(p)));
                }
            }
            formats::merge_toml(&exe_dir.join("xemu.toml"), &entries)
        }
        "cemu" => {
            // A `portable` dir next to Cemu.exe keeps settings, saves and the mlc
            // beside the emulator; without settings.xml Cemu opens its setup wizard.
            let portable = exe_dir.join("portable");
            let profile = portable.join("controllerProfiles").join("controller0.xml");
            if !profile.exists() {
                fs::create_dir_all(profile.parent().unwrap_or(&portable))?;
                fs::write(&profile, CEMU_GAMEPAD)?;
            }
            Ok(())
        }
        "melonds" => {
            // Portable dir wins over the build's default location.
            let config = exe_dir.join("portable").join("melonDS.toml");
            if !formats::toml_has_table(&config, &["Instance0", "Joystick"]) {
                // Raw SDL joystick indices for the XInput backend: A0 B1 X2 Y3 LB4 RB5
                // Back6 Start7; D-pad is hat 0 (0x100 | dir). Nintendo-positional.
                let joy = |key, v: i64| formats::entry(&["Instance0", "Joystick"], key, v);
                formats::merge_toml(
                    &config,
                    &[
                        formats::entry(&["Instance0"], "JoystickID", 0),
                        joy("A", 1),
                        joy("B", 0),
                        joy("X", 3),
                        joy("Y", 2),
                        joy("L", 4),
                        joy("R", 5),
                        joy("Select", 6),
                        joy("Start", 7),
                        joy("Up", 257),
                        joy("Right", 258),
                        joy("Down", 260),
                        joy("Left", 264),
                        joy("HK_SwapScreens", 9),
                    ],
                )?;
            }
            Ok(())
        }
        "azahar" => {
            // A `user` dir next to the exe is Azahar's portable mode.
            let ini = exe_dir.join("user").join("config").join("qt-config.ini");
            let controls = azahar_controls();
            let refs: Vec<(&str, String)> = controls
                .iter()
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect();
            add_section_if_missing(&ini, "Controls", &refs)
        }
        "rpcs3" => {
            // Without a profile RPCS3 drives player 1 from the keyboard. Handler and
            // Device are enough: RPCS3 fills the XInput button defaults. Written once
            // so in-emulator remaps survive.
            let inputs = exe_dir.join("config").join("input_configs");
            let profile = inputs.join("global").join("Default.yml");
            if !profile.exists() {
                formats::merge_yaml(
                    &profile,
                    &[
                        formats::entry(&["Player 1 Input"], "Handler", "XInput"),
                        formats::entry(&["Player 1 Input"], "Device", "XInput Pad #1"),
                    ],
                )?;
            }
            let active = inputs.join("active_input_configurations.yml");
            if !active.exists() {
                formats::merge_yaml(
                    &active,
                    &[formats::entry(
                        &["Active Configurations"],
                        "global",
                        "Default",
                    )],
                )?;
            }
            Ok(())
        }
        "shadps4" => {
            // Portable mode is `<cwd>/user`; Lumen launches with cwd = exe dir.
            // Pre-made home dirs skip shadPS4's blocking save-migration dialog.
            for user in 1000..=1003 {
                for sub in ["savedata", "trophy", "inputs"] {
                    fs::create_dir_all(
                        exe_dir
                            .join("user")
                            .join("home")
                            .join(user.to_string())
                            .join(sub),
                    )?;
                }
            }
            // LLE system modules are looked up in the portable user dir.
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
            ("Rumble/Motor", "Motor L`|`Motor R"),
        ]
        .map(|(key, input)| (key, format!("`{input}`"))),
    );
    pad
}

/// Emulated Wiimote + Nunchuk on the first XInput pad (Dolphin `WiimoteNew.ini`).
/// Pointer on the right stick, Nunchuk stick on the left, shake on R3.
fn xinput_wiimote() -> Vec<(&'static str, String)> {
    let mut pad = vec![
        ("Source", "1".to_string()),
        ("Device", "XInput/0/Gamepad".to_string()),
        ("Extension", "Nunchuk".to_string()),
    ];
    pad.extend(
        [
            ("Buttons/A", "Button A"),
            ("Buttons/B", "Trigger R"),
            ("Buttons/1", "Button X"),
            ("Buttons/2", "Button Y"),
            ("Buttons/-", "Back"),
            ("Buttons/+", "Start"),
            ("Buttons/Home", "Thumb L"),
            ("D-Pad/Up", "Pad N"),
            ("D-Pad/Down", "Pad S"),
            ("D-Pad/Left", "Pad W"),
            ("D-Pad/Right", "Pad E"),
            ("IR/Up", "Right Y+"),
            ("IR/Down", "Right Y-"),
            ("IR/Left", "Right X-"),
            ("IR/Right", "Right X+"),
            ("Shake/X", "Thumb R"),
            ("Shake/Y", "Thumb R"),
            ("Shake/Z", "Thumb R"),
            ("Rumble/Motor", "Motor L`|`Motor R"),
            ("Nunchuk/Buttons/C", "Shoulder L"),
            ("Nunchuk/Buttons/Z", "Trigger L"),
            ("Nunchuk/Stick/Up", "Left Y+"),
            ("Nunchuk/Stick/Down", "Left Y-"),
            ("Nunchuk/Stick/Left", "Left X-"),
            ("Nunchuk/Stick/Right", "Left X+"),
            ("Nunchuk/Shake/X", "Shoulder R"),
            ("Nunchuk/Shake/Y", "Shoulder R"),
            ("Nunchuk/Shake/Z", "Shoulder R"),
        ]
        .map(|(key, input)| (key, format!("`{input}`"))),
    );
    pad
}

/// Cemu `controllerProfiles/controller0.xml`: Wii U GamePad on XInput pad 0,
/// using Cemu's own XInput default button ids (VPADController.cpp).
const CEMU_GAMEPAD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<emulated_controller>
    <type>Wii U GamePad</type>
    <toggle_display>0</toggle_display>
    <controller>
        <api>XInput</api>
        <uuid>0</uuid>
        <display_name>Controller 1</display_name>
        <rumble>0.5</rumble>
        <axis><deadzone>0.15</deadzone><range>1</range></axis>
        <rotation><deadzone>0.15</deadzone><range>1</range></rotation>
        <trigger><deadzone>0.15</deadzone><range>1</range></trigger>
        <mappings>
            <entry><mapping>1</mapping><button>13</button></entry>
            <entry><mapping>2</mapping><button>12</button></entry>
            <entry><mapping>3</mapping><button>15</button></entry>
            <entry><mapping>4</mapping><button>14</button></entry>
            <entry><mapping>5</mapping><button>8</button></entry>
            <entry><mapping>6</mapping><button>9</button></entry>
            <entry><mapping>7</mapping><button>42</button></entry>
            <entry><mapping>8</mapping><button>43</button></entry>
            <entry><mapping>9</mapping><button>4</button></entry>
            <entry><mapping>10</mapping><button>5</button></entry>
            <entry><mapping>11</mapping><button>0</button></entry>
            <entry><mapping>12</mapping><button>1</button></entry>
            <entry><mapping>13</mapping><button>2</button></entry>
            <entry><mapping>14</mapping><button>3</button></entry>
            <entry><mapping>15</mapping><button>6</button></entry>
            <entry><mapping>16</mapping><button>7</button></entry>
            <entry><mapping>17</mapping><button>39</button></entry>
            <entry><mapping>18</mapping><button>45</button></entry>
            <entry><mapping>19</mapping><button>44</button></entry>
            <entry><mapping>20</mapping><button>38</button></entry>
            <entry><mapping>21</mapping><button>41</button></entry>
            <entry><mapping>22</mapping><button>47</button></entry>
            <entry><mapping>23</mapping><button>46</button></entry>
            <entry><mapping>24</mapping><button>40</button></entry>
        </mappings>
    </controller>
</emulated_controller>
"#;

/// Azahar `[Controls]` profile: SDL GameController on port 0, Nintendo-positional
/// face buttons like Azahar's own automap. `guid:0` matches any pad. Values hold
/// commas, so they are quoted (QSettings would read a list otherwise).
fn azahar_controls() -> Vec<(String, String)> {
    let mut controls = vec![
        ("profile".to_string(), "0".to_string()),
        (r"profiles\size".to_string(), "1".to_string()),
        (r"profiles\1\name".to_string(), "Default".to_string()),
        // 2 = match by GUID + port, so port 0 is the first pad.
        (r"profiles\1\input_maptype".to_string(), "2".to_string()),
    ];
    controls.extend(
        [
            ("button_a", "button:1"),
            ("button_b", "button:0"),
            ("button_x", "button:3"),
            ("button_y", "button:2"),
            ("button_up", "button:11"),
            ("button_down", "button:12"),
            ("button_left", "button:13"),
            ("button_right", "button:14"),
            ("button_l", "button:9"),
            ("button_r", "button:10"),
            ("button_start", "button:6"),
            ("button_select", "button:4"),
            ("button_home", "button:5"),
            ("button_zl", "axis:4,direction:+,threshold:0.5"),
            ("button_zr", "axis:5,direction:+,threshold:0.5"),
            ("circle_pad", "axis_x:0,axis_y:1,deadzone:0.1"),
            ("c_stick", "axis_x:2,axis_y:3,deadzone:0.1"),
        ]
        .map(|(key, input)| {
            (
                format!(r"profiles\1\{key}"),
                format!("\"engine:sdl,api:controller,port:0,guid:0,{input}\""),
            )
        }),
    );
    controls
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
pub(crate) fn set_ini(path: &Path, entries: &[(&str, &str, String)]) -> anyhow::Result<()> {
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
