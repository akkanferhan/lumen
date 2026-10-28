//! Background controller listener. While an emulator owns the screen the
//! webview cannot see the gamepad, so the in-game combo is detected here:
//! tap Select + Start for Lumen's quick menu, hold it to save and quit.

use crate::state::Running;
use gilrs::{Button, Gilrs};
use std::time::{Duration, Instant};
use tauri::AppHandle;

/// Holding the combo this long saves and quits instead of opening the menu.
const HOLD: Duration = Duration::from_millis(800);

pub const QUICK_MENU_EVENT: &str = "quick-menu";

pub fn spawn(app: AppHandle, running: Running) {
    std::thread::spawn(move || {
        let Ok(mut gilrs) = Gilrs::new() else { return };
        // When the combo went down, and whether the hold already fired.
        let mut held: Option<(Instant, bool)> = None;

        loop {
            while gilrs.next_event().is_some() {}

            let combo = gilrs
                .gamepads()
                .any(|(_, pad)| pad.is_pressed(Button::Select) && pad.is_pressed(Button::Start));
            let in_game = running.lock().unwrap().is_some();

            match (combo && in_game, held) {
                (true, None) => held = Some((Instant::now(), false)),
                (true, Some((since, false))) if since.elapsed() >= HOLD => {
                    if let Some(game) = running.lock().unwrap().as_mut() {
                        if let Some(stop) = game.stop.take() {
                            let _ = stop.send(());
                        }
                    }
                    held = Some((since, true));
                }
                // Released before the hold fired: a tap.
                (false, Some((_, false))) => {
                    if in_game {
                        crate::launcher::open_quick_menu(&app);
                    }
                    held = None;
                }
                (false, Some(_)) => held = None,
                _ => {}
            }

            std::thread::sleep(Duration::from_millis(16));
        }
    });
}
