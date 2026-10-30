//! Keeps the ROG Ally family's built-in controller in gamepad mode. In
//! Armoury Crate's desktop mode the controller types keys and moves the
//! mouse instead of being an XInput pad, which Lumen's UI and the emulators
//! can't use. Same HID command Armoury Crate and G-Helper send: feature
//! report 0x5A to ASUS's vendor collection (usage page 0xFF31), a wake-up
//! handshake then `D1 01 01 <mode>`.

/// Puts the controller in gamepad mode. A no-op on other hardware and when
/// the controller can't be reached; cheap enough to call on every start,
/// wake and launch.
pub fn force_gamepad_mode() {
    // Only on a recognised Ally-family handheld, so no other ASUS device
    // (a laptop keyboard shares the vendor collection) gets the command.
    if crate::device::device().model.is_none() {
        return;
    }
    #[cfg(windows)]
    std::thread::spawn(|| {
        if let Err(e) = send_gamepad_mode() {
            eprintln!("ally: gamepad mode not set: {e}");
        }
    });
}

#[cfg(windows)]
fn send_gamepad_mode() -> anyhow::Result<()> {
    const ASUS: u16 = 0x0B05;
    const VENDOR_PAGE: u16 = 0xFF31;
    const REPORT_ID: u8 = 0x5A;
    const GAMEPAD: u8 = 0x01;

    let api = hidapi::HidApi::new()?;
    let mut sent = 0;
    for info in api
        .device_list()
        .filter(|d| d.vendor_id() == ASUS && d.usage_page() == VENDOR_PAGE)
    {
        let Ok(device) = info.open_device(&api) else {
            continue;
        };
        let mut wake = vec![REPORT_ID];
        wake.extend_from_slice(b"ASUS Tech.Inc.");
        let mode = [REPORT_ID, 0xD1, 0x01, 0x01, GAMEPAD];
        if feature(&device, &wake).is_ok() && feature(&device, &mode).is_ok() {
            sent += 1;
        }
    }
    anyhow::ensure!(
        sent > 0,
        "no ASUS controller collection accepted the command"
    );
    Ok(())
}

/// Sends a feature report padded to the controller's 64-byte report length.
#[cfg(windows)]
fn feature(device: &hidapi::HidDevice, data: &[u8]) -> hidapi::HidResult<()> {
    let mut report = [0u8; 64];
    report[..data.len()].copy_from_slice(data);
    device.send_feature_report(&report)
}
