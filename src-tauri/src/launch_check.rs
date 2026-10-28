//! Explains why an emulator didn't start, and what fixes it: Windows refusing
//! to run it (Smart App Control, Defender, policies) or the emulator closing
//! right away (a missing runtime, damaged files, a core that failed to load).
//! Without this the user only saw Lumen come back with nothing on screen.
//! `autofix` applies the fix.

use serde::Serialize;
use std::{
    io,
    path::{Path, PathBuf},
    time::Duration,
};

/// An exit this soon after launch is a failure, not a session.
const EARLY: Duration = Duration::from_secs(8);
/// Even with exit code 0: nobody plays for under this long.
const INSTANT: Duration = Duration::from_secs(3);

/// What gets a failed launch going again.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum Fix {
    /// Download the emulator again over the install (saves stay).
    ReinstallEmulator(String),
    /// Download a RetroArch core again.
    ReinstallCore(String),
    /// Install Microsoft's Visual C++ runtime.
    VcRedist,
    /// Security features stay the user's call: open their Windows page.
    SmartAppControl,
    Defender,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnosis {
    pub message: String,
    pub fix: Option<Fix>,
}

/// What the launch was: names for messages, ids for fixes.
pub struct Launch<'a> {
    pub emulator_id: &'a str,
    pub emulator: &'a str,
    pub core: Option<&'a str>,
}

impl Launch<'_> {
    fn reinstall(&self) -> Option<Fix> {
        Some(Fix::ReinstallEmulator(self.emulator_id.to_string()))
    }
}

/// An emulator Windows refused to start.
pub fn spawn_error(launch: &Launch, error: &io::Error) -> Diagnosis {
    let emulator = launch.emulator;
    let (message, fix) = match error.raw_os_error() {
        Some(2 | 3) => (format!("{emulator} dosyası eksik."), launch.reinstall()),
        Some(193 | 216) => (format!("{emulator} dosyası bozuk."), launch.reinstall()),
        Some(5) => (
            format!("Windows {emulator} için erişimi reddetti; bir güvenlik özelliği engelliyor olabilir."),
            None,
        ),
        Some(225 | 226) => (
            format!(
                "Windows Defender {emulator} dosyasını tehdit sanıp engelledi. Emülatör resmi kaynağından \
                 indirildi; izin vermek istersen Koruma geçmişi'nden verebilirsin."
            ),
            Some(Fix::Defender),
        ),
        Some(740) => (format!("{emulator} yönetici izni istedi; Lumen bunu vermiyor."), None),
        Some(1260) => (
            format!("{emulator} bu bilgisayarda bir ilkeyle (sistem yöneticisi) engellenmiş."),
            None,
        ),
        // ERROR_SYSTEM_INTEGRITY_POLICY_VIOLATION and friends.
        Some(4551 | 4553 | 4556) => (
            format!("Windows {emulator} uygulamasını engelledi."),
            Some(Fix::SmartAppControl),
        ),
        _ => (format!("{emulator} başlatılamadı: {error}"), None),
    };
    blocked_by_smart_app_control(Diagnosis { message, fix }, error.raw_os_error() == Some(5))
}

/// An emulator that closed right after starting, or None for a normal session.
pub fn early_exit(
    launch: &Launch,
    code: Option<i32>,
    ran: Duration,
    log: Option<&Path>,
) -> Option<Diagnosis> {
    let failed = ran < EARLY && code != Some(0) || ran < INSTANT;
    if !failed {
        return None;
    }
    let emulator = launch.emulator;
    // Windows exit codes are NTSTATUS values, negative as i32.
    let status = code.map(|c| c as u32);
    let (mut message, mut fix, blocked) = match status {
        Some(0xC000_0135) => (
            format!("{emulator} açılamadı: Microsoft Visual C++ çalışma zamanı eksik."),
            Some(Fix::VcRedist),
            false,
        ),
        Some(0xC000_007B) => (
            format!("{emulator} dosyaları bozuk."),
            launch.reinstall(),
            false,
        ),
        Some(0xC000_0022 | 0xC000_0428 | 0xC000_0142) => (
            format!("{emulator} açılırken Windows bir dosyasının yüklenmesine izin vermedi."),
            None,
            true,
        ),
        Some(0xC000_0005 | 0xC000_0409 | 0xC000_001D) => {
            (format!("{emulator} açılır açılmaz çöktü."), None, false)
        }
        Some(c) if c != 0 => (
            format!("{emulator} açılır açılmaz kapandı (kod {c:#x})."),
            None,
            false,
        ),
        _ => (format!("{emulator} açılır açılmaz kapandı."), None, false),
    };
    let errors = log.map(log_errors).unwrap_or_default();
    if !errors.is_empty() {
        message = format!("{message} {}", errors.join(" · "));
    }
    let core_failed = errors
        .iter()
        .any(|e| e.contains("libretro core") || e.contains("Failed to load symbol"));
    if let (true, Some(core)) = (core_failed, launch.core) {
        fix = Some(Fix::ReinstallCore(core.to_string()));
    }
    // No better explanation: a block is the likeliest cause when it's on.
    Some(blocked_by_smart_app_control(
        Diagnosis { message, fix },
        blocked || core_failed || errors.is_empty(),
    ))
}

/// When Smart App Control is enforcing and fits the failure, it is the cause:
/// re-downloading the same unsigned files would only be blocked again.
fn blocked_by_smart_app_control(diagnosis: Diagnosis, relevant: bool) -> Diagnosis {
    if diagnosis.fix == Some(Fix::SmartAppControl)
        || relevant && crate::windows::smart_app_control_enforced()
    {
        Diagnosis {
            message: format!(
                "{} Windows Akıllı Uygulama Denetimi imzasız emülatörleri engelliyor; kapatmak senin kararın \
                 (kapatınca Windows sıfırlanmadan yeniden açılamaz).",
                diagnosis.message
            ),
            fix: Some(Fix::SmartAppControl),
        }
    } else {
        diagnosis
    }
}

/// Where RetroArch writes this launch's log (`-v --log-file`).
pub fn retroarch_log(exe_dir: &Path) -> PathBuf {
    exe_dir.join("logs").join("lumen-launch.log")
}

/// The last two `[ERROR]` lines of a RetroArch log, without the prefix.
fn log_errors(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut errors: Vec<String> = text
        .lines()
        .filter_map(|l| l.split_once("[ERROR]").map(|(_, rest)| rest.trim()))
        .filter(|l| !l.is_empty())
        .map(|l| l.chars().take(160).collect())
        .collect();
    errors.dedup();
    let skip = errors.len().saturating_sub(2);
    errors.split_off(skip)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RETROARCH: Launch = Launch {
        emulator_id: "retroarch",
        emulator: "RetroArch",
        core: Some("mgba"),
    };

    #[test]
    fn sessions_are_not_failures() {
        assert_eq!(
            early_exit(&RETROARCH, Some(0), Duration::from_secs(600), None),
            None
        );
        // Quitting a game after a few seconds with a clean exit is fine too.
        assert_eq!(
            early_exit(&RETROARCH, Some(0), Duration::from_secs(5), None),
            None
        );
        assert_eq!(
            early_exit(&RETROARCH, Some(1), Duration::from_secs(60), None),
            None
        );
    }

    #[test]
    fn explains_early_exits_with_a_fix() {
        let dolphin = Launch {
            emulator_id: "dolphin",
            emulator: "Dolphin",
            core: None,
        };
        let missing = early_exit(
            &dolphin,
            Some(0xC000_0135u32 as i32),
            Duration::from_secs(1),
            None,
        )
        .unwrap();
        assert!(
            missing.message.contains("Visual C++"),
            "{}",
            missing.message
        );
        assert_eq!(missing.fix, Some(Fix::VcRedist));
        let damaged = early_exit(
            &dolphin,
            Some(0xC000_007Bu32 as i32),
            Duration::from_secs(1),
            None,
        )
        .unwrap();
        assert_eq!(damaged.fix, Some(Fix::ReinstallEmulator("dolphin".into())));
        let crash = early_exit(
            &dolphin,
            Some(0xC000_0005u32 as i32),
            Duration::from_secs(1),
            None,
        )
        .unwrap();
        assert_eq!(crash.fix, None);
    }

    #[test]
    fn a_core_that_failed_to_load_is_downloaded_again() {
        let dir = std::env::temp_dir().join(format!("lumen-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("retroarch.log");
        std::fs::write(
            &log,
            "[INFO] Loading core\n[ERROR] [Core]: Failed to open libretro core: \"mgba_libretro.dll\"\n\
             [ERROR] [Content]: Failed to load content.\n",
        )
        .unwrap();
        let found = early_exit(&RETROARCH, Some(1), Duration::from_secs(1), Some(&log)).unwrap();
        assert!(
            found.message.contains("Failed to open libretro core"),
            "{}",
            found.message
        );
        assert!(
            found.message.contains("Failed to load content"),
            "{}",
            found.message
        );
        assert_eq!(found.fix, Some(Fix::ReinstallCore("mgba".into())));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn explains_blocked_starts() {
        let xemu = Launch {
            emulator_id: "xemu",
            emulator: "xemu",
            core: None,
        };
        let virus = spawn_error(&xemu, &io::Error::from_raw_os_error(225));
        assert!(virus.message.contains("Defender"), "{}", virus.message);
        assert_eq!(virus.fix, Some(Fix::Defender));
        let policy = spawn_error(&xemu, &io::Error::from_raw_os_error(4551));
        assert_eq!(policy.fix, Some(Fix::SmartAppControl));
        let missing = spawn_error(&xemu, &io::Error::from_raw_os_error(2));
        assert_eq!(missing.fix, Some(Fix::ReinstallEmulator("xemu".into())));
    }
}
