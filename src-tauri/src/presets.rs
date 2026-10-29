//! Tuned emulator settings for the ROG Ally family, re-applied before every
//! launch so users never open an emulator's own settings.
//!
//! `config.rs` owns paths, BIOS and controller wiring; this module owns how
//! games look and run. Every key was checked against the emulator's current
//! source; tier values follow community guidance for Ally/Deck-class APUs.
//!
//! Emulators that support it pause on focus loss: Lumen's quick menu takes
//! focus, which pauses the game until "back to game" returns it. The rest
//! (Vita3K, xemu, Xenia, shadPS4, SharpEmu, Cemu) are frozen instead; see
//! `launcher::FREEZE_FOR_MENU`.

use crate::config::{set_cfg, set_ini};
use crate::device::Tier;
use crate::formats::{entry, merge_json, merge_toml, merge_yaml, patch_xml};
use crate::manifest::Emulator;
use std::{fs, path::Path};

pub fn apply(emu: &Emulator, exe_dir: &Path, tier: Tier) -> anyhow::Result<()> {
    let high = tier == Tier::High;
    match emu.id.as_str() {
        "rpcs3" => rpcs3(exe_dir, high),
        "shadps4" => shadps4(exe_dir, high),
        "xemu" => xemu(exe_dir, high),
        "retroarch" => retroarch(exe_dir, high),
        "duckstation" => duckstation(exe_dir, high),
        "pcsx2" => pcsx2(exe_dir, high),
        "ppsspp" => ppsspp(exe_dir, high),
        "dolphin" => dolphin(exe_dir, high),
        "cemu" => cemu(exe_dir),
        "melonds" => melonds(exe_dir, high),
        "azahar" => azahar(exe_dir, high),
        // Xenia rewrites its whole TOML and replays default migrations, so its
        // settings go on the command line instead (see `launch_args`).
        _ => Ok(()),
    }
}

/// Extra arguments placed before the emulator's own launch arguments.
pub fn launch_args(emu: &Emulator, exe_dir: &Path, tier: Tier, fresh: bool) -> Vec<String> {
    let high = tier == Tier::High;
    match emu.id.as_str() {
        // Our settings live in their own file layered over retroarch.cfg, so
        // the user's own RetroArch config is never rewritten.
        "retroarch" => {
            let mut layers = vec![exe_dir.join(RETROARCH_OVERRIDES)];
            if fresh {
                layers.push(exe_dir.join(RETROARCH_FRESH));
            }
            let layers: Vec<String> = layers
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            vec!["--appendconfig".into(), layers.join("|")]
        }
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
        // SharpEmu's CLI flags override its GUI settings file without touching it.
        "sharpemu" => [
            "--window-mode=borderless",
            "--scaling=fit",
            "--vsync=on",
            "--hdr=off",
            "--overlay=off",
        ]
        .map(String::from)
        .into(),
        _ => Vec::new(),
    }
}

/// Environment variables the emulator's own launcher would set.
pub fn launch_env(emu: &Emulator) -> &'static [(&'static str, &'static str)] {
    match emu.id.as_str() {
        // SharpEmu's GUI enables this by default; many PS5 games write to their
        // own install folder (app0) and fail to boot without it.
        "sharpemu" => &[("SHARPEMU_WRITABLE_APP0", "1")],
        _ => &[],
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
                true,
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

fn dolphin(exe_dir: &Path, high: bool) -> anyhow::Result<()> {
    let config = exe_dir.join("User").join("Config");
    set_ini(
        &config.join("Dolphin.ini"),
        &[
            ("Core", "GFXBackend", "Vulkan".into()),
            ("Core", "SIDevice0", "6".into()),
            ("Display", "Fullscreen", "True".into()),
            ("Display", "RenderToMain", "False".into()),
            ("Interface", "ConfirmStop", "False".into()),
            ("Interface", "CursorVisibility", "0".into()),
            ("Interface", "PauseOnFocusLost", "True".into()),
            ("Interface", "UsePanicHandlers", "False".into()),
            ("Analytics", "PermissionAsked", "True".into()),
            ("Analytics", "Enabled", "False".into()),
            // Empty track = no update check.
            ("AutoUpdate", "UpdateTrack", String::new()),
        ],
    )?;
    set_ini(
        &config.join("GFX.ini"),
        &[
            // Multiple of native 640x528: 2x ≈ 720p, 3x ≈ 1080p.
            (
                "Settings",
                "InternalResolution",
                if high { "3" } else { "2" }.into(),
            ),
            // Asynchronous ubershaders: no stutter while shaders compile.
            ("Settings", "ShaderCompilationMode", "2".into()),
            ("Settings", "WaitForShadersBeforeStarting", "True".into()),
            ("Settings", "AspectRatio", "0".into()),
            ("Settings", "BorderlessFullscreen", "True".into()),
            ("Settings", "ShowFPS", "False".into()),
            ("Hardware", "VSync", "True".into()),
            (
                "Enhancements",
                "MaxAnisotropy",
                if high { "4" } else { "2" }.into(),
            ),
        ],
    )
}

/// Written before Cemu's first start, which otherwise opens its setup wizard.
const CEMU_SETTINGS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<content>
    <fullscreen>true</fullscreen>
    <fullscreen_menubar>false</fullscreen_menubar>
    <check_update>false</check_update>
    <receive_untested_updates>false</receive_untested_updates>
    <Graphic>
        <api>1</api>
        <VSync>1</VSync>
        <AsyncCompile>true</AsyncCompile>
        <FullscreenScaling>0</FullscreenScaling>
        <UpscaleFilter>2</UpscaleFilter>
        <DownscaleFilter>0</DownscaleFilter>
    </Graphic>
</content>
"#;

fn cemu(exe_dir: &Path) -> anyhow::Result<()> {
    // Internal resolution comes from per-game graphic packs, so Cemu has no
    // tier-specific values here.
    let path = exe_dir.join("portable").join("settings.xml");
    let Ok(mut xml) = fs::read_to_string(&path) else {
        fs::create_dir_all(exe_dir.join("portable"))?;
        fs::write(&path, CEMU_SETTINGS)?;
        return Ok(());
    };
    // Cemu rewrites the whole file on save; patch only our elements.
    for (tag, value) in [
        ("fullscreen", "true"),
        ("fullscreen_menubar", "false"),
        ("check_update", "false"),
    ] {
        xml = patch_xml(&xml, None, tag, value);
    }
    for (tag, value) in [("api", "1"), ("VSync", "1"), ("AsyncCompile", "true")] {
        xml = patch_xml(&xml, Some("Graphic"), tag, value);
    }
    fs::write(&path, xml)?;
    Ok(())
}

fn melonds(exe_dir: &Path, high: bool) -> anyhow::Result<()> {
    merge_toml(
        &exe_dir.join("portable").join("melonDS.toml"),
        &[
            // 1 OpenGL, 2 OpenGL compute (sharper upscaling, needs GL 4.3).
            entry(&["3D"], "Renderer", if high { 2 } else { 1 }),
            entry(&["3D", "GL"], "ScaleFactor", if high { 6 } else { 4 }),
            entry(&["3D", "GL"], "BetterPolygons", true),
            entry(&["3D", "Soft"], "Threaded", true),
            entry(&["Screen"], "UseGL", true),
            entry(&["Screen"], "VSync", false),
            // Built-in FreeBIOS: no BIOS files needed.
            entry(&["Emu"], "ExternalBIOSEnable", false),
            entry(&["Emu"], "DirectBoot", true),
            entry(&["Emu"], "ConsoleType", 0),
            entry(&["DSi"], "ExternalBIOSEnable", false),
            // Side by side suits a 16:9 screen.
            entry(&["Instance0", "Window0"], "ScreenLayout", 2),
            entry(&["Instance0", "Window0"], "ScreenSizing", 0),
            entry(&["Instance0", "Window0"], "ShowOSD", false),
            entry(&[], "PauseLostFocus", true),
            entry(&["Mouse"], "Hide", true),
            entry(&["Mouse"], "HideSeconds", 1),
        ],
    )
}

fn azahar(exe_dir: &Path, high: bool) -> anyhow::Result<()> {
    // Switchable settings need `\default=false`, or Azahar applies its built-in
    // default instead of our value.
    let mut entries: Vec<(&str, String, String)> = Vec::new();
    let mut setting = |group: &'static str, key: &str, value: &str| {
        entries.push((group, key.to_string(), value.to_string()));
        entries.push((group, format!(r"{key}\default"), "false".into()));
        entries.push((group, format!(r"{key}\use_global"), "true".into()));
    };
    setting("Renderer", "graphics_api", "2");
    setting(
        "Renderer",
        "resolution_factor",
        if high { "4" } else { "2" },
    );
    setting("Renderer", "async_shader_compilation", "true");
    setting("Renderer", "use_disk_shader_cache", "true");
    setting("Renderer", "use_vsync", "true");
    setting("Renderer", "texture_filter", "0");
    setting("Layout", "layout_option", "2");
    setting("Layout", "large_screen_proportion", "4");
    setting("Layout", "small_screen_position", "2");
    setting("UI", "fullscreen", "true");
    setting("UI", "confirmClose", "false");
    setting("UI", "pauseWhenInBackground", "true");
    setting("UI", "hideInactiveMouse", "true");
    setting("UI", "singleWindowMode", "true");
    setting("Miscellaneous", "check_for_update_on_start", "false");

    let refs: Vec<(&str, &str, String)> = entries
        .iter()
        .map(|(g, k, v)| (*g, k.as_str(), v.clone()))
        .collect();
    set_ini(
        &exe_dir.join("user").join("config").join("qt-config.ini"),
        &refs,
    )
}

const RETROARCH_OVERRIDES: &str = "lumen.cfg";
/// Layered on top for "start over": skips loading the auto save state.
const RETROARCH_FRESH: &str = "lumen-fresh.cfg";

fn retroarch(exe_dir: &Path, high: bool) -> anyhow::Result<()> {
    let on = |v: bool| if v { "true" } else { "false" }.to_string();
    let path = |p: std::path::PathBuf| p.to_string_lossy().into_owned();
    set_cfg(
        &exe_dir.join(RETROARCH_OVERRIDES),
        &[
            // Vulkan also enables ParaLLEl-RDP for N64; GL-only cores may switch.
            ("video_driver", "vulkan".into()),
            ("driver_switch_enable", on(true)),
            ("video_fullscreen", on(true)),
            ("video_windowed_fullscreen", on(true)),
            ("video_vsync", on(true)),
            // Auto: 60 fps content gets swap interval 2 on the 120 Hz panel.
            ("video_swap_interval", "0".into()),
            ("video_threaded", on(false)),
            ("video_frame_delay_auto", on(high)),
            ("audio_driver", "wasapi".into()),
            ("audio_latency", "64".into()),
            ("input_joypad_driver", "xinput".into()),
            ("input_autodetect_enable", on(true)),
            ("confirm_quit", on(false)),
            // L3 + R3 opens RetroArch's menu; quitting is Lumen's Select + Start.
            ("input_menu_toggle_gamepad_combo", "2".into()),
            ("input_quit_gamepad_combo", "0".into()),
            // Saves and states in fixed folders next to RetroArch (not beside the
            // ROMs), so resume detection and backups know where to look.
            ("savefile_directory", path(exe_dir.join("saves"))),
            ("savestate_directory", path(exe_dir.join("states"))),
            // Resume where you left off.
            ("savestate_auto_save", on(true)),
            ("savestate_auto_load", on(true)),
            // Quick menu slots: Lumen sends SAVE/LOAD_STATE_SLOT on the local
            // command port; each state gets a screenshot to show there.
            ("network_cmd_enable", on(true)),
            ("network_cmd_port", crate::states::PORT.to_string()),
            ("savestate_thumbnail_enable", on(true)),
            ("sort_savestates_enable", on(false)),
            ("sort_savestates_by_content_enable", on(false)),
            // Core options below apply everywhere; stale per-game files can't override them.
            ("global_core_options", on(true)),
            ("game_specific_options", on(false)),
            ("pause_nonactive", on(true)),
            ("menu_driver", "ozone".into()),
            ("config_save_on_exit", on(false)),
        ],
    )?;
    set_cfg(
        &exe_dir.join(RETROARCH_FRESH),
        &[("savestate_auto_load", on(false))],
    )?;
    set_cfg(
        &exe_dir.join("retroarch-core-options.cfg"),
        &[
            // N64: ParaLLEl RDP/RSP (Vulkan), upscaled on high tier.
            ("mupen64plus-rdp-plugin", "parallel".into()),
            ("mupen64plus-rsp-plugin", "parallel".into()),
            (
                "mupen64plus-parallel-rdp-upscaling",
                if high { "2x" } else { "1x" }.into(),
            ),
            ("mupen64plus-parallel-rdp-synchronous", "True".into()),
            ("mupen64plus-cpucore", "dynamic_recompiler".into()),
            ("mupen64plus-aspect", "4:3".into()),
            // Dreamcast (Flycast keeps the old `reicast_` prefix).
            (
                "reicast_internal_resolution",
                if high { "1920x1440" } else { "1280x960" }.into(),
            ),
            (
                "reicast_alpha_sorting",
                if high {
                    "per-pixel (accurate)"
                } else {
                    "per-triangle (normal)"
                }
                .into(),
            ),
            ("reicast_threaded_rendering", "enabled".into()),
            ("reicast_widescreen_hack", "disabled".into()),
            (
                "reicast_anisotropic_filtering",
                if high { "16" } else { "4" }.into(),
            ),
            // 2D systems: correct aspect and handheld colors.
            ("snes9x_aspect", "4:3".into()),
            ("mesen_aspect_ratio", "Auto".into()),
            ("genesis_plus_gx_aspect_ratio", "auto".into()),
            ("gambatte_gbc_color_correction", "GBC only".into()),
            ("mgba_color_correction", "Auto".into()),
            ("mgba_skip_bios", "ON".into()),
        ],
    )
}

fn duckstation(exe_dir: &Path, high: bool) -> anyhow::Result<()> {
    set_ini(
        &exe_dir.join("settings.ini"),
        &[
            ("Main", "StartFullscreen", "true".into()),
            ("Main", "HideCursorInFullscreen", "true".into()),
            ("Main", "ConfirmPowerOff", "false".into()),
            ("Main", "SaveStateOnExit", "true".into()),
            ("Main", "PauseOnFocusLoss", "true".into()),
            // The panel runs at 120 Hz; don't speed games up to match it.
            ("Main", "SyncToHostRefreshRate", "false".into()),
            ("Main", "ApplyCompatibilitySettings", "true".into()),
            ("AutoUpdater", "CheckAtStartup", "false".into()),
            ("GPU", "Renderer", "Vulkan".into()),
            (
                "GPU",
                "ResolutionScale",
                if high { "6" } else { "4" }.into(),
            ),
            // PGXP: fixes PS1 polygon wobble.
            ("GPU", "PGXPEnable", "true".into()),
            ("GPU", "PGXPCulling", "true".into()),
            ("GPU", "PGXPTextureCorrection", "true".into()),
            ("GPU", "PGXPDepthBuffer", "false".into()),
            ("GPU", "TextureFilter", "Nearest".into()),
            ("GPU", "WidescreenHack", "false".into()),
            ("GPU", "UseThread", "true".into()),
            ("Display", "VSync", "true".into()),
            ("Display", "OptimalFramePacing", "true".into()),
            ("BIOS", "PatchFastBoot", "true".into()),
        ],
    )
}

fn pcsx2(exe_dir: &Path, high: bool) -> anyhow::Result<()> {
    set_ini(
        &exe_dir.join("inis").join("PCSX2.ini"),
        &[
            ("UI", "SettingsVersion", "1".into()),
            ("UI", "StartFullscreen", "true".into()),
            ("UI", "ConfirmShutdown", "false".into()),
            ("UI", "HideMouseCursor", "true".into()),
            ("UI", "PauseOnFocusLoss", "true".into()),
            ("AutoUpdater", "CheckAtStartup", "false".into()),
            // 14 = Vulkan (GSRendererType).
            ("EmuCore/GS", "Renderer", "14".into()),
            (
                "EmuCore/GS",
                "upscale_multiplier",
                if high { "3" } else { "2" }.into(),
            ),
            ("EmuCore/GS", "filter", "2".into()),
            ("EmuCore/GS", "TriFilter", "-1".into()),
            // Blending accuracy: 1 Basic, 2 Medium.
            (
                "EmuCore/GS",
                "accurate_blending_unit",
                if high { "2" } else { "1" }.into(),
            ),
            ("EmuCore/GS", "mipmap", "true".into()),
            ("EmuCore/GS", "hw_mipmap", "true".into()),
            ("EmuCore/GS", "UserHacks", "false".into()),
            ("EmuCore/GS", "VsyncEnable", "true".into()),
            ("EmuCore/GS", "SyncToHostRefreshRate", "false".into()),
            // Multi-threaded VU1; PCSX2 only auto-detects it when it writes defaults itself.
            ("EmuCore/Speedhacks", "vuThread", "true".into()),
            ("EmuCore", "EnableFastBoot", "true".into()),
            ("EmuCore", "SaveStateOnShutdown", "true".into()),
        ],
    )
}

fn ppsspp(exe_dir: &Path, high: bool) -> anyhow::Result<()> {
    // Without installed.txt PPSSPP keeps its memstick next to the exe.
    set_ini(
        &exe_dir
            .join("memstick")
            .join("PSP")
            .join("SYSTEM")
            .join("ppsspp.ini"),
        &[
            ("General", "FirstRun", "False".into()),
            ("General", "CheckForNewVersion", "False".into()),
            ("General", "AskForExitConfirmationAfterSeconds", "0".into()),
            ("General", "PauseOnLostFocus", "True".into()),
            ("CPU", "CPUCore", "1".into()),
            ("CPU", "FastMemoryAccess", "True".into()),
            ("Graphics", "GraphicsBackend", "3 (VULKAN)".into()),
            // Multiples of 480x272: 3x ≈ 1440x816, 4x ≈ 1920x1088.
            (
                "Graphics",
                "InternalResolution",
                if high { "4" } else { "3" }.into(),
            ),
            ("Graphics", "FrameSkip", "0".into()),
            ("Graphics", "AutoFrameSkip", "False".into()),
            ("Graphics", "VerticalSync", "True".into()),
            ("Graphics", "FullScreen", "True".into()),
            ("Graphics", "TextureFiltering", "1".into()),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::manifest;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("lumen-presets-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn every_preset_applies_twice_on_both_tiers() {
        for tier in [Tier::Standard, Tier::High] {
            for emu in &manifest().emulators {
                let dir = temp_dir(&format!("{}-{tier:?}", emu.id));
                // Twice: the second run must merge into what the first wrote.
                apply(emu, &dir, tier).unwrap_or_else(|e| panic!("{}: {e}", emu.id));
                apply(emu, &dir, tier).unwrap_or_else(|e| panic!("{} again: {e}", emu.id));
            }
        }
    }

    #[test]
    fn structured_configs_stay_parseable_and_keep_emulator_state() {
        let dir = temp_dir("state");
        fs::create_dir_all(dir.join("config")).unwrap();
        fs::write(
            dir.join("config/config.yml"),
            "System:\n  Console PSID: CAFE\n",
        )
        .unwrap();
        rpcs3(&dir, true).unwrap();
        let yml: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&fs::read_to_string(dir.join("config/config.yml")).unwrap())
                .unwrap();
        assert_eq!(yml["System"]["Console PSID"].as_str(), Some("CAFE"));
        assert_eq!(yml["Video"]["Resolution Scale"].as_i64(), Some(150));

        shadps4(&dir, false).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join("user/config.json")).unwrap())
                .unwrap();
        assert_eq!(json["GPU"]["full_screen"], serde_json::Value::Bool(true));
        assert_eq!(
            json["GPU"]["internal_screen_width"],
            serde_json::json!(1280)
        );

        xemu(&dir, true).unwrap();
        let toml: toml_edit::DocumentMut = fs::read_to_string(dir.join("xemu.toml"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(toml["display"]["renderer"].as_str(), Some("VULKAN"));
    }

    #[test]
    fn xenia_settings_go_on_the_command_line() {
        let xenia = crate::manifest::emulator("xenia").unwrap();
        let standard = launch_args(xenia, Path::new("."), Tier::Standard, false);
        let high = launch_args(xenia, Path::new("."), Tier::High, false);
        assert!(standard.contains(&"--draw_resolution_scale_x=1".to_string()));
        assert!(high.contains(&"--draw_resolution_scale_x=2".to_string()));
        assert!(standard.iter().all(|a| !a.ends_with('=')), "no empty cvars");
    }

    #[test]
    fn sharpemu_runs_borderless_with_a_writable_app0() {
        let sharpemu = crate::manifest::emulator("sharpemu").unwrap();
        let args = launch_args(sharpemu, Path::new("."), Tier::Standard, false);
        assert!(args.contains(&"--window-mode=borderless".to_string()));
        assert!(args.contains(&"--overlay=off".to_string()));
        assert_eq!(launch_env(sharpemu), &[("SHARPEMU_WRITABLE_APP0", "1")]);
    }
}
