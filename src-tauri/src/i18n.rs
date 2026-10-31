//! Turkish and English for the messages the backend writes (errors,
//! progress). The UI tells the backend which one it shows (`set_language`);
//! until then Windows' display language decides. Messages are written in
//! both languages where they're made: `tr!("{} kurulu değil", "{} is not
//! installed", name)`.

use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

/// Messages are in English.
pub fn english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

pub fn set_english(english: bool) {
    ENGLISH.store(english, Ordering::Relaxed);
}

/// Windows' display language is Turkish (or this isn't Windows).
pub fn system_is_turkish() -> bool {
    #[cfg(windows)]
    {
        // SAFETY: no arguments; returns the user's UI LANGID.
        let lang = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
        // Primary language id 0x1F is Turkish.
        lang & 0x3FF == 0x1F
    }
    #[cfg(not(windows))]
    true
}

/// `tr!(turkish, english, args…)`: a formatted message in the UI's language.
#[macro_export]
macro_rules! tr {
    ($tr:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        if $crate::i18n::english() {
            format!($en $(, $arg)*)
        } else {
            format!($tr $(, $arg)*)
        }
    };
}
