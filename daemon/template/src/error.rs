// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

use std::{ffi::CStr, fmt::Display, io, process};

pub type Result<T> = anyhow::Result<T>;

/// Log at critical level, naming the process, and exit with status 1.
/// For errors once the daemon is up; startup checks use [`errx`].
pub fn fatal(msg: impl Display) -> ! {
    crate::log::crit(&format!("fatal in {}: {msg:#}", crate::log::procname()));
    process::exit(1)
}

/// Log at error level and exit with status 1, like `errx(3)`.
pub fn errx(msg: impl Display) -> ! {
    log::error!("{msg:#}");
    process::exit(1)
}

/// Unwrap or exit.
pub trait OrFatal<T> {
    fn or_fatal(self) -> T;
    fn or_errx(self) -> T;
}

impl<T, E: Display> OrFatal<T> for std::result::Result<T, E> {
    fn or_fatal(self) -> T {
        match self {
            Ok(v) => v,
            Err(e) => fatal(e),
        }
    }

    fn or_errx(self) -> T {
        match self {
            Ok(v) => v,
            Err(e) => errx(e),
        }
    }
}

/// `strerror(3)` text for an I/O error, without the `(os error N)`
/// suffix std appends.
pub fn strerror(e: &io::Error) -> String {
    match e.raw_os_error() {
        // SAFETY: strerror returns a pointer to a static NUL-terminated
        // string for any errno.
        Some(n) => unsafe { CStr::from_ptr(libc::strerror(n)) }
            .to_string_lossy()
            .into_owned(),
        None => e.to_string(),
    }
}
