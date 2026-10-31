//! In-game shortcuts Lumen adds on top of the emulators: hold Select + LT to
//! rewind, Select + RT to fast-forward, Select + Y for a screenshot; the
//! quick menu switches discs and takes screenshots too. The webview can't see the pad while a game runs, so
//! `input.rs` reads it and Lumen presses a keyboard key the emulator has
//! bound to the action (see `presets.rs`), delivered to the game window.
//! Only RetroArch and DuckStation get keys: other emulators bind F-keys to
//! things like loading states, so they get nothing.

use serde::Serialize;
use std::{path::Path, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hotkey {
    Rewind,
    FastForward,
    NextDisc,
    PreviousDisc,
    Screenshot,
}

impl Hotkey {
    /// Keyboard scan code of the key bound to it (F5–F9).
    #[cfg_attr(not(windows), allow(dead_code))]
    fn scan_code(self) -> u16 {
        match self {
            Hotkey::Rewind => 0x3F,
            Hotkey::FastForward => 0x40,
            Hotkey::NextDisc => 0x41,
            Hotkey::PreviousDisc => 0x42,
            Hotkey::Screenshot => 0x43,
        }
    }
}

/// RetroArch's key names for the same keys, for `lumen.cfg`.
pub const RETROARCH_KEYS: &[(&str, &str)] = &[
    ("input_rewind", "f5"),
    ("input_hold_fast_forward", "f6"),
    ("input_disk_next", "f7"),
    ("input_disk_prev", "f8"),
    ("input_screenshot", "f9"),
    // RetroArch's own defaults on those keys.
    ("input_desktop_menu_toggle", "nul"),
    ("input_state_slot_decrease", "nul"),
    ("input_state_slot_increase", "nul"),
    ("input_audio_mute", "nul"),
];

/// DuckStation's `[Hotkeys]` bindings for the same keys.
pub const DUCKSTATION_KEYS: &[(&str, &str)] = &[
    ("Rewind", "Keyboard/F5"),
    ("FastForward", "Keyboard/F6"),
    ("SwitchToNextDisc", "Keyboard/F7"),
    ("SwitchToPreviousDisc", "Keyboard/F8"),
    ("Screenshot", "Keyboard/F9"),
];

/// Systems light enough for RetroArch to keep a rewind buffer every frame
/// without slowing down (8/16-bit consoles, handhelds up to the GBA).
pub const REWIND_SYSTEMS: &[&str] = &[
    "nes", "snes", "gb", "gbc", "gba", "sms", "genesis", "pce", "ngp", "ngpc", "ws", "wsc",
    "a2600", "a7800",
];

/// What the running game supports, for the in-game screen's hints.
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Controls {
    pub rewind: bool,
    pub fast_forward: bool,
    /// Started from a multi-disc playlist (`.m3u`): discs can be switched.
    pub discs: bool,
}

pub fn for_game(emulator: &str, system_id: &str, rom: &Path) -> Controls {
    let driven = matches!(emulator, "retroarch" | "duckstation");
    Controls {
        rewind: emulator == "duckstation"
            || (emulator == "retroarch" && REWIND_SYSTEMS.contains(&system_id)),
        fast_forward: driven,
        discs: driven
            && rom
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("m3u")),
    }
}

/// Presses (`down`) or releases a hotkey in the focused game window.
pub fn press(hotkey: Hotkey, down: bool) {
    #[cfg(windows)]
    {
        crate::windows::send_key(hotkey.scan_code(), down);
    }
    #[cfg(not(windows))]
    let _ = (hotkey, down);
}

/// A short press, long enough for emulators that poll once per frame.
pub fn tap(hotkey: Hotkey) {
    press(hotkey, true);
    std::thread::sleep(Duration::from_millis(80));
    press(hotkey, false);
}

/// Switches to the next (or previous) disc of the running playlist, once
/// the game is back in front. RetroArch swaps through its command port
/// (open the tray, change disc, close it); DuckStation gets its hotkey.
pub fn switch_disc(emulator: &str, pid: u32, next: bool) {
    // The quick menu just handed focus back to the game.
    std::thread::sleep(Duration::from_millis(400));
    match emulator {
        "retroarch" => {
            let step = if next { "DISK_NEXT" } else { "DISK_PREV" };
            for command in ["DISK_EJECT_TOGGLE", step, "DISK_EJECT_TOGGLE"] {
                crate::states::send(command);
                std::thread::sleep(Duration::from_millis(200));
            }
        }
        "duckstation" if crate::windows::is_foreground(pid) => tap(if next {
            Hotkey::NextDisc
        } else {
            Hotkey::PreviousDisc
        }),
        _ => {}
    }
}
