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
use std::sync::OnceLock;

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
    const ASUS: u16 = 0x0B05;
    const VENDOR_PAGE: u16 = 0xFF31;
    const REPORT_ID: u8 = 0x5A;
    const GAMEPAD: u8 = 0x01;

    let mut wake = vec![REPORT_ID];
    wake.extend_from_slice(b"ASUS Tech.Inc.");
    let commands: [&[u8]; 3] = [
        &wake,
        // Xbox controller on (desktop mode can leave it switched off).
        &[REPORT_ID, 0xD1, 0x0B, 0x01, 0x01],
        &[REPORT_ID, 0xD1, 0x01, 0x01, GAMEPAD],
    ];

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
