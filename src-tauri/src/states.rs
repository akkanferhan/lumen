//! Save-state slots in the quick menu. RetroArch takes `SAVE_STATE_SLOT n` /
//! `LOAD_STATE_SLOT n` over its local UDP command port and answers once the
//! state is written or restored; each slot gets a screenshot beside it.
//! Standalone emulators have no such interface, so their quick menu has no
//! slots (they still resume where you left off).

use base64::Engine;
use serde::Serialize;
use std::{
    net::UdpSocket,
    path::{Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};

/// RetroArch's default `network_cmd_port`, set in Lumen's overrides.
pub const PORT: u16 = 55355;
pub const SLOTS: u8 = 4;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    pub slot: u8,
    /// Unix seconds of the last save; None for an empty slot.
    pub saved_at: Option<u64>,
    /// The screenshot RetroArch took with the state, as a data URL.
    pub thumbnail: Option<String>,
}

/// `<states>/<content name>.state<n>`: RetroArch names states after the
/// loaded file (cue, m3u, zip…), without sorting into subfolders.
fn state_path(states_dir: &Path, rom: &Path, slot: u8) -> Option<PathBuf> {
    let stem = rom.file_stem()?.to_string_lossy();
    Some(states_dir.join(format!("{stem}.state{slot}")))
}

pub fn slots(states_dir: &Path, rom: &Path) -> Vec<Slot> {
    (1..=SLOTS)
        .map(|slot| {
            let path = state_path(states_dir, rom, slot);
            let saved_at = path
                .as_ref()
                .and_then(|p| std::fs::metadata(p).ok())
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            let thumbnail = saved_at
                .and(path.as_ref())
                .and_then(|p| std::fs::read(format!("{}.png", p.display())).ok())
                .map(|png| {
                    format!(
                        "data:image/png;base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(png)
                    )
                });
            Slot {
                slot,
                saved_at,
                thumbnail,
            }
        })
        .collect()
}

/// Sends a command RetroArch doesn't answer (disc tray, disc swap).
pub fn send(command: &str) {
    if let Ok(socket) = UdpSocket::bind(("127.0.0.1", 0)) {
        let _ = socket.send_to(format!("{command}\n").as_bytes(), ("127.0.0.1", PORT));
    }
}

/// Sends `SAVE_STATE_SLOT n` or `LOAD_STATE_SLOT n` and waits for the answer.
pub fn command(load: bool, slot: u8) -> anyhow::Result<()> {
    let name = if load {
        "LOAD_STATE_SLOT"
    } else {
        "SAVE_STATE_SLOT"
    };
    let socket = UdpSocket::bind(("127.0.0.1", 0))?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    socket.send_to(format!("{name} {slot}\n").as_bytes(), ("127.0.0.1", PORT))?;
    let mut buf = [0u8; 512];
    let n = socket
        .recv(&mut buf)
        .map_err(|_| anyhow::anyhow!("Emülatör yanıt vermedi"))?;
    let reply = String::from_utf8_lossy(&buf[..n]);
    if reply.trim() == format!("{name} {slot}") {
        Ok(())
    } else if load {
        anyhow::bail!("Bu slotta yüklenebilecek bir kayıt yok")
    } else {
        anyhow::bail!("Oyun şu an kaydedilemiyor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_slots_with_their_screenshots() {
        let dir = std::env::temp_dir().join(format!("lumen-states-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Zelda (USA).state2"), b"state").unwrap();
        std::fs::write(dir.join("Zelda (USA).state2.png"), b"png").unwrap();
        let slots = slots(&dir, Path::new("C:/roms/snes/Zelda (USA).sfc"));
        assert_eq!(slots.len(), SLOTS as usize);
        assert!(slots[0].saved_at.is_none() && slots[0].thumbnail.is_none());
        assert!(slots[1].saved_at.is_some());
        assert_eq!(
            slots[1].thumbnail.as_deref(),
            Some("data:image/png;base64,cG5n")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn talks_to_retroarch_over_udp() {
        // A stand-in for RetroArch on the command port, if it's free here.
        let Ok(server) = UdpSocket::bind(("127.0.0.1", PORT)) else {
            return;
        };
        let handle = std::thread::spawn(move || {
            let mut buf = [0u8; 64];
            for answer in [
                "SAVE_STATE_SLOT 3\n",
                "LOAD_STATE_SLOT -1 no state in slot 2\n",
            ] {
                let (n, from) = server.recv_from(&mut buf).unwrap();
                assert!(String::from_utf8_lossy(&buf[..n]).ends_with('\n'));
                server.send_to(answer.as_bytes(), from).unwrap();
            }
        });
        assert!(command(false, 3).is_ok());
        assert!(command(true, 2).is_err());
        handle.join().unwrap();
    }
}
