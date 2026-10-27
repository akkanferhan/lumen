//! Downloads emulators and libretro cores from their official sources and
//! unpacks them into `<app data>/emulators/<id>`.

use crate::config;
use crate::manifest::{Emulator, Source};
use anyhow::{anyhow, Context};
use futures_util::StreamExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

pub const PROGRESS_EVENT: &str = "install-progress";
const CORES_URL: &str = "https://buildbot.libretro.com/nightly/windows/x86_64/latest";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// Emulator id, or `core:<name>` for libretro cores.
    pub id: String,
    pub stage: &'static str,
    pub progress: f32,
    pub message: Option<String>,
}

pub fn emit(app: &AppHandle, id: &str, stage: &'static str, progress: f32, message: Option<String>) {
    let _ = app.emit(PROGRESS_EVENT, Progress { id: id.to_string(), stage, progress, message });
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("Lumen/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .build()
            .expect("http client")
    })
}

pub fn find_exe(emu: &Emulator, install_dir: &Path) -> Option<PathBuf> {
    emu.exe.iter().find_map(|pattern| crate::library::find_file(install_dir, pattern))
}

pub async fn install_emulator(
    app: &AppHandle,
    emu: &Emulator,
    emulators_dir: &Path,
    downloads_dir: &Path,
    bios: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    emit(app, &emu.id, "downloading", 0.0, None);
    let url = resolve_url(&emu.source).await?;
    let target = emulators_dir.join(&emu.id);
    let archive = download(app, &emu.id, &url, downloads_dir).await?;

    emit(app, &emu.id, "extracting", 1.0, None);
    // Extract over an existing install so portable saves and settings survive updates.
    unpack(archive, target.clone()).await?;

    emit(app, &emu.id, "configuring", 1.0, None);
    let exe = find_exe(emu, &target).ok_or_else(|| anyhow!("{} çalıştırılabilir dosyası bulunamadı", emu.name))?;
    let exe_dir = exe.parent().unwrap_or(&target);
    for marker in &emu.portable_files {
        let path = exe_dir.join(marker);
        if !path.exists() {
            std::fs::write(path, "")?;
        }
    }
    config::configure(emu, exe_dir, bios)?;

    emit(app, &emu.id, "done", 1.0, None);
    Ok(exe)
}

pub async fn install_core(app: &AppHandle, core: &str, cores_dir: &Path, downloads_dir: &Path) -> anyhow::Result<()> {
    let id = format!("core:{core}");
    emit(app, &id, "downloading", 0.0, None);
    let url = format!("{CORES_URL}/{core}_libretro.dll.zip");
    let archive = download(app, &id, &url, downloads_dir).await?;
    emit(app, &id, "extracting", 1.0, None);
    unpack(archive, cores_dir.to_path_buf()).await?;
    emit(app, &id, "done", 1.0, None);
    Ok(())
}

async fn resolve_url(source: &Source) -> anyhow::Result<String> {
    match source {
        Source::Url { url } => Ok(url.clone()),
        Source::Github { repo, asset, tag } => {
            #[derive(Deserialize)]
            struct Release {
                assets: Vec<Asset>,
            }
            #[derive(Deserialize)]
            struct Asset {
                name: String,
                browser_download_url: String,
            }

            let api = match tag {
                Some(t) => format!("https://api.github.com/repos/{repo}/releases/tags/{t}"),
                None => format!("https://api.github.com/repos/{repo}/releases/latest"),
            };
            let release: Release = client()
                .get(&api)
                .header("Accept", "application/vnd.github+json")
                .send()
                .await?
                .error_for_status()
                .with_context(|| format!("{repo} sürüm bilgisi alınamadı"))?
                .json()
                .await?;

            let pattern = Regex::new(asset)?;
            release
                .assets
                .into_iter()
                .find(|a| pattern.is_match(&a.name))
                .map(|a| a.browser_download_url)
                .ok_or_else(|| anyhow!("{repo} sürümünde Windows paketi bulunamadı"))
        }
    }
}

async fn download(app: &AppHandle, id: &str, url: &str, downloads_dir: &Path) -> anyhow::Result<PathBuf> {
    let response = client()
        .get(url)
        .send()
        .await?
        .error_for_status()
        .with_context(|| format!("İndirme başarısız: {url}"))?;
    let total = response.content_length();

    let file_name = url.rsplit('/').next().unwrap_or("download").split('?').next().unwrap_or("download");
    let path = downloads_dir.join(format!("{}-{file_name}", id.replace(':', "_")));
    let mut file = tokio::fs::File::create(&path).await?;

    let mut stream = response.bytes_stream();
    let mut received = 0u64;
    let mut last_emit = Instant::now();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        received += chunk.len() as u64;
        if last_emit.elapsed() >= Duration::from_millis(120) {
            last_emit = Instant::now();
            let progress = total.map_or(0.0, |t| received as f32 / t as f32);
            emit(app, id, "downloading", progress, None);
        }
    }
    file.flush().await?;
    Ok(path)
}

async fn unpack(archive: PathBuf, target: PathBuf) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&target)?;
        let name = archive.to_string_lossy().to_lowercase();
        let result = if name.ends_with(".7z") {
            sevenz_rust::decompress_file(&archive, &target).map_err(|e| anyhow!("7z açılamadı: {e}"))
        } else if name.ends_with(".zip") {
            let file = std::fs::File::open(&archive)?;
            zip::ZipArchive::new(file)?.extract(&target).map_err(|e| anyhow!("zip açılamadı: {e}"))
        } else {
            Err(anyhow!("Desteklenmeyen arşiv: {name}"))
        };
        let _ = std::fs::remove_file(&archive);
        result
    })
    .await?
}
