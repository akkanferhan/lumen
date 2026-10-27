//! Small Win32 helpers. No-ops elsewhere so the app still builds for UI work.

/// Asks every top-level window of `pid` to close (WM_CLOSE), the same as
/// clicking the window's X. Emulators then save and shut down normally.
/// Returns how many windows were asked.
#[cfg(windows)]
pub fn request_close(pid: u32) -> usize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    let windows = process_windows(pid);
    for hwnd in &windows {
        // SAFETY: posting a message to a window handle we just enumerated.
        unsafe { PostMessageW(*hwnd, WM_CLOSE, 0, 0) };
    }
    windows.len()
}

/// Visible top-level windows owned by `pid`.
#[cfg(windows)]
fn process_windows(pid: u32) -> Vec<windows_sys::Win32::Foundation::HWND> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
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
    unsafe { EnumWindows(Some(visit), &mut search as *mut Search as LPARAM) };
    search.windows
}

#[cfg(not(windows))]
pub fn request_close(_pid: u32) -> usize {
    0
}

/// Brings a window to the foreground even when Lumen isn't the active app.
/// Windows only lets the foreground thread hand out focus, so we briefly
/// share its input queue (AttachThreadInput) while switching.
#[cfg(windows)]
pub fn bring_to_front(hwnd: isize) {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
        ShowWindow, SW_SHOW,
    };
    let hwnd = hwnd as windows_sys::Win32::Foundation::HWND;
    // SAFETY: plain Win32 calls on window handles; invalid handles just fail.
    unsafe {
        let current = GetCurrentThreadId();
        let foreground = GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
        let attached = foreground != 0
            && foreground != current
            && AttachThreadInput(current, foreground, 1) != 0;
        ShowWindow(hwnd, SW_SHOW);
        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd);
        if attached {
            AttachThreadInput(current, foreground, 0);
        }
    }
}

/// Focuses the main visible window of `pid` (back to the game).
#[cfg(windows)]
pub fn focus_process(pid: u32) -> bool {
    match process_windows(pid).first() {
        Some(hwnd) => {
            bring_to_front(*hwnd as isize);
            true
        }
        None => false,
    }
}

#[cfg(not(windows))]
pub fn focus_process(_pid: u32) -> bool {
    false
}
