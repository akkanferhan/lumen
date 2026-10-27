//! Background controller listener. While an emulator owns the screen the
//! webview cannot see the gamepad, so the "go home" combo is detected here.

use crate::state::Running;
use gilrs::{Button, Gilrs};
use std::time::{Duration, Instant};

/// How long Select + Start must be held to leave a game.
const HOLD: Duration = Duration::from_millis(800);

pub fn spawn(running: Running) {
    std::thread::spawn(move || {
        let Ok(mut gilrs) = Gilrs::new() else { return };
        let mut held_since: Option<Instant> = None;

        loop {
            while gilrs.next_event().is_some() {}

            let combo = gilrs
                .gamepads()
                .any(|(_, pad)| pad.is_pressed(Button::Select) && pad.is_pressed(Button::Start));

            if combo && running.lock().unwrap().is_some() {
                let since = *held_since.get_or_insert_with(Instant::now);
                if since.elapsed() >= HOLD {
                    if let Some(game) = running.lock().unwrap().as_mut() {
                        if let Some(kill) = game.kill.take() {
                            let _ = kill.send(());
                        }
                    }
                }
            } else {
                held_since = None;
            }

            std::thread::sleep(Duration::from_millis(16));
        }
    });
}
