//! Explains why an emulator didn't start: Windows refusing to run it (Smart
//! App Control, Defender, policies) or the emulator closing right away (a
//! missing runtime, a core that failed to load). Without this the user only
//! saw Lumen come back with nothing on screen.

use std::{
    io,
    path::{Path, PathBuf},
    time::Duration,
};

/// An exit this soon after launch is a failure, not a session.
const EARLY: Duration = Duration::from_secs(8);
/// Even with exit code 0: nobody plays for under this long.
const INSTANT: Duration = Duration::from_secs(3);

const SMART_APP_CONTROL: &str = "Windows Akıllı Uygulama Denetimi imzasız emülatörleri engelliyor. \
Kapatmak için: Windows Güvenliği → Uygulama ve tarayıcı denetimi → Akıllı Uygulama Denetimi → Kapalı \
(kapatınca Windows sıfırlanmadan yeniden açılamaz).";

/// Message for an emulator Windows refused to start.
pub fn spawn_error(emulator: &str, error: &io::Error) -> String {
    let reason = match error.raw_os_error() {
        Some(2 | 3) => format!("{emulator} dosyası bulunamadı; Emülatörler menüsünden yeniden kur."),
        Some(5) => format!(
            "Windows {emulator} için erişimi reddetti. Antivirüs ya da Denetimli klasör erişimi engelliyor olabilir."
        ),
        Some(193 | 216) => {
            format!("{emulator} dosyası bozuk ya da bu cihazla uyumsuz; Emülatörler menüsünden yeniden kur.")
        }
        Some(225 | 226) => format!(
            "Windows Defender {emulator} dosyasını tehdit sanıp engelledi. Emülatör resmi kaynağından indirildi; \
             Windows Güvenliği → Virüs ve tehdit koruması → Koruma geçmişi'nden izin verebilirsin."
        ),
        Some(740) => format!("{emulator} yönetici izni istedi; Lumen bunu vermiyor."),
        Some(1260) => format!("{emulator} bu bilgisayarda bir ilkeyle (sistem yöneticisi) engellenmiş."),
        // ERROR_SYSTEM_INTEGRITY_POLICY_VIOLATION and friends.
        Some(4551 | 4553 | 4556) => format!("{emulator} açılamadı. {SMART_APP_CONTROL}"),
        _ => format!("{emulator} başlatılamadı: {error}"),
    };
    with_smart_app_control_hint(reason, error.raw_os_error() == Some(5))
}

/// Message for an emulator that closed right after starting, or None for a
/// normal session.
pub fn early_exit(
    emulator: &str,
    code: Option<i32>,
    ran: Duration,
    log: Option<&Path>,
) -> Option<String> {
    let failed = ran < EARLY && code != Some(0) || ran < INSTANT;
    if !failed {
        return None;
    }
    // Windows exit codes are NTSTATUS values, negative as i32.
    let status = code.map(|c| c as u32);
    let (mut reason, blocked) = match status {
        Some(0xC000_0135) => (
            format!(
                "{emulator} açılamadı: Microsoft Visual C++ Redistributable (x64) eksik. \
                 aka.ms/vs/17/release/vc_redist.x64.exe adresinden kurabilirsin."
            ),
            false,
        ),
        Some(0xC000_007B) => (
            format!(
                "{emulator} dosyaları bozuk ya da uyumsuz; Emülatörler menüsünden yeniden kur."
            ),
            false,
        ),
        Some(0xC000_0022 | 0xC000_0428 | 0xC000_0142) => (
            format!("{emulator} açılırken Windows bir dosyasının yüklenmesine izin vermedi."),
            true,
        ),
        Some(0xC000_0005 | 0xC000_0409 | 0xC000_001D) => {
            (format!("{emulator} açılır açılmaz çöktü."), false)
        }
        Some(c) if c != 0 => (
            format!("{emulator} açılır açılmaz kapandı (kod {c:#x})."),
            false,
        ),
        _ => (format!("{emulator} açılır açılmaz kapandı."), false),
    };
    let errors = log.map(log_errors).unwrap_or_default();
    if !errors.is_empty() {
        reason = format!("{reason} {}", errors.join(" · "));
    }
    // No better explanation: a block is the likeliest cause when it's on.
    Some(with_smart_app_control_hint(
        reason,
        blocked || errors.is_empty(),
    ))
}

fn with_smart_app_control_hint(reason: String, relevant: bool) -> String {
    if relevant
        && !reason.contains(SMART_APP_CONTROL)
        && crate::windows::smart_app_control_enforced()
    {
        format!("{reason} {SMART_APP_CONTROL}")
    } else {
        reason
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

    #[test]
    fn sessions_are_not_failures() {
        assert_eq!(
            early_exit("RetroArch", Some(0), Duration::from_secs(600), None),
            None
        );
        // Quitting a game after a few seconds with a clean exit is fine too.
        assert_eq!(
            early_exit("RetroArch", Some(0), Duration::from_secs(5), None),
            None
        );
        assert_eq!(
            early_exit("RetroArch", Some(1), Duration::from_secs(60), None),
            None
        );
    }

    #[test]
    fn explains_early_exits() {
        let missing = early_exit(
            "Dolphin",
            Some(0xC000_0135u32 as i32),
            Duration::from_secs(1),
            None,
        )
        .unwrap();
        assert!(missing.contains("Visual C++"), "{missing}");
        let quick = early_exit("RetroArch", Some(1), Duration::from_secs(2), None).unwrap();
        assert!(quick.contains("kod 0x1"), "{quick}");
    }

    #[test]
    fn reads_retroarch_errors() {
        let dir = std::env::temp_dir().join(format!("lumen-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("retroarch.log");
        std::fs::write(
            &log,
            "[INFO] Loading core\n[ERROR] [Core]: Failed to open libretro core: \"mgba_libretro.dll\"\n\
             [ERROR] [Content]: Failed to load content.\n",
        )
        .unwrap();
        let message = early_exit("RetroArch", Some(1), Duration::from_secs(1), Some(&log)).unwrap();
        assert!(
            message.contains("Failed to open libretro core"),
            "{message}"
        );
        assert!(message.contains("Failed to load content"), "{message}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn explains_blocked_starts() {
        let virus = spawn_error("xemu", &io::Error::from_raw_os_error(225));
        assert!(virus.contains("Defender"), "{virus}");
        let policy = spawn_error("xemu", &io::Error::from_raw_os_error(4551));
        assert!(policy.contains("Akıllı Uygulama Denetimi"), "{policy}");
    }
}
