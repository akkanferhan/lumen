//! Display filters for RetroArch systems, picked by name instead of shader
//! settings: Original (sharp pixels), Smooth, CRT and LCD, or Auto (LCD on
//! handhelds, CRT on 2D consoles and arcade, sharp pixels on 3D systems).
//! Each launch writes a small config layer and passes the shader preset;
//! the slang shaders are downloaded once if RetroArch's folder lacks them.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilterSetting {
    Auto,
    #[default]
    Original,
    Smooth,
    Crt,
    Lcd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    Original,
    Smooth,
    Crt,
    Lcd,
}

const SHADERS_URL: &str = "https://buildbot.libretro.com/assets/frontend/shaders_slang.zip";
pub const SHADERS_ID: &str = "shaders";
/// Layered after lumen.cfg on each launch (see `launcher`).
pub const LAYER: &str = "lumen-filter.cfg";

impl FilterSetting {
    pub fn resolve(self, system_id: &str) -> Filter {
        match self {
            FilterSetting::Original => Filter::Original,
            FilterSetting::Smooth => Filter::Smooth,
            FilterSetting::Crt => Filter::Crt,
            FilterSetting::Lcd => Filter::Lcd,
            FilterSetting::Auto => match system_id {
                "gb" | "gbc" | "gba" => Filter::Lcd,
                "nes" | "snes" | "sms" | "genesis" | "arcade" => Filter::Crt,
                _ => Filter::Original,
            },
        }
    }
}

impl Filter {
    pub fn needs_shader(self) -> bool {
        self.preset().is_some()
    }

    /// Preset path inside `shaders/shaders_slang`.
    fn preset(self) -> Option<&'static str> {
        match self {
            Filter::Crt => Some("crt/zfast-crt.slangp"),
            Filter::Lcd => Some("handheld/lcd-grid-v2.slangp"),
            Filter::Original | Filter::Smooth => None,
        }
    }
}

fn shaders_dir(exe_dir: &Path) -> PathBuf {
    exe_dir.join("shaders").join("shaders_slang")
}

/// The shader preset this filter needs, downloading the shader pack first
/// if it's missing. None for filters without a shader.
pub async fn ensure_preset(
    app: &tauri::AppHandle,
    filter: Filter,
    exe_dir: &Path,
    downloads: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    let Some(preset) = filter.preset() else {
        return Ok(None);
    };
    let path = shaders_dir(exe_dir).join(preset);
    if !path.exists() {
        crate::installer::download_and_unpack(
            app,
            SHADERS_ID,
            SHADERS_URL,
            &shaders_dir(exe_dir),
            downloads,
        )
        .await?;
    }
    Ok(path.exists().then_some(path))
}

/// The config layer for this launch, and the `--set-shader` arguments.
pub fn apply(exe_dir: &Path, filter: Filter, preset: Option<&Path>) -> anyhow::Result<Vec<String>> {
    let on = |v: bool| if v { "true" } else { "false" }.to_string();
    crate::config::set_cfg(
        &exe_dir.join(LAYER),
        &[
            ("video_smooth", on(filter == Filter::Smooth)),
            ("video_shader_enable", on(preset.is_some())),
        ],
    )?;
    Ok(match preset {
        Some(p) => vec!["--set-shader".into(), p.to_string_lossy().into_owned()],
        None => Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_fits_the_system() {
        assert_eq!(FilterSetting::Auto.resolve("gba"), Filter::Lcd);
        assert_eq!(FilterSetting::Auto.resolve("snes"), Filter::Crt);
        assert_eq!(FilterSetting::Auto.resolve("n64"), Filter::Original);
        assert_eq!(FilterSetting::Crt.resolve("n64"), Filter::Crt);
    }

    #[test]
    fn writes_the_layer_and_shader_args() {
        let dir = std::env::temp_dir().join(format!("lumen-filter-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let preset = dir.join("crt.slangp");
        let args = apply(&dir, Filter::Crt, Some(&preset)).unwrap();
        assert_eq!(args[0], "--set-shader");
        let cfg = std::fs::read_to_string(dir.join(LAYER)).unwrap();
        assert!(cfg.contains("video_shader_enable = \"true\""), "{cfg}");
        apply(&dir, Filter::Smooth, None).unwrap();
        let cfg = std::fs::read_to_string(dir.join(LAYER)).unwrap();
        assert!(cfg.contains("video_smooth = \"true\""), "{cfg}");
        assert!(cfg.contains("video_shader_enable = \"false\""), "{cfg}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
