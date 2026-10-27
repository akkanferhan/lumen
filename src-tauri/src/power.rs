//! ASUS power modes (Silent / Performance / Turbo) through the ATKACPI
//! driver, the same firmware interface Armoury Crate and G-Helper use, so
//! the limits stay within ASUS' own validated profiles. No-op on other
//! hardware or when the driver isn't present.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PowerMode {
    Silent,
    Performance,
    Turbo,
}

/// User choice: follow each system's profile, force one mode, or leave it alone.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PowerSetting {
    #[default]
    Auto,
    Off,
    Silent,
    Performance,
    Turbo,
}

impl PowerSetting {
    /// The mode to use for a game whose system prefers `system_mode`.
    pub fn resolve(self, system_mode: PowerMode) -> Option<PowerMode> {
        match self {
            PowerSetting::Auto => Some(system_mode),
            PowerSetting::Off => None,
            PowerSetting::Silent => Some(PowerMode::Silent),
            PowerSetting::Performance => Some(PowerMode::Performance),
            PowerSetting::Turbo => Some(PowerMode::Turbo),
        }
    }
}

#[cfg(windows)]
mod acpi {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;

    const CONTROL_CODE: u32 = 0x0022_240C;
    /// "DEVS": set a device value.
    const DEVS: u32 = 0x5356_4544;
    const PERFORMANCE_MODE: u32 = 0x0012_0075;

    pub fn set_device(device: u32, value: u32) -> bool {
        let path: Vec<u16> = r"\\.\ATKACPI".encode_utf16().chain(Some(0)).collect();
        // SAFETY: standard driver handle open / ioctl / close with local buffers.
        unsafe {
            let handle = CreateFileW(
                path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            );
            if handle == INVALID_HANDLE_VALUE {
                return false;
            }
            // [method id][args length][device id][value]
            let mut input = [0u8; 16];
            input[0..4].copy_from_slice(&DEVS.to_le_bytes());
            input[4..8].copy_from_slice(&8u32.to_le_bytes());
            input[8..12].copy_from_slice(&device.to_le_bytes());
            input[12..16].copy_from_slice(&value.to_le_bytes());
            let mut output = [0u8; 16];
            let mut returned = 0u32;
            let ok = DeviceIoControl(
                handle,
                CONTROL_CODE,
                input.as_ptr().cast(),
                input.len() as u32,
                output.as_mut_ptr().cast(),
                output.len() as u32,
                &mut returned,
                std::ptr::null_mut(),
            ) != 0;
            CloseHandle(handle);
            ok
        }
    }

    pub fn set_mode(mode: u32) -> bool {
        set_device(PERFORMANCE_MODE, mode)
    }
}

/// Switches the ASUS power mode; false when unsupported (non-ASUS hardware).
pub fn set(mode: PowerMode) -> bool {
    #[cfg(windows)]
    {
        acpi::set_mode(match mode {
            PowerMode::Performance => 0,
            PowerMode::Turbo => 1,
            PowerMode::Silent => 2,
        })
    }
    #[cfg(not(windows))]
    {
        let _ = mode;
        false
    }
}

/// Whether the ASUS ACPI driver is reachable (decides if the menu shows the option).
pub fn available() -> bool {
    #[cfg(windows)]
    {
        use std::sync::OnceLock;
        static AVAILABLE: OnceLock<bool> = OnceLock::new();
        *AVAILABLE.get_or_init(|| {
            std::fs::OpenOptions::new()
                .read(true)
                .open(r"\\.\ATKACPI")
                .is_ok()
        })
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_resolve_against_system_profile() {
        assert_eq!(
            PowerSetting::Auto.resolve(PowerMode::Turbo),
            Some(PowerMode::Turbo)
        );
        assert_eq!(PowerSetting::Off.resolve(PowerMode::Turbo), None);
        assert_eq!(
            PowerSetting::Silent.resolve(PowerMode::Turbo),
            Some(PowerMode::Silent)
        );
    }
}
