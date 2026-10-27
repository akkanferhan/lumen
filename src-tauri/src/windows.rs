//! Small Win32 helpers. No-ops elsewhere so the app still builds for UI work.

/// Asks every top-level window of `pid` to close (WM_CLOSE), the same as
/// clicking the window's X. Emulators then save and shut down normally.
/// Returns how many windows were asked.
#[cfg(windows)]
pub fn request_close(pid: u32) -> usize {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE,
    };

    struct Search {
        pid: u32,
        windows: Vec<HWND>,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> i32 {
        let search = &mut *(lparam as *mut Search);
        let mut owner = 0u32;
        GetWindowThreadProcessId(hwnd, &mut owner);
        if owner == search.pid && IsWindowVisible(hwnd) != 0 {
            search.windows.push(hwnd);
        }
        1 // keep enumerating
    }

    let mut search = Search {
        pid,
        windows: Vec::new(),
    };
    // SAFETY: `visit` only runs during this call and `search` outlives it.
    unsafe {
        EnumWindows(Some(visit), &mut search as *mut Search as LPARAM);
        for hwnd in &search.windows {
            PostMessageW(*hwnd, WM_CLOSE, 0, 0);
        }
    }
    search.windows.len()
}

#[cfg(not(windows))]
pub fn request_close(_pid: u32) -> usize {
    0
}
