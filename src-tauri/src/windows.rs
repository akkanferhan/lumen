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

/// `pid` and every process it started, directly or not. SharpEmu, for one,
/// runs the game in a child process that owns the window.
#[cfg(windows)]
fn process_tree(pid: u32) -> Vec<u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut pairs = Vec::new();
    // SAFETY: the snapshot handle is checked and closed; `entry.dwSize` is set
    // as Process32FirstW requires.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return vec![pid];
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            pairs.push((entry.th32ProcessID, entry.th32ParentProcessID));
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    descendants(pid, &pairs)
}

/// `root` followed by its descendants, from `(pid, parent pid)` pairs.
#[cfg(any(windows, test))]
fn descendants(root: u32, pairs: &[(u32, u32)]) -> Vec<u32> {
    let mut tree = vec![root];
    let mut i = 0;
    while i < tree.len() {
        let parent = tree[i];
        for &(pid, ppid) in pairs {
            if ppid == parent && pid != 0 && !tree.contains(&pid) {
                tree.push(pid);
            }
        }
        i += 1;
    }
    tree
}

/// Visible top-level windows owned by `pid` or one of its child processes.
#[cfg(windows)]
fn process_windows(pid: u32) -> Vec<windows_sys::Win32::Foundation::HWND> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
    };

    struct Search {
        pids: Vec<u32>,
        windows: Vec<HWND>,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> i32 {
        let search = &mut *(lparam as *mut Search);
        let mut owner = 0u32;
        GetWindowThreadProcessId(hwnd, &mut owner);
        if search.pids.contains(&owner) && IsWindowVisible(hwnd) != 0 {
            search.windows.push(hwnd);
        }
        1 // keep enumerating
    }

    let mut search = Search {
        pids: process_tree(pid),
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
    match main_window(pid) {
        Some(hwnd) => {
            bring_to_front(hwnd as isize);
            true
        }
        None => false,
    }
}

/// The largest visible window of `pid`: the game, not a splash or progress
/// window that some emulators (DuckStation) show first and then hide.
#[cfg(windows)]
fn main_window(pid: u32) -> Option<windows_sys::Win32::Foundation::HWND> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;
    process_windows(pid).into_iter().max_by_key(|&hwnd| {
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        // SAFETY: `hwnd` was just enumerated; a stale one leaves `rect` zeroed.
        unsafe { GetWindowRect(hwnd, &mut rect) };
        (rect.right - rect.left) as i64 * (rect.bottom - rect.top) as i64
    })
}

/// Whether the foreground window belongs to `pid` or one of its children.
#[cfg(windows)]
pub fn is_foreground(pid: u32) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    // SAFETY: no arguments; returns null when nothing has focus.
    let foreground = unsafe { GetForegroundWindow() };
    process_windows(pid).contains(&foreground)
}

#[cfg(not(windows))]
pub fn is_foreground(_pid: u32) -> bool {
    false
}

#[cfg(not(windows))]
pub fn focus_process(_pid: u32) -> bool {
    false
}

/// Freezes (`suspend = true`) or thaws every thread of `pid` and its child
/// processes via ntdll's NtSuspendProcess / NtResumeProcess, the calls
/// debuggers use. Used as a universal pause for emulators that can't pause on
/// focus loss. Returns whether `pid` itself was handled.
#[cfg(windows)]
pub fn set_process_suspended(pid: u32, suspend: bool) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SUSPEND_RESUME};

    type NtProcessCall =
        unsafe extern "system" fn(process: windows_sys::Win32::Foundation::HANDLE) -> i32;
    let ntdll: Vec<u16> = "ntdll.dll".encode_utf16().chain(Some(0)).collect();
    let name: &[u8] = if suspend {
        b"NtSuspendProcess\0"
    } else {
        b"NtResumeProcess\0"
    };

    // SAFETY: ntdll is always loaded; the looked-up export has the
    // NTSTATUS(HANDLE) signature; the handle is closed before returning.
    unsafe {
        let module = GetModuleHandleW(ntdll.as_ptr());
        if module.is_null() {
            return false;
        }
        let Some(proc_addr) = GetProcAddress(module, name.as_ptr()) else {
            return false;
        };
        let call: NtProcessCall = std::mem::transmute(proc_addr);
        let apply = |pid: u32| {
            let process = OpenProcess(PROCESS_SUSPEND_RESUME, 0, pid);
            if process.is_null() {
                return false;
            }
            let status = call(process);
            CloseHandle(process);
            status >= 0
        };
        let root = apply(pid);
        for child in process_tree(pid).into_iter().skip(1) {
            apply(child);
        }
        root
    }
}

#[cfg(not(windows))]
pub fn set_process_suspended(_pid: u32, _suspend: bool) -> bool {
    false
}

/// Presses or releases a key in the foreground window (the game) by scan
/// code, which both window messages and raw-input readers see.
#[cfg(windows)]
pub fn send_key(scan: u16, down: bool) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE,
    };
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: scan,
                dwFlags: KEYEVENTF_SCANCODE | if down { 0 } else { KEYEVENTF_KEYUP },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    // SAFETY: one fully initialised INPUT with its own size.
    unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) };
}

/// The whole primary screen as RGBA pixels (width, height, pixels), through
/// GDI: what's on screen in a borderless window, which is how Lumen runs
/// emulators. Some exclusive or flip-model swap chains read back black.
#[cfg(windows)]
pub fn capture_screen() -> Option<(u32, u32, Vec<u8>)> {
    use windows_sys::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT,
        DIB_RGB_COLORS, SRCCOPY,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    // SAFETY: GDI objects are created, used and released in order; the pixel
    // buffer is sized for the bitmap GetDIBits describes.
    unsafe {
        let (w, h) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
        if w <= 0 || h <= 0 {
            return None;
        }
        let screen = GetDC(std::ptr::null_mut());
        let memory = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(memory, bitmap as _);
        let copied = BitBlt(memory, 0, 0, w, h, screen, 0, 0, SRCCOPY | CAPTUREBLT) != 0;
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h, // top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let rows = GetDIBits(
            memory,
            bitmap,
            0,
            h as u32,
            pixels.as_mut_ptr().cast(),
            &mut info,
            DIB_RGB_COLORS,
        );
        SelectObject(memory, old);
        DeleteObject(bitmap as _);
        DeleteDC(memory);
        ReleaseDC(std::ptr::null_mut(), screen);
        if !copied || rows == 0 {
            return None;
        }
        // BGRA → RGBA, opaque.
        for px in pixels.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
            px[3] = 255;
        }
        Some((w as u32, h as u32, pixels))
    }
}

#[cfg(not(windows))]
pub fn capture_screen() -> Option<(u32, u32, Vec<u8>)> {
    None
}

/// Turns a window into one that never takes focus, even when tapped, and
/// stays out of Alt+Tab (the in-game menu button).
#[cfg(windows)]
pub fn make_no_activate(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    let hwnd = hwnd as windows_sys::Win32::Foundation::HWND;
    // SAFETY: plain Win32 calls on a window handle; an invalid one just fails.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            style | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as isize,
        );
    }
}

/// Hides a window (the in-game menu button).
#[cfg(windows)]
pub fn hide_window(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    // SAFETY: plain Win32 call on a window handle; an invalid one just fails.
    unsafe { ShowWindow(hwnd as windows_sys::Win32::Foundation::HWND, SW_HIDE) };
}

/// Places a window at the given physical rect and shows it above everything
/// in one step, without taking focus from the game: it is never on screen at
/// a stale size or position (the in-game menu button).
#[cfg(windows)]
pub fn show_topmost_no_activate(hwnd: isize, x: i32, y: i32, width: i32, height: i32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    };
    let hwnd = hwnd as windows_sys::Win32::Foundation::HWND;
    // SAFETY: plain Win32 call on a window handle; an invalid one just fails.
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

/// Whether Smart App Control is enforcing: it blocks unsigned executables
/// and DLLs without cloud reputation, which includes most emulator builds.
#[cfg(windows)]
pub fn smart_app_control_enforced() -> bool {
    use windows_sys::Win32::System::Registry::{
        RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD,
    };
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let key = wide(r"SYSTEM\CurrentControlSet\Control\CI\Policy");
    let value = wide("VerifiedAndReputablePolicyState");
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: wide strings are NUL-terminated; `data` is a DWORD of `size` bytes.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut data as *mut u32 as _,
            &mut size,
        )
    };
    // 0 off, 1 on, 2 evaluation (only logs).
    status == 0 && data == 1
}

#[cfg(not(windows))]
pub fn smart_app_control_enforced() -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn descendants_follow_the_whole_tree() {
        // 10 → 11 → 12, 10 → 13; 20 is unrelated; 0 is the idle process.
        let pairs = [(11, 10), (12, 11), (13, 10), (20, 1), (0, 0), (10, 10)];
        assert_eq!(super::descendants(10, &pairs), vec![10, 11, 13, 12]);
        assert_eq!(super::descendants(20, &pairs), vec![20]);
    }
}
