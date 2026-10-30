//! Sleep and wake while a game runs. Before the handheld sleeps, Lumen's
//! quick menu takes focus so the emulator pauses (focus-loss pause, see
//! presets.rs); after waking the game stays paused behind the menu instead of
//! resuming mid-action with audio blaring, until "back to game".

use tauri::AppHandle;

#[cfg_attr(not(windows), allow(dead_code))]
pub const RESUMED_EVENT: &str = "system-resumed";

#[cfg(windows)]
pub fn watch(app: AppHandle) {
    use std::sync::OnceLock;
    use tauri::{Emitter, Manager};
    use windows_sys::Win32::System::Power::{
        PowerRegisterSuspendResumeNotification, DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
    };

    static APP: OnceLock<AppHandle> = OnceLock::new();
    if APP.set(app).is_err() {
        return;
    }

    unsafe extern "system" fn on_power(
        _context: *const core::ffi::c_void,
        kind: u32,
        _setting: *const core::ffi::c_void,
    ) -> u32 {
        let Some(app) = APP.get() else { return 0 };
        // The controller may wake up in its default (desktop) mode.
        if matches!(kind, PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND) {
            crate::ally::force_gamepad_mode();
        }
        let in_game = app
            .state::<crate::state::AppState>()
            .running
            .lock()
            .unwrap()
            .is_some();
        if !in_game {
            return 0;
        }
        match kind {
            PBT_APMSUSPEND => crate::launcher::open_quick_menu(app),
            PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => {
                crate::launcher::open_quick_menu(app);
                let _ = app.emit(RESUMED_EVENT, ());
            }
            _ => {}
        }
        0
    }

    // Registered for the life of the process, so the parameters are leaked on purpose.
    let params: &'static mut DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS =
        Box::leak(Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
            Callback: Some(on_power),
            Context: std::ptr::null_mut(),
        }));
    let mut registration = std::ptr::null_mut();
    // SAFETY: with DEVICE_NOTIFY_CALLBACK the recipient is a pointer to the
    // subscribe parameters, which live forever.
    unsafe {
        PowerRegisterSuspendResumeNotification(
            DEVICE_NOTIFY_CALLBACK,
            params as *mut DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS as _,
            &mut registration,
        );
    }
}

#[cfg(not(windows))]
pub fn watch(_app: AppHandle) {}
