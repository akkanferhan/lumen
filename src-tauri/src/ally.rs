//! Keeps the ROG Ally family's built-in controller in gamepad mode. In
//! Armoury Crate's desktop mode the controller types keys and moves the
//! mouse instead of being an XInput pad, which Lumen's UI and the emulators
//! can't use. Same HID commands Armoury Crate and G-Helper send: feature
//! report 0x5A to ASUS's vendor collection (usage page 0xFF31), a wake-up
//! handshake, the Xbox controller switched on, then `D1 01 01 <mode>`.
//!
//! Armoury Crate applies its own mode a moment after another app comes to
//! the front (desktop, since Lumen isn't a game it knows), which would undo
//! a single command. So each request sends a short burst spread over a few
//! seconds, and the last word is Lumen's.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// The back buttons (M1/M2) open Lumen's in-game menu: they're bound to
/// the keyboard's App-menu key, which a keyboard hook swallows while a game
/// runs, so the game never sees the press (unlike Select + Start, which
/// the emulator reads from the same pad).
static BACK_BUTTONS: AtomicBool = AtomicBool::new(false);
/// A back button is held (set by the keyboard hook).
static BACK_DOWN: AtomicBool = AtomicBool::new(false);
/// A game runs: the hook takes the App-menu key instead of passing it on.
static SWALLOW: AtomicBool = AtomicBool::new(false);

/// Firmware code for the App-menu key in the controller's key table
/// (the same table G-Helper uses) and the Windows key it arrives as.
#[cfg(windows)]
const APP_MENU: u8 = 0x84;
/// ASUS's own M1 / M2 codes, restored when Lumen lets go of the buttons.
#[cfg(windows)]
const DEFAULT_M1: u8 = 0x8F;
#[cfg(windows)]
const DEFAULT_M2: u8 = 0x8E;

/// Turns the back-button menu on or off; the controller is re-bound with
/// the next burst (see `force_gamepad_mode`).
pub fn set_back_buttons(enabled: bool) {
    BACK_BUTTONS.store(enabled, Ordering::SeqCst);
    force_gamepad_mode();
}

pub fn back_buttons() -> bool {
    BACK_BUTTONS.load(Ordering::SeqCst) && crate::device::device().model.is_some()
}

/// A back button is held right now.
pub fn back_button_down() -> bool {
    BACK_DOWN.load(Ordering::SeqCst)
}

/// While a game runs the hook swallows the back buttons' key.
pub fn set_in_game(in_game: bool) {
    SWALLOW.store(in_game && back_buttons(), Ordering::SeqCst);
    if !in_game {
        BACK_DOWN.store(false, Ordering::SeqCst);
    }
}

/// Watches the keyboard for the back buttons' key (a low-level hook on its
/// own thread with a message loop, as Windows requires).
pub fn start_hook() {
    #[cfg(windows)]
    std::thread::spawn(|| {
        use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, MSG, WH_KEYBOARD_LL,
        };
        // SAFETY: the hook procedure is a plain function; the message loop
        // keeps this thread alive (and the hook served) for the app's life.
        unsafe {
            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_hook),
                GetModuleHandleW(std::ptr::null()),
                0,
            );
            if hook.is_null() {
                log("keyboard hook failed: back buttons can't open the menu");
                return;
            }
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
            UnhookWindowsHookEx(hook);
        }
    });
}

#[cfg(windows)]
unsafe extern "system" fn keyboard_hook(
    code: i32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_APPS;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, HC_ACTION, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_SYSKEYDOWN,
    };
    if code == HC_ACTION as i32 && SWALLOW.load(Ordering::SeqCst) {
        let key = &*(lparam as *const KBDLLHOOKSTRUCT);
        if key.vkCode == u32::from(VK_APPS) {
            let down = wparam == WM_KEYDOWN as usize || wparam == WM_SYSKEYDOWN as usize;
            BACK_DOWN.store(down, Ordering::SeqCst);
            return 1; // never reaches the game
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

/// The controller's M1/M2 binding command (zone 8): both buttons send `m1`
/// / `m2` as keyboard keys, primary and secondary layer alike.
#[cfg(windows)]
fn back_button_binding(m1: u8, m2: u8) -> [u8; 50] {
    let mut report = [0u8; 50];
    report[..5].copy_from_slice(&[0x5A, 0xD1, 0x02, 0x08, 0x2C]);
    // Each binding is 10 bytes: [0x02 = keyboard, 0, key, 0…]; the zone
    // lists M2 then M1, each with a second-layer copy.
    for (offset, key) in [(5, m2), (16, m2), (27, m1), (38, m1)] {
        report[offset] = 0x02;
        report[offset + 2] = key;
    }
    report
}

/// Gives M1/M2 back to ASUS's defaults (on exit, or when turned off).
pub fn restore_back_buttons() {
    #[cfg(windows)]
    if crate::device::device().model.is_some() {
        let binding = back_button_binding(DEFAULT_M1, DEFAULT_M2);
        let _ = send(&[&binding]);
    }
}

/// `ally.log` in Lumen's data dir: what was found and sent, for diagnosis.
static LOG: OnceLock<PathBuf> = OnceLock::new();

/// Delays (ms, from the request) at which the command is (re)sent.
#[cfg(windows)]
const BURST: &[u64] = &[0, 1000, 2500, 5000, 8000];

/// Starts logging to `data_dir/ally.log`; call once at startup.
pub fn init(data_dir: &std::path::Path) {
    let path = data_dir.join("ally.log");
    // Fresh log each start, so it stays small.
    let _ = std::fs::write(&path, "");
    let _ = LOG.set(path);
}

#[cfg_attr(not(windows), allow(dead_code))]
fn log(line: &str) {
    use std::io::Write;
    let Some(path) = LOG.get() else { return };
    if let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(path) {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "[{secs}] {line}");
    }
}

/// Puts the controller in gamepad mode. A no-op on other hardware and when
/// the controller can't be reached; cheap enough to call on every start,
/// focus, wake and launch.
pub fn force_gamepad_mode() {
    #[cfg(windows)]
    {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::Duration;
        /// A newer request takes over the burst of an older one.
        static GENERATION: AtomicU64 = AtomicU64::new(0);

        // Only on a recognised Ally-family handheld, so no other ASUS device
        // (a laptop keyboard shares the vendor collection) gets the command.
        let device = crate::device::device();
        if device.model.is_none() {
            log(&format!("skipped: not an Ally-family CPU ({})", device.cpu));
            return;
        }
        let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        std::thread::spawn(move || {
            let mut elapsed = 0;
            for &at in BURST {
                std::thread::sleep(Duration::from_millis(at - elapsed));
                elapsed = at;
                if GENERATION.load(Ordering::SeqCst) != generation {
                    return;
                }
                match send_gamepad_mode() {
                    Ok(sent) => log(&format!("+{at}ms gamepad mode sent to {sent}")),
                    Err(e) => log(&format!("+{at}ms gamepad mode not set: {e}")),
                }
            }
        });
    }
}

/// Sends the commands to every ASUS vendor collection; returns which ones
/// accepted them.
#[cfg(windows)]
fn send_gamepad_mode() -> anyhow::Result<String> {
    const REPORT_ID: u8 = 0x5A;
    const GAMEPAD: u8 = 0x01;

    let mut wake = vec![REPORT_ID];
    wake.extend_from_slice(b"ASUS Tech.Inc.");
    let binding = if back_buttons() {
        back_button_binding(APP_MENU, APP_MENU)
    } else {
        back_button_binding(DEFAULT_M1, DEFAULT_M2)
    };
    send(&[
        &wake,
        // Xbox controller on (desktop mode can leave it switched off).
        &[REPORT_ID, 0xD1, 0x0B, 0x01, 0x01],
        &[REPORT_ID, 0xD1, 0x01, 0x01, GAMEPAD],
        &binding,
    ])
}

/// Sends feature reports to every ASUS vendor collection (usage page
/// 0xFF31); returns which ones took them all.
#[cfg(windows)]
fn send(commands: &[&[u8]]) -> anyhow::Result<String> {
    const ASUS: u16 = 0x0B05;
    const VENDOR_PAGE: u16 = 0xFF31;

    let api = hidapi::HidApi::new()?;
    let mut sent = Vec::new();
    let mut failures = Vec::new();
    for info in api
        .device_list()
        .filter(|d| d.vendor_id() == ASUS && d.usage_page() == VENDOR_PAGE)
    {
        let name = format!("{:04x}/{:04x}", info.product_id(), info.usage());
        let device = match info.open_device(&api) {
            Ok(device) => device,
            Err(e) => {
                failures.push(format!("{name} open: {e}"));
                continue;
            }
        };
        // hidapi pads each report to the collection's feature report length.
        match commands
            .iter()
            .try_for_each(|c| device.send_feature_report(c))
        {
            Ok(()) => sent.push(name),
            Err(e) => failures.push(format!("{name} write: {e}")),
        }
    }
    if sent.is_empty() {
        let asus: Vec<String> = api
            .device_list()
            .filter(|d| d.vendor_id() == ASUS)
            .map(|d| {
                format!(
                    "{:04x} page {:04x} usage {:04x}",
                    d.product_id(),
                    d.usage_page(),
                    d.usage()
                )
            })
            .collect();
        anyhow::bail!(
            "no collection accepted it; failures: [{}]; ASUS HID: [{}]",
            failures.join(", "),
            asus.join(", ")
        );
    }
    Ok(sent.join(", "))
}
