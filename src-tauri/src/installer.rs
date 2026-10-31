//! Downloads emulators and libretro cores from their official sources and
//! unpacks them into `<app data>/emulators/<id>`.

use crate::config;
use crate::device::Tier;
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

pub fn emit(
    app: &AppHandle,
    id: &str,
    stage: &'static str,
    progress: f32,
    message: Option<String>,
) {
    let _ = app.emit(
        PROGRESS_EVENT,
        Progress {
            id: id.to_string(),
            stage,
            progress,
            message,
        },
    );
}

/// Shared HTTP client (user agent, timeouts, OS certificate store).
pub fn http() -> &'static reqwest::Client {
    client()
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
    emu.exe
        .iter()
        .find_map(|pattern| crate::library::find_file(install_dir, pattern))
}

pub async fn install_emulator(
    app: &AppHandle,
    emu: &Emulator,
    emulators_dir: &Path,
    downloads_dir: &Path,
    bios: Option<&Path>,
    tier: Tier,
) -> anyhow::Result<PathBuf> {
    emit(app, &emu.id, "downloading", 0.0, None);
    let release = resolve_release(&emu.source).await?;
    let target = emulators_dir.join(&emu.id);
    let archive = download(app, &emu.id, &release.url, downloads_dir).await?;

    emit(app, &emu.id, "extracting", 1.0, None);
    // Extract over an existing install so portable saves and settings survive updates.
    unpack(archive, target.clone()).await?;

    emit(app, &emu.id, "configuring", 1.0, None);
    let exe = find_exe(emu, &target).ok_or_else(|| {
        anyhow!(crate::tr!(
            "{} çalıştırılabilir dosyası bulunamadı",
            "Couldn't find {}'s program file",
            emu.name
        ))
    })?;
    let exe_dir = exe.parent().unwrap_or(&target);
    for marker in &emu.portable_files {
        let path = exe_dir.join(marker);
        if !path.exists() {
            std::fs::write(path, "")?;
        }
    }
    config::configure(emu, exe_dir, bios, tier)?;
    std::fs::write(target.join(VERSION_FILE), &release.version)?;

    emit(app, &emu.id, "done", 1.0, None);
    Ok(exe)
}

/// Records which build is installed, so updates compare against it.
const VERSION_FILE: &str = ".lumen-version";

pub fn installed_version(install_dir: &Path) -> Option<String> {
    std::fs::read_to_string(install_dir.join(VERSION_FILE)).ok()
}

pub async fn install_core(
    app: &AppHandle,
    core: &str,
    cores_dir: &Path,
    downloads_dir: &Path,
) -> anyhow::Result<()> {
    let id = format!("core:{core}");
    emit(app, &id, "downloading", 0.0, None);
    let url = format!("{CORES_URL}/{core}_libretro.dll.zip");
    let archive = download(app, &id, &url, downloads_dir).await?;
    emit(app, &id, "extracting", 1.0, None);
    unpack(archive, cores_dir.to_path_buf()).await?;
    emit(app, &id, "done", 1.0, None);
    Ok(())
}

/// Downloads a single file (no unpacking) and returns its path.
pub async fn download_file(
    app: &AppHandle,
    id: &str,
    url: &str,
    downloads_dir: &Path,
) -> anyhow::Result<PathBuf> {
    emit(app, id, "downloading", 0.0, None);
    download(app, id, url, downloads_dir).await
}

/// Sony's official PS3 system software list (plain HTTP is all Sony serves).
const PS3_UPDATE_LIST: &str =
    "http://fus01.ps3.update.playstation.net/update/ps3/list/us/ps3-updatelist.txt";

/// Downloads the PS3 system software from Sony, which distributes it for free.
pub async fn download_ps3_firmware(
    app: &AppHandle,
    id: &str,
    downloads_dir: &Path,
) -> anyhow::Result<PathBuf> {
    emit(app, id, "downloading", 0.0, None);
    let list = client()
        .get(PS3_UPDATE_LIST)
        .send()
        .await?
        .error_for_status()
        .with_context(|| {
            crate::tr!(
                "Sony güncelleme listesine ulaşılamadı",
                "Couldn't reach Sony's update list"
            )
        })?
        .text()
        .await?;
    let url = ps3_firmware_url(&list).ok_or_else(|| {
        anyhow!(crate::tr!(
            "PS3 firmware bağlantısı bulunamadı",
            "Couldn't find the PS3 firmware link"
        ))
    })?;
    download(app, id, &url, downloads_dir).await
}

fn ps3_firmware_url(list: &str) -> Option<String> {
    list.split([';', '\n'])
        .filter_map(|field| field.trim().strip_prefix("CDN="))
        .find(|url| url.ends_with("/PS3UPDAT.PUP"))
        .map(String::from)
}

fn xbox_hdd_source() -> Source {
    Source::Github {
        repo: "xemu-project/xemu-hdd-image".into(),
        asset: r"xbox_hdd\.qcow2\.zip$".into(),
        tag: None,
    }
}

/// xemu's own blank, pre-formatted Xbox HDD image (no Microsoft content).
pub async fn ensure_xbox_hdd(
    app: &AppHandle,
    exe_dir: &Path,
    downloads_dir: &Path,
) -> anyhow::Result<()> {
    if crate::library::find_file(exe_dir, "**/xbox_hdd.qcow2").is_some() {
        return Ok(());
    }
    let id = "hdd:xbox";
    let result = async {
        let url = resolve_url(&xbox_hdd_source()).await?;
        let archive = download(app, id, &url, downloads_dir).await?;
        emit(app, id, "extracting", 1.0, None);
        unpack(archive, exe_dir.to_path_buf()).await
    }
    .await;
    match &result {
        Ok(()) => emit(app, id, "done", 1.0, None),
        Err(e) => emit(app, id, "error", 0.0, Some(e.to_string())),
    }
    result
}

async fn resolve_url(source: &Source) -> anyhow::Result<String> {
    Ok(resolve_release(source).await?.url)
}

/// A downloadable build and an id that changes whenever the build does.
pub struct Release {
    pub url: String,
    pub version: String,
}

pub async fn resolve_release(source: &Source) -> anyhow::Result<Release> {
    match source {
        // Rolling URLs (RetroArch nightly): the server's ETag/Last-Modified tells builds apart.
        Source::Url { url } => {
            let head = client().head(url).send().await?.error_for_status()?;
            let stamp = ["etag", "last-modified"]
                .iter()
                .find_map(|h| head.headers().get(*h)?.to_str().ok().map(String::from))
                .unwrap_or_default();
            Ok(Release {
                url: url.clone(),
                version: format!("{url}@{stamp}"),
            })
        }
        Source::Github { repo, asset, tag } => {
            #[derive(Deserialize)]
            struct GithubRelease {
                tag_name: String,
                assets: Vec<Asset>,
            }
            #[derive(Deserialize)]
            struct Asset {
                name: String,
                browser_download_url: String,
                updated_at: String,
            }

            let api = match tag {
                Some(t) => format!("https://api.github.com/repos/{repo}/releases/tags/{t}"),
                None => format!("https://api.github.com/repos/{repo}/releases/latest"),
            };
            let mut request = client()
                .get(&api)
                .header("Accept", "application/vnd.github+json");
            // Optional; only lifts the 60 requests/hour anonymous limit (used by CI).
            if let Ok(token) = std::env::var("GITHUB_TOKEN") {
                request = request.bearer_auth(token);
            }
            let release: GithubRelease = request
                .send()
                .await?
                .error_for_status()
                .with_context(|| {
                    crate::tr!(
                        "{} sürüm bilgisi alınamadı",
                        "Couldn't get {} release info",
                        repo
                    )
                })?
                .json()
                .await?;

            let pattern = Regex::new(asset)?;
            let tag_name = release.tag_name;
            release
                .assets
                .into_iter()
                .find(|a| pattern.is_match(&a.name))
                // Rolling tags (DuckStation's `latest`) keep the URL; updated_at moves.
                .map(|a| Release {
                    version: format!("{tag_name}/{}/{}", a.name, a.updated_at),
                    url: a.browser_download_url,
                })
                .ok_or_else(|| {
                    anyhow!(crate::tr!(
                        "{} sürümünde Windows paketi bulunamadı",
                        "No Windows package found in the {} release",
                        repo
                    ))
                })
        }
        Source::Page {
            page,
            pattern,
            base,
        } => {
            let body = client()
                .get(page)
                .send()
                .await?
                .error_for_status()
                .with_context(|| {
                    crate::tr!(
                        "İndirme sayfası açılamadı: {}",
                        "Couldn't open the download page: {}",
                        page
                    )
                })?
                .text()
                .await?;
            let found = Regex::new(pattern)?
                .find(&body)
                .map(|m| m.as_str().to_string())
                .ok_or_else(|| {
                    anyhow!(crate::tr!(
                        "İndirme bağlantısı bulunamadı: {}",
                        "Couldn't find the download link: {}",
                        page
                    ))
                })?;
            let url = match base {
                Some(base) if found.starts_with('/') => format!("{base}{found}"),
                _ => found,
            };
            // Page links carry the version (dolphin-2609a, 1_20_4).
            Ok(Release {
                version: url.clone(),
                url,
            })
        }
    }
}

pub async fn download_to(
    app: &AppHandle,
    id: &str,
    url: &str,
    target: &Path,
    downloads_dir: &Path,
) -> anyhow::Result<()> {
    emit(app, id, "downloading", 0.0, None);
    let temp = download(app, id, url, downloads_dir).await?;
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    // Copy + remove: downloads may sit on a different drive than the ROM folder.
    tokio::fs::copy(&temp, target).await?;
    let _ = tokio::fs::remove_file(&temp).await;
    emit(app, id, "done", 1.0, None);
    Ok(())
}

/// Downloads an archive and unpacks it into `target_dir`.
pub async fn download_and_unpack(
    app: &AppHandle,
    id: &str,
    url: &str,
    target_dir: &Path,
    downloads_dir: &Path,
) -> anyhow::Result<()> {
    emit(app, id, "downloading", 0.0, None);
    let archive = download(app, id, url, downloads_dir).await?;
    emit(app, id, "extracting", 1.0, None);
    unpack(archive, target_dir.to_path_buf()).await?;
    emit(app, id, "done", 1.0, None);
    Ok(())
}

async fn download(
    app: &AppHandle,
    id: &str,
    url: &str,
    downloads_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let response = client()
        .get(url)
        .send()
        .await?
        .error_for_status()
        .with_context(|| crate::tr!("İndirme başarısız: {}", "Download failed: {}", url))?;
    let total = response.content_length();

    let file_name = url
        .rsplit('/')
        .next()
        .unwrap_or("download")
        .split('?')
        .next()
        .unwrap_or("download");
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
            sevenz_rust::decompress_file(&archive, &target).map_err(|e| {
                anyhow!(crate::tr!(
                    "7z açılamadı: {}",
                    "Couldn't open the 7z: {}",
                    e
                ))
            })
        } else if name.ends_with(".zip") {
            let file = std::fs::File::open(&archive)?;
            zip::ZipArchive::new(file)?.extract(&target).map_err(|e| {
                anyhow!(crate::tr!(
                    "zip açılamadı: {}",
                    "Couldn't open the zip: {}",
                    e
                ))
            })
        } else {
            Err(anyhow!(crate::tr!(
                "Desteklenmeyen arşiv: {}",
                "Unsupported archive: {}",
                name
            )))
        };
        let _ = std::fs::remove_file(&archive);
        result
    })
    .await?
}

#[cfg(test)]
mod tests {
    use crate::manifest::manifest;

    /// Hits the real download sources. Run by the weekly `sources` workflow:
    /// `cargo test -- --ignored live_sources`.
    #[test]
    fn finds_full_ps3_firmware_in_sony_list() {
        let list = "# US\nDest=84;CompatibleSystemSoftwareVersion=4.9300-;\n\
            Dest=84;ImageVersion=00010b94;CDN=http://dus01.example/us/abc/PS3PATCH.PUP;CDN_Timeout=30;\n\
            Dest=84;ImageVersion=00010b94;SystemSoftwareVersion=4.9300;CDN=http://dus01.example/us/abc/PS3UPDAT.PUP;CDN_Timeout=30;\n";
        assert_eq!(
            super::ps3_firmware_url(list).as_deref(),
            Some("http://dus01.example/us/abc/PS3UPDAT.PUP")
        );
    }

    #[tokio::test]
    #[ignore = "needs network"]
    async fn live_sources_resolve() {
        let mut failures = Vec::new();
        for emu in &manifest().emulators {
            match super::resolve_url(&emu.source).await {
                Ok(url) => println!("{:<12} {url}", emu.id),
                Err(e) => failures.push(format!("{}: {e:#}", emu.id)),
            }
        }
        // Firmware and HDD sources fetched outside the manifest.
        let list = super::client()
            .get(super::PS3_UPDATE_LIST)
            .send()
            .await
            .and_then(|r| r.error_for_status());
        match list {
            Ok(r) => match super::ps3_firmware_url(&r.text().await.unwrap_or_default()) {
                Some(url) => println!("{:<12} {url}", "ps3-fw"),
                None => failures.push("ps3-fw: no PS3UPDAT.PUP in list".into()),
            },
            Err(e) => failures.push(format!("ps3-fw: {e}")),
        }
        for (name, url) in [
            ("vita-fw", crate::vita::FIRMWARE),
            ("vita-fonts", crate::vita::FONTS),
        ] {
            match super::client()
                .head(url)
                .send()
                .await
                .and_then(|r| r.error_for_status())
            {
                Ok(_) => println!("{name:<12} {url}"),
                Err(e) => failures.push(format!("{name}: {e}")),
            }
        }
        match super::resolve_url(&super::xbox_hdd_source()).await {
            Ok(url) => println!("{:<12} {url}", "xbox-hdd"),
            Err(e) => failures.push(format!("xbox-hdd: {e:#}")),
        }
        assert!(
            failures.is_empty(),
            "unresolved sources:\n{}",
            failures.join("\n")
        );
    }
}
