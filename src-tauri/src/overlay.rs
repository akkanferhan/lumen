//! A small touch button over the running game that opens the quick menu.
//! Unlike Select + Start it can't reach the game: a tap on the screen is
//! never a controller input. It is its own transparent, always-on-top
//! window that never takes focus (not even when tapped), shown once the
//! game's window is up and hidden while Lumen is in front.

use tauri::{AppHandle, Manager};

pub const LABEL: &str = "overlay";
/// Logical size of the button window and its distance from the screen edges.
#[cfg(windows)]
const SIZE: f64 = 64.0;
#[cfg(windows)]
const MARGIN: f64 = 16.0;

/// Creates the (hidden) button window. Windows only: elsewhere Lumen runs as
/// a UI preview without emulators. It is placed and sized each time it is
/// shown (see `show`), so it never flashes up at a default size.
pub fn create(app: &AppHandle) {
    #[cfg(windows)]
    {
        use tauri::{WebviewUrl, WebviewWindowBuilder};
        let Ok(window) =
            WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?overlay".into()))
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
        if let Ok(hwnd) = window.hwnd() {
            crate::windows::make_no_activate(hwnd.0 as isize);
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
        // Top-right corner of the screen the game is on (the built-in one).
        let (Ok(hwnd), Ok(Some(monitor))) = (window.hwnd(), window.primary_monitor()) else {
            return;
        };
        let scale = monitor.scale_factor();
        let size = (SIZE * scale).round() as i32;
        let margin = (MARGIN * scale).round() as i32;
        let x = monitor.position().x + monitor.size().width as i32 - size - margin;
        let y = monitor.position().y + margin;
        crate::windows::show_topmost_no_activate(hwnd.0 as isize, x, y, size, size);
    }
    #[cfg(not(windows))]
    let _ = enabled;
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.hide();
    }
}
