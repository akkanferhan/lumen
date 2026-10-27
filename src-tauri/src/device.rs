//! Identifies the handheld so emulator presets can match its GPU budget.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// How much GPU work presets may ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tier {
    /// Z1, Z2 A: ~4–8 RDNA compute units.
    Standard,
    /// Z1 Extreme, Z2 Extreme (Ally, Ally X, Xbox Ally X): 12–16 compute units.
    High,
}

/// User choice in settings; `Auto` follows the detected hardware.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TierSetting {
    #[default]
    Auto,
    Standard,
    High,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// e.g. "AMD Ryzen Z1 Extreme"
    pub cpu: String,
    /// Friendly name when we recognise the chip, e.g. "ROG Ally (Z1 Extreme)".
    pub model: Option<String>,
    pub detected_tier: Tier,
}

pub fn device() -> &'static Device {
    static DEVICE: OnceLock<Device> = OnceLock::new();
    DEVICE.get_or_init(|| classify(&cpu_name()))
}

pub fn tier(setting: TierSetting) -> Tier {
    match setting {
        TierSetting::Auto => device().detected_tier,
        TierSetting::Standard => Tier::Standard,
        TierSetting::High => Tier::High,
    }
}

fn classify(cpu: &str) -> Device {
    let lower = cpu.to_lowercase();
    let (model, tier) = if lower.contains("z2 extreme") || lower.contains("z2 ai extreme") {
        (Some("ROG Xbox Ally X (Z2 Extreme)"), Tier::High)
    } else if lower.contains("z1 extreme") {
        (Some("ROG Ally / Ally X (Z1 Extreme)"), Tier::High)
    } else if lower.contains("z2") {
        (Some("ROG Xbox Ally (Z2 A)"), Tier::Standard)
    } else if lower.contains("z1") {
        (Some("ROG Ally (Z1)"), Tier::Standard)
    } else {
        // Unknown hardware: stay conservative, the user can raise it in the menu.
        (None, Tier::Standard)
    };
    Device {
        cpu: cpu.trim().to_string(),
        model: model.map(String::from),
        detected_tier: tier,
    }
}

#[cfg(windows)]
fn cpu_name() -> String {
    use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0")
        .and_then(|key| key.get_value::<String, _>("ProcessorNameString"))
        .unwrap_or_default()
}

#[cfg(not(windows))]
fn cpu_name() -> String {
    // Development machines only; presets are never applied off Windows.
    std::env::consts::ARCH.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_ally_family() {
        for (cpu, tier) in [
            ("AMD Ryzen Z1 Extreme", Tier::High),
            ("AMD Ryzen Z1", Tier::Standard),
            ("AMD Ryzen AI Z2 Extreme", Tier::High),
            ("AMD Ryzen Z2 A", Tier::Standard),
            ("Intel(R) Core(TM) i7-12700H", Tier::Standard),
        ] {
            assert_eq!(classify(cpu).detected_tier, tier, "{cpu}");
        }
        assert!(classify("AMD Ryzen Z1").model.unwrap().contains("Z1"));
    }
}
