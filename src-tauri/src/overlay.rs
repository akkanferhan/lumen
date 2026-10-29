//! A small touch button over the running game that opens the quick menu.
//! Unlike Select + Start it can't reach the game: a tap on the screen is
//! never a controller input. It is its own transparent, always-on-top
//! window, shown without taking focus once the game's window is up, and
//! hidden while Lumen is in front.

use tauri::{AppHandle, Manager};

pub const LABEL: &str = "overlay";
/// Logical size of the button window and its distance from the screen edges.
#[cfg(windows)]
const SIZE: f64 = 64.0;
#[cfg(windows)]
const MARGIN: f64 = 16.0;

/// Creates the (hidden) button window. Windows only: elsewhere Lumen runs as
/// a UI preview without emulators.
pub fn create(app: &AppHandle) {
    #[cfg(windows)]
    {
        use tauri::{WebviewUrl, WebviewWindowBuilder};
        let Ok(window) =
            WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
                .title("Lumen")
                .inner_size(SIZE, SIZE)
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .resizable(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .focused(false)
                .visible(false)
                .build()
        else {
            return;
        };
        // Top-right corner of the screen the game is on (the built-in one).
        if let Ok(Some(monitor)) = window.primary_monitor() {
            let scale = monitor.scale_factor();
            let x =
                monitor.position().x as f64 + monitor.size().width as f64 - (SIZE + MARGIN) * scale;
            let y = monitor.position().y as f64 + MARGIN * scale;
            let _ = window.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
        }
    }
    #[cfg(not(windows))]
    let _ = app;
}

pub fn show(app: &AppHandle) {
    let enabled = app
        .state::<crate::state::AppState>()
        .settings
        .lock()
        .unwrap()
        .overlay_button;
    #[cfg(windows)]
    if let (true, Some(window)) = (enabled, app.get_webview_window(LABEL)) {
        if let Ok(hwnd) = window.hwnd() {
            crate::windows::show_topmost_no_activate(hwnd.0 as isize);
        }
    }
    #[cfg(not(windows))]
    let _ = enabled;
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.hide();
    }
}
