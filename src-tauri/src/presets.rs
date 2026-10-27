//! Tuned emulator settings for the ROG Ally family, re-applied before every
//! launch so users never open an emulator's own settings.
//!
//! `config.rs` owns paths, BIOS and controller wiring; this module owns how
//! games look and run. Every key was checked against the emulator's current
//! source; tier values follow community guidance for Ally/Deck-class APUs.

use crate::config::set_ini;
use crate::device::Tier;
use crate::formats::{entry, merge_json, merge_toml, merge_yaml};
use crate::manifest::Emulator;
use std::path::Path;

pub fn apply(emu: &Emulator, exe_dir: &Path, tier: Tier) -> anyhow::Result<()> {
    let high = tier == Tier::High;
    match emu.id.as_str() {
        "rpcs3" => rpcs3(exe_dir, high),
        "shadps4" => shadps4(exe_dir, high),
        "xemu" => xemu(exe_dir, high),
        // Xenia rewrites its whole TOML and replays default migrations, so its
        // settings go on the command line instead (see `launch_args`).
        _ => Ok(()),
    }
}

/// Extra arguments placed before the emulator's own launch arguments.
pub fn launch_args(emu: &Emulator, tier: Tier) -> Vec<String> {
    let high = tier == Tier::High;
    match emu.id.as_str() {
        // Every Xenia cvar accepts `--name=value`; command line wins over the
        // config file and is never written back to it.
        "xenia" => {
            let scale = if high { 2 } else { 1 };
            let mut args: Vec<String> = [
                "--gpu=d3d12",
                "--vsync=true",
                "--framerate_limit=0",
                "--async_shader_compilation=true",
                "--readback_resolve=none",
                "--fullscreen=true",
                "--present_letterbox=true",
                "--apu=xaudio2",
                "--hid=xinput",
                "--discord=false",
                "--show_achievement_notification=false",
                "--license_mask=1",
                "--apply_patches=true",
            ]
            .map(String::from)
            .into();
            args.push(format!("--draw_resolution_scale_x={scale}"));
            args.push(format!("--draw_resolution_scale_y={scale}"));
            if high {
                args.extend(
                    [
                        "--anisotropic_override=5",
                        "--postprocess_scaling_and_sharpening=fsr",
                        "--postprocess_antialiasing=fxaa",
                    ]
                    .map(String::from),
                );
            }
            args
        }
        _ => Vec::new(),
    }
}

fn rpcs3(exe_dir: &Path, high: bool) -> anyhow::Result<()> {
    let config = exe_dir.join("config");
    // Merged, never replaced: the file holds a random console PSID.
    merge_yaml(
        &config.join("config.yml"),
        &[
            entry(&["Core"], "PPU Decoder", "Recompiler (LLVM)"),
            entry(&["Core"], "SPU Decoder", "Recompiler (LLVM)"),
            entry(
                &["Core"],
                "SPU Block Size",
                if high { "Mega" } else { "Safe" },
            ),
            entry(&["Core"], "Preferred SPU Threads", 0),
            entry(&["Video"], "Renderer", "Vulkan"),
            // Output stays 720p (1080p output breaks many games); scale internally.
            entry(&["Video"], "Resolution", "1280x720"),
            entry(&["Video"], "Resolution Scale", if high { 150 } else { 100 }),
            entry(
                &["Video"],
                "Output Scaling Mode",
                if high {
                    "Bilinear"
                } else {
                    "FidelityFX Super Resolution"
                },
            ),
            entry(
                &["Video"],
                "Shader Mode",
                "Async Recompiler with Shader Interpreter",
            ),
            entry(&["Video"], "Frame limit", "Auto"),
            entry(
                &["Video"],
                "Anisotropic Filter Override",
                if high { 16 } else { 0 },
            ),
            // Borderless, so switching back to Lumen is instant.
            entry(&["Video", "Vulkan"], "Exclusive Fullscreen Mode", "Disable"),
            entry(&["Video", "Performance Overlay"], "Enabled", false),
            entry(&["Miscellaneous"], "Start games in fullscreen mode", true),
            entry(&["Miscellaneous"], "Exit RPCS3 when process finishes", true),
            entry(
                &["Miscellaneous"],
                "Automatically start games after boot",
                true,
            ),
            entry(&["Miscellaneous"], "Show shader compilation hint", false),
            entry(
                &["Miscellaneous"],
                "Prevent display sleep while running games",
                true,
            ),
            entry(
                &["Miscellaneous"],
                "Pause emulation on RPCS3 focus loss",
                false,
            ),
        ],
    )?;
    set_ini(
        &exe_dir.join("GuiConfigs").join("CurrentSettings.ini"),
        &[
            ("main_window", "confirmationBoxExitGame", "false".into()),
            ("main_window", "confirmationBoxBootGame", "false".into()),
            ("main_window", "infoBoxEnabledWelcome", "false".into()),
            ("Meta", "checkUpdateStart", "false".into()),
        ],
    )
}

fn shadps4(exe_dir: &Path, high: bool) -> anyhow::Result<()> {
    let (width, height) = if high { (1920, 1080) } else { (1280, 720) };
    // Native JSON types only: shadPS4 aborts on e.g. "true" as a string.
    merge_json(
        &exe_dir.join("user").join("config.json"),
        &[
            entry(&["GPU"], "full_screen", true),
            entry(&["GPU"], "full_screen_mode", "Fullscreen (Borderless)"),
            entry(&["GPU"], "present_mode", "Mailbox"),
            entry(&["GPU"], "window_width", 1920),
            entry(&["GPU"], "window_height", 1080),
            entry(&["GPU"], "internal_screen_width", width),
            entry(&["GPU"], "internal_screen_height", height),
            entry(&["GPU"], "fsr_enabled", false),
            entry(&["GPU"], "readbacks_mode", 0),
            entry(&["GPU"], "hdr_allowed", false),
            entry(&["Vulkan"], "pipeline_cache_enabled", true),
            entry(&["General"], "neo_mode", false),
            entry(&["General"], "show_splash", false),
            entry(&["General"], "show_fps_counter", false),
            entry(&["General"], "discord_rpc_enabled", false),
            entry(&["Log"], "filter", "*:error"),
            entry(&["Input"], "cursor_state", 2),
            entry(&["Input"], "use_unified_input_config", true),
        ],
    )
}

fn xemu(exe_dir: &Path, high: bool) -> anyhow::Result<()> {
    // Enum values are case-sensitive in xemu.
    merge_toml(
        &exe_dir.join("xemu.toml"),
        &[
            entry(&["general"], "show_welcome", false),
            entry(&["general"], "skip_boot_anim", true),
            entry(&["general", "updates"], "check", false),
            entry(&["display"], "renderer", "VULKAN"),
            entry(
                &["display", "quality"],
                "surface_scale",
                if high { 3 } else { 2 },
            ),
            entry(&["display", "window"], "fullscreen_on_startup", true),
            entry(&["display", "window"], "fullscreen_exclusive", false),
            entry(&["display", "window"], "vsync", true),
            entry(&["display", "window"], "startup_size", "1920x1080"),
            entry(&["display", "ui"], "fit", "scale"),
            entry(&["display", "ui"], "show_menubar", false),
            entry(&["input"], "auto_bind", true),
        ],
    )
}
