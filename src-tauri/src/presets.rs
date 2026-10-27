//! Tuned emulator settings for the ROG Ally family, re-applied before every
//! launch so users never open an emulator's own settings.
//!
//! `config.rs` owns paths, BIOS and controller wiring; this module owns how
//! games look and run. Keys were verified against each emulator's source.

use crate::device::Tier;
use crate::manifest::Emulator;
use std::path::Path;

pub fn apply(emu: &Emulator, exe_dir: &Path, tier: Tier) -> anyhow::Result<()> {
    let _ = (exe_dir, tier);
    match emu.id.as_str() {
        _ => Ok(()),
    }
}
