//! The C ABI the launcher uses to start Horizon inside its own process.
//!
//! The launcher owns the app and the JVM. `horizon_ios_start` is called on
//! the main thread after the launcher has prepared the resources folder; it
//! runs the game's event loop until the player quits. Every entry point
//! returns 0 on success and a negative error code otherwise.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

const ABI_VERSION: u32 = 1;
const ERR_ALREADY_STARTED: c_int = -2;
const ERR_INVALID_ARGUMENT: c_int = -3;
const ERR_PANIC: c_int = -4;
const ERR_FAILED: c_int = -1;

static STARTED: AtomicBool = AtomicBool::new(false);
static LAST_ERROR: OnceLock<CString> = OnceLock::new();

/// The ABI this library was built for. The launcher refuses a mismatch.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_ios_abi_version() -> u32 {
    ABI_VERSION
}

/// The library's version, NUL-terminated, for diagnostics.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_ios_version() -> *const c_char {
    c"0.1.0".as_ptr()
}

/// The message of the last failed call, NUL-terminated, or null.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_ios_last_error() -> *const c_char {
    LAST_ERROR
        .get()
        .map_or(std::ptr::null(), |error| error.as_ptr())
}

/// Starts the client. `resources` is the folder that has
/// `horizon-resources.json`; `game_dir` holds saves, options and logs;
/// `home` is the launcher's Horizon home. Runs until the game quits.
///
/// # Safety
/// The three pointers must be valid, NUL-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_ios_start(
    resources: *const c_char,
    game_dir: *const c_char,
    home: *const c_char,
) -> c_int {
    if resources.is_null() || game_dir.is_null() || home.is_null() {
        return ERR_INVALID_ARGUMENT;
    }
    if STARTED.swap(true, Ordering::SeqCst) {
        return ERR_ALREADY_STARTED;
    }
    let (Some(resources), Some(game_dir), Some(home)) = (
        string_arg(resources),
        string_arg(game_dir),
        string_arg(home),
    ) else {
        return ERR_INVALID_ARGUMENT;
    };
    let args = vec![
        "--resources".to_owned(),
        resources,
        "--game-dir".to_owned(),
        game_dir,
        "--home".to_owned(),
        home,
    ];
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        horizon_client::embed::run(args)
    })) {
        Ok(Ok(())) => 0,
        Ok(Err(error)) => {
            log::error!("Horizon iOS: {error}");
            set_error(error.to_string());
            ERR_FAILED
        }
        Err(_) => {
            log::error!("Horizon iOS: the game panicked");
            set_error("the game panicked".to_owned());
            ERR_PANIC
        }
    }
}

/// Placeholder for the launcher's stop button. The event loop ends when the
/// player quits; a later revision will inject a quit event from here.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_ios_stop() {}

fn string_arg(ptr: *const c_char) -> Option<String> {
    // SAFETY: the caller promises a valid NUL-terminated string.
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .ok()
        .map(str::to_owned)
}

fn set_error(message: String) {
    if let Ok(message) = CString::new(message.replace('\0', " ")) {
        let _ = LAST_ERROR.set(message);
    }
}
