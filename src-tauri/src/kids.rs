//! Kids mode: behind a PIN, Lumen shows one collection (or every emulated
//! game), hides settings, stores and removal, and can cap play time per
//! day. When the day's time runs out the game is saved and closed, and no
//! other starts until tomorrow. The UI passes local midnight (it knows the
//! time zone, the backend doesn't).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Kids {
    pub active: bool,
    /// SHA-256 of the PIN (salted with a fixed prefix); None before setup.
    pub pin_hash: Option<String>,
    /// The only collection shown; None shows every emulated game.
    pub collection: Option<String>,
    /// Minutes of play per day; None for no limit.
    pub daily_minutes: Option<u32>,
}

pub fn hash(pin: &str) -> String {
    let digest = Sha256::digest(format!("lumen-kids:{}", pin.trim()).as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

impl Kids {
    pub fn pin_matches(&self, pin: &str) -> bool {
        self.pin_hash.as_deref() == Some(hash(pin).as_str())
    }

    /// Seconds left today; None when there's no limit.
    pub fn remaining(&self, used_today: u64) -> Option<u64> {
        let limit = u64::from(self.daily_minutes?) * 60;
        Some(limit.saturating_sub(used_today))
    }
}

/// Seconds played since `day_start` (unix seconds of local midnight).
pub fn used_since(data_dir: &Path, day_start: u64) -> u64 {
    crate::stats::load(data_dir)
        .iter()
        .filter(|s| s.start >= day_start)
        .map(|s| s.seconds)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_and_limits() {
        let kids = Kids {
            active: true,
            pin_hash: Some(hash("1234")),
            collection: None,
            daily_minutes: Some(30),
        };
        assert!(kids.pin_matches("1234") && kids.pin_matches(" 1234 "));
        assert!(!kids.pin_matches("4321"));
        assert_eq!(kids.remaining(600), Some(1200));
        assert_eq!(kids.remaining(4000), Some(0));
        assert_eq!(Kids::default().remaining(4000), None);
    }
}
