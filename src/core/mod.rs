pub mod calendar;
pub mod config;
pub mod saver;
pub mod theme;
pub mod vault;

use std::sync::OnceLock;

/// Print a debug line to stderr when `OMADAY_DEBUG` is set.
pub fn debug(msg: impl AsRef<str>) {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var_os("OMADAY_DEBUG").is_some()) {
        eprintln!("omaday: {}", msg.as_ref());
    }
}
