//! Background controller listener. While an emulator owns the screen the
//! webview cannot see the gamepad, so the in-game combo is detected here:
//! tap a back button (Ally) or Select + Start for Lumen's quick menu, hold
//! it to save and quit.
//! Holding Select + LT rewinds and Select + RT fast-forwards (see `controls`).

use crate::controls::M2Action;
use crate::state::Running;
use gilrs::{Button, Gilrs};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// Holding the combo this long saves and quits instead of opening the menu.
const HOLD: Duration = Duration::from_millis(800);

pub const QUICK_MENU_EVENT: &str = "quick-menu";

pub fn spawn(app: AppHandle, running: Running) {
    std::thread::spawn(move || {
        let Ok(mut gilrs) = Gilrs::new() else { return };
        // When the combo went down, and whether the hold already fired.
        let mut held: Option<(Instant, bool)> = None;
        // Hotkeys Lumen is holding down in the game.
        let mut rewinding = false;
        let mut fast = false;
        // Select + Y was down last time round (one screenshot per press).
        let mut shot_held = false;

        loop {
            while gilrs.next_event().is_some() {}

            let (quick_combo, m2_action) = {
                let state = app.state::<crate::state::AppState>();
                let settings = state.settings.lock().unwrap();
                (settings.quick_combo, settings.m2_action)
            };
            // The Ally's M2 runs its shortcut without the game seeing it.
            let m2 = |action| crate::ally::m2_down() && m2_action == action;
            let playing = running.lock().unwrap().is_some();
            crate::ally::set_in_game(playing);
            // Ally back buttons (never seen by the game), or Select + Start.
            let combo = crate::ally::back_button_down()
                || (quick_combo
                    && gilrs.gamepads().any(|(_, pad)| {
                        pad.is_pressed(Button::Select) && pad.is_pressed(Button::Start)
                    }));
            let in_game = playing;

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

            let (want_rewind, want_fast) = {
                let pads: Vec<_> = gilrs.gamepads().map(|(_, pad)| pad).collect();
                let with = |button| {
                    pads.iter()
                        .any(|p| p.is_pressed(Button::Select) && p.is_pressed(button))
                };
                // Only into the game's own window: a key in Lumen (F5) would
                // reload its page.
                let game = running
                    .lock()
                    .unwrap()
                    .as_ref()
                    .and_then(|g| g.pid.filter(|_| !g.suspended).map(|pid| (pid, g.controls)));
                match game {
                    Some((pid, controls)) if crate::windows::is_foreground(pid) => (
                        controls.rewind && (with(Button::LeftTrigger2) || m2(M2Action::Rewind)),
                        controls.fast_forward
                            && (with(Button::RightTrigger2) || m2(M2Action::FastForward)),
                    ),
                    _ => (false, false),
                }
            };
            let shot = in_game_now(&running)
                && (m2(M2Action::Screenshot)
                    || gilrs
                        .gamepads()
                        .any(|(_, p)| p.is_pressed(Button::Select) && p.is_pressed(Button::North)));
            if shot && !shot_held {
                let app = app.clone();
                std::thread::spawn(move || {
                    let _ = crate::screenshots::take(&app);
                });
            }
            shot_held = shot;

            if want_rewind != rewinding {
                crate::controls::press(crate::controls::Hotkey::Rewind, want_rewind);
                rewinding = want_rewind;
            }
            if want_fast != fast {
                crate::controls::press(crate::controls::Hotkey::FastForward, want_fast);
                fast = want_fast;
            }

            std::thread::sleep(Duration::from_millis(16));
        }
    });
}

/// A game runs and its window is in front (Lumen's menu isn't).
fn in_game_now(running: &Running) -> bool {
    running
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|g| g.pid.filter(|_| !g.suspended))
        .is_some_and(crate::windows::is_foreground)
}
